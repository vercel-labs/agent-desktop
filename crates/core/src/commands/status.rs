use crate::{
    AppError, PermissionReport,
    adapter::PlatformAdapter,
    commands::permissions::{self, PermissionsArgs},
    context::CommandContext,
    refs::refs_store::RefStore,
};
use serde_json::{Value, json};

pub fn execute_with_report_with_context(
    adapter: &dyn PlatformAdapter,
    report: &PermissionReport,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let permissions = permissions::execute_with_report(
        PermissionsArgs {
            request: false,
            guide: false,
        },
        adapter,
        report,
    )?;

    let state_root = crate::storage::state_root::resolve_configured_state_root()?;
    let privacy = crate::storage::private_file_parent::ensure_private(&state_root);
    let (snapshot_id, ref_count) = if privacy.is_ok() {
        let store = RefStore::for_session(context.session_id())?;
        let snapshot_id = store.latest_snapshot_id()?;
        let ref_count = match snapshot_id.as_deref() {
            Some(snapshot_id) => Some(store.load_snapshot(snapshot_id)?.len()),
            None => None,
        };
        (snapshot_id, ref_count)
    } else {
        (None, None)
    };
    let privacy = match privacy {
        Ok(()) => json!({ "ok": true }),
        Err(error) => json!({
            "ok": false,
            "error": crate::ErrorPayload::from_app_error(&AppError::Io(error)),
        }),
    };
    let session_id = context.session_id().map(str::to_string);
    let tracing = context.trace_enabled();
    let artifacts = session_id
        .as_deref()
        .and_then(|id| crate::session::read_manifest(id).ok().flatten())
        .map(|manifest| manifest.artifacts);

    let mut body = json!({
        "platform": std::env::consts::OS,
        "version": env!("CARGO_PKG_VERSION"),
        "permissions": permissions,
        "snapshot_id": snapshot_id,
        "ref_count": ref_count,
        "session_id": session_id,
        "tracing": tracing,
        "state_root_privacy": privacy,
        "supported_surfaces": adapter
            .supported_surfaces()
            .into_iter()
            .map(|surface| surface.as_str())
            .collect::<Vec<_>>(),
    });
    if let Some(artifacts) = artifacts {
        body["artifacts"] = json!(artifacts);
    }
    body["state_root"] = json!(state_root.to_string_lossy());
    Ok(body)
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
