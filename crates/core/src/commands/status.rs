use crate::{
    AppError, ErrorCode, PermissionReport,
    adapter::PlatformAdapter,
    commands::permissions::{self, PermissionsArgs},
    context::CommandContext,
    refs::refs_store::RefStore,
};
use serde_json::{Value, json};

/// The latest snapshot in the selected namespace, as its id and ref count.
///
/// A pointer whose snapshot is no longer on disk is not "the latest snapshot in
/// the namespace": the snapshot was deleted, so the answer is `None` and the
/// report carries the same `null` it carries before any snapshot is taken. A
/// snapshot that exists but cannot be read stays an error, because reporting it
/// as absent would hide a damaged store behind a healthy-looking one.
fn latest_snapshot(store: &RefStore) -> Result<Option<(String, usize)>, AppError> {
    let Some(snapshot_id) = store.latest_snapshot_id()? else {
        return Ok(None);
    };
    match store.load_snapshot(&snapshot_id) {
        Ok(refmap) => Ok(Some((snapshot_id, refmap.len()))),
        Err(err) if err.code() == ErrorCode::SnapshotNotFound.as_str() => Ok(None),
        Err(err) => Err(err),
    }
}

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
        match latest_snapshot(&store)? {
            Some((snapshot_id, ref_count)) => (Some(snapshot_id), Some(ref_count)),
            None => (None, None),
        }
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
