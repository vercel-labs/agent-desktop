use crate::{
    AppError, WindowInfo, WindowOp,
    adapter::{PlatformAdapter, WindowFilter},
    system::window_lookup,
};
use serde_json::{Value, json};

pub struct AppArgs {
    pub app: Option<String>,
    pub window_id: Option<String>,
}

pub(crate) fn window_op_command(
    args: AppArgs,
    adapter: &dyn PlatformAdapter,
    op: WindowOp,
    policy: crate::InteractionPolicy,
) -> Result<Value, AppError> {
    let deadline = crate::Deadline::standard()?;
    let win = resolve_window(
        args.app.as_deref(),
        args.window_id.as_deref(),
        adapter,
        deadline,
    )?;
    let lease = adapter.acquire_interaction_lease(deadline)?;
    let live = revalidate_window_for_mutation(adapter, &win, &lease)?;
    refuse_minimizing_the_focused_window(&live, &op, policy)?;
    let report = guarded_window_op(adapter, &live, op, policy, &lease)?;
    super::operation_response::operation_response(&report, json!({}))
}

/// How long the front guard keeps watching after a window operation that can
/// change the window order returns.
const WINDOW_SETTLE: std::time::Duration = std::time::Duration::from_millis(150);

/// Runs a minimize, maximize or restore under the front guard. A move or a
/// resize never changes the window order and does not come through here.
fn guarded_window_op(
    adapter: &dyn PlatformAdapter,
    live: &WindowInfo,
    op: WindowOp,
    policy: crate::InteractionPolicy,
    lease: &crate::InteractionLease,
) -> Result<crate::OperationReport, AppError> {
    let (report, front) = crate::system::front_guard::guarded(
        adapter,
        policy,
        lease,
        crate::system::front_guard::FrontWatch {
            owned_pid: Some(live.pid.get()),
            settle: WINDOW_SETTLE,
        },
        || adapter.window_op(live, op, policy, lease),
    );
    let mut report = report.map_err(|mut error| {
        front.note_in_error(&mut error);
        error
    })?;
    front.note_in(&mut report.result.details);
    Ok(report)
}

/// The window manager hands the focus to another window when the focused one
/// minimizes, so a headless minimize of it would move the user's focus.
fn refuse_minimizing_the_focused_window(
    live: &WindowInfo,
    op: &WindowOp,
    policy: crate::InteractionPolicy,
) -> Result<(), AppError> {
    if matches!(op, WindowOp::Minimize) && live.state.is_focused {
        policy.require(crate::Effect::FocusSteal, "minimize of the focused window")?;
    }
    Ok(())
}

pub(crate) fn resolve_window_for_app(
    app: Option<&str>,
    window_id: Option<&str>,
    adapter: &dyn PlatformAdapter,
) -> Result<WindowInfo, AppError> {
    resolve_window(app, window_id, adapter, crate::Deadline::standard()?)
}

fn resolve_window(
    app: Option<&str>,
    window_id: Option<&str>,
    adapter: &dyn PlatformAdapter,
    deadline: crate::Deadline,
) -> Result<WindowInfo, AppError> {
    if let Some(window_id) = window_id {
        let candidates = adapter
            .list_windows(
                &WindowFilter {
                    focused_only: false,
                    app: app.map(str::to_string),
                },
                deadline,
            )?
            .into_iter()
            .filter(|window| window.id == window_id)
            .collect();
        return window_lookup::select_window(
            candidates,
            crate::AdapterError::new(
                crate::ErrorCode::WindowNotFound,
                format!("Window '{window_id}' was not found"),
            )
            .with_suggestion("Run 'list-windows' to refresh window IDs, then retry."),
            "Multiple windows matched the requested window ID",
        );
    }
    let app = super::helpers::resolve_app(app, adapter, deadline)?;
    window_lookup::find_window_for_process(
        super::helpers::process_identity(&app)?,
        adapter,
        deadline,
    )
}

pub(crate) fn revalidate_window_for_mutation(
    adapter: &dyn PlatformAdapter,
    expected: &WindowInfo,
    lease: &crate::InteractionLease,
) -> Result<WindowInfo, AppError> {
    let live = adapter.resolve_window_strict(expected, lease.deadline())?;
    if live.id != expected.id
        || live.pid != expected.pid
        || live.process_instance != expected.process_instance
    {
        return Err(crate::AdapterError::new(
            crate::ErrorCode::StaleRef,
            "Window identity changed before mutation",
        )
        .into());
    }
    Ok(live)
}

#[cfg(test)]
#[path = "window_target_tests.rs"]
mod tests;
