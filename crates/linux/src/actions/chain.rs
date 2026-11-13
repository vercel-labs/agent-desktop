//! Linux rungs are semantic AT-SPI calls with no foreground effect, so every
//! chain runs under the headless policy and its steps become the action result.

use agent_desktop_core::{
    Action, ActionResult, AdapterError, ChainRung, Deadline, InteractionPolicy, execute_chain,
};

pub(crate) fn run_semantic_chain(
    action: &Action,
    rungs: &mut [ChainRung<'_>],
    deadline: Deadline,
    suggestion: &'static str,
) -> Result<ActionResult, AdapterError> {
    execute_chain(rungs, InteractionPolicy::headless(), deadline, suggestion)
        .map(|steps| ActionResult::from_execution(action, steps))
}
