use agent_desktop_core::{AdapterError, Deadline, system::launch_options::LaunchOptions};

use super::launch_plan::LaunchArguments;

pub(crate) fn open(
    id: &str,
    options: &LaunchOptions,
    arguments: &LaunchArguments,
    deadline: Deadline,
) -> Result<(i32, String), AdapterError> {
    if deadline.is_expired() {
        return Err(deadline
            .timeout_error()
            .with_disposition(agent_desktop_core::DeliverySemantics::not_delivered()));
    }
    crate::system::cocoa_runtime::ensure_cocoa_multithreaded().map_err(|error| {
        error.with_disposition(agent_desktop_core::DeliverySemantics::not_delivered())
    })?;
    let request = serde_json::to_vec(&serde_json::json!({
        "identifier": id,
        "bundle_id": super::launch::looks_like_bundle_id(id),
        "arguments": arguments.plain,
        "documents": arguments.documents,
        "environment": options.env,
        "activates": options.activate,
        "prompts": false,
        "substitution": false,
    }))
    .map_err(|error| {
        AdapterError::internal(format!("Encode launch request: {error}"))
            .with_disposition(agent_desktop_core::DeliverySemantics::not_delivered())
    })?;
    unsafe { crate::system::launch_completion::open_and_wait(&request, deadline) }
}
