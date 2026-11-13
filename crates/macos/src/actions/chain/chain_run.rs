use agent_desktop_core::{ActionStep, AdapterError, ChainRung};

use crate::actions::chain::ChainContext;
use crate::tree::AXElement;

/// Bounds every accessibility call the rungs make on the target and on its
/// application by the action's deadline, then runs the rungs through the core
/// delivery chain.
pub(crate) fn run_chain(
    element: &AXElement,
    ctx: &ChainContext<'_>,
    mut rungs: Vec<ChainRung<'_>>,
    suggestion: &'static str,
) -> Result<Vec<ActionStep>, AdapterError> {
    if let Some(pid) = crate::system::app::app_ops::pid_from_element(element, ctx.deadline) {
        crate::tree::properties::attributes::set_messaging_timeout(
            &crate::tree::element_for_pid(pid),
            ctx.deadline,
        )?;
    }
    crate::tree::properties::attributes::set_messaging_timeout(element, ctx.deadline)?;
    agent_desktop_core::execute_chain(&mut rungs, ctx.policy, ctx.deadline, suggestion)
}
