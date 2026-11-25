use crate::{AdapterError, DeliverySemantics, ErrorCode, ProcessId};

/// The refusal for a launch that must start a fresh instance while one already
/// runs. Every platform refuses the same way before anything is sent, so the
/// caller reads one error whatever the application or the system.
pub fn already_running(identifier: &str, pids: &[ProcessId]) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!("'{identifier}' is already running and a new launch was required"),
    )
    .with_details(serde_json::json!({
        "pid": pids.first(),
        "candidate_pids": pids,
    }))
    .with_suggestion(
        "Launch without --no-attach to attach to the running instance, or close the running instance first.",
    )
    .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(test)]
#[path = "launch_refusal_tests.rs"]
mod tests;
