//! Maximize on macOS is a frame move and resize, so the frame it replaced is
//! kept in the state root and `restore` puts it back.

use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionPolicy, OperationReport,
    ProcessIdentity, Rect, WindowFrameMemory, WindowFrameRecord, WindowInfo, WindowOp,
    WindowReadback,
};
use serde_json::json;

use crate::system::display_work_area;
use crate::system::window::{
    window_frame_place::{FramePlace, refreshed, same_frame},
    window_ops,
    window_postcondition::TOLERANCE,
    window_report, window_resolve,
    window_restore_plan::WindowRestorePlan,
};

const FRAME_READ_MS: u64 = 1_000;

pub(crate) fn window_op(
    win: &WindowInfo,
    op: WindowOp,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<OperationReport, AdapterError> {
    match op {
        WindowOp::Maximize => maximize(win, deadline),
        WindowOp::Restore => restore(win, policy, deadline),
        _ => dispatch(win, &op, window_report::window_readback(&op), deadline),
    }
}

fn dispatch(
    win: &WindowInfo,
    op: &WindowOp,
    readback: Option<WindowReadback>,
    deadline: Deadline,
) -> Result<OperationReport, AdapterError> {
    let dispatched = window_ops::execute(win, op.clone(), deadline);
    OperationReport::from_window_dispatch(win, op, readback, dispatched)
}

fn maximize(win: &WindowInfo, deadline: Deadline) -> Result<OperationReport, AdapterError> {
    if let Err(error) = record_before_maximize(win, deadline) {
        tracing::debug!(%error, "window frame was not recorded");
    }
    let report = dispatch(
        win,
        &WindowOp::Maximize,
        window_report::window_readback(&WindowOp::Maximize),
        deadline,
    );
    if let Err(error) = refresh_maximized_frame(win) {
        tracing::debug!(%error, "maximized window frame was not recorded");
    }
    report
}

fn record_before_maximize(win: &WindowInfo, deadline: Deadline) -> Result<(), AdapterError> {
    let (Some(before), Some(process)) = (win.bounds, process_of(win)) else {
        return Ok(());
    };
    let memory = WindowFrameMemory::open()?;
    let key = window_key(win);
    if memory
        .recall(&key)?
        .is_some_and(|held| FramePlace::of(&held, before) == FramePlace::Ours)
    {
        return Ok(());
    }
    let maximized = display_work_area::for_window(win.bounds, deadline)?;
    if same_frame(before, maximized) {
        return Ok(());
    }
    memory.remember(
        &key,
        WindowFrameRecord {
            process,
            before,
            maximized,
        },
        process_alive,
    )
}

/// Runs after the dispatch whatever it returned, so it reads the frame on a
/// deadline of its own: a maximize that waited out its deadline still records.
fn refresh_maximized_frame(win: &WindowInfo) -> Result<(), AdapterError> {
    let memory = WindowFrameMemory::open()?;
    let key = window_key(win);
    let Some(record) = memory.recall(&key)? else {
        return Ok(());
    };
    let current = current_frame(win, Deadline::detached_after(FRAME_READ_MS)?)?;
    let Some(record) = refreshed(record, current) else {
        return Ok(());
    };
    memory.remember(&key, record, process_alive)
}

fn restore(
    win: &WindowInfo,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<OperationReport, AdapterError> {
    let key = window_key(win);
    let record = match win.state.minimized {
        Some(false) => WindowFrameMemory::open()?.recall(&key)?,
        _ => None,
    };
    let plan = WindowRestorePlan::decide(
        win.state.minimized,
        record.as_ref(),
        || current_frame(win, deadline),
        |frame| display_work_area::for_window(Some(frame), deadline),
    )?
    .permitted_by(policy)?;
    match plan {
        WindowRestorePlan::Unminimize => dispatch(
            win,
            &WindowOp::Restore,
            window_report::window_readback(&WindowOp::Restore),
            deadline,
        ),
        WindowRestorePlan::AlreadyNormal => {
            forget_record(&key, record.as_ref())?;
            Ok(OperationReport::window_already_satisfied(
                win,
                &WindowOp::Restore,
            ))
        }
        WindowRestorePlan::Unrecorded => {
            forget_record(&key, record.as_ref())?;
            Err(unrecorded_error())
        }
        WindowRestorePlan::SetFrame(frame) => {
            set_frame(win, frame, deadline)?;
            forget_record(&key, record.as_ref())?;
            let size = WindowOp::Resize {
                width: frame.width,
                height: frame.height,
            };
            let readback = WindowReadback::for_window_op(&size, TOLERANCE, "points");
            OperationReport::from_window_dispatch(win, &WindowOp::Restore, Some(readback), Ok(()))
        }
    }
}

fn forget_record(key: &str, record: Option<&WindowFrameRecord>) -> Result<(), AdapterError> {
    if record.is_some() {
        WindowFrameMemory::open()?.forget(key)?;
    }
    Ok(())
}

fn unrecorded_error() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "The window fills its display and no earlier frame was recorded, so there is nothing to restore it to",
    )
    .with_suggestion(
        "Set the window size with resize-window and its position with move-window",
    )
    .with_details(json!({ "cause": "no_recorded_frame" }))
    .with_disposition(DeliverySemantics::not_delivered())
}

fn set_frame(win: &WindowInfo, frame: Rect, deadline: Deadline) -> Result<(), AdapterError> {
    let place = WindowOp::Move {
        x: frame.x,
        y: frame.y,
    };
    let size = WindowOp::Resize {
        width: frame.width,
        height: frame.height,
    };
    window_ops::execute(win, place, deadline)?;
    window_ops::execute(win, size, deadline)
}

fn current_frame(win: &WindowInfo, deadline: Deadline) -> Result<Rect, AdapterError> {
    let element = window_resolve::window_element_for_info(win, deadline)?;
    crate::tree::element_bounds::read_bounds_with_deadline(&element, deadline)?
        .ok_or_else(|| AdapterError::internal("window frame is unreadable"))
}

fn window_key(win: &WindowInfo) -> String {
    format!(
        "{}:{}:{}",
        win.pid,
        win.process_instance.as_deref().unwrap_or_default(),
        win.id
    )
}

fn process_of(win: &WindowInfo) -> Option<ProcessIdentity> {
    win.process_instance
        .as_deref()
        .map(|instance| ProcessIdentity::new(win.pid, instance))
}

fn process_alive(record: &WindowFrameRecord) -> bool {
    crate::system::process::process_identity::to_pid_t(record.process.pid)
        .and_then(|pid| {
            crate::system::process::process_identity::matches_instance(
                pid,
                &record.process.instance,
            )
        })
        .unwrap_or(false)
}
