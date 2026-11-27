use agent_desktop_core::{AdapterError, DeliverySemantics, ProcessId, SurfaceInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloseDispatch {
    Sent,
    AlreadyGone,
}

#[cfg(target_os = "windows")]
pub(crate) fn open_sheet_surfaces(pid: ProcessId, instance: &str) -> Vec<SurfaceInfo> {
    let Ok(probe_deadline) = agent_desktop_core::Deadline::after(200) else {
        return Vec::new();
    };
    let identity = agent_desktop_core::ProcessIdentity {
        pid,
        instance: instance.to_string(),
    };
    crate::tree::surface_inventory::list_surfaces_for_process(identity, probe_deadline)
        .unwrap_or_default()
        .into_iter()
        .filter(|surface| surface.kind == agent_desktop_core::SnapshotSurface::Sheet.as_str())
        .collect()
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) fn timeout_with_sheets(
    timeout: AdapterError,
    app: &str,
    pid: ProcessId,
    force: bool,
    sheets: &[SurfaceInfo],
) -> AdapterError {
    if sheets.is_empty() {
        return timeout
            .with_details(serde_json::json!({
                "app": app,
                "pid": u32::from(pid),
                "force": force,
            }))
            .with_disposition(DeliverySemantics::delivered_unverified());
    }
    let sheet_summaries: Vec<serde_json::Value> = sheets
        .iter()
        .map(|surface| {
            serde_json::json!({
                "id": surface.id,
                "title": surface.title,
            })
        })
        .collect();
    timeout
        .with_details(serde_json::json!({
            "app": app,
            "pid": u32::from(pid),
            "force": force,
            "cause": "dialog_pending",
            "sheets": sheet_summaries,
        }))
        .with_suggestion(
            "A modal prompt or sheet is open and waiting for input. Run list-surfaces or snapshot to inspect and answer it, or retry with --force.",
        )
        .with_disposition(DeliverySemantics::delivered_unverified())
}
