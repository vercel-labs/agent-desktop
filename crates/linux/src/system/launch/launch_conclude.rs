use agent_desktop_core::system::launch_options::LaunchOptions;
use agent_desktop_core::system::launch_result::LaunchResult;
use agent_desktop_core::{AdapterError, DeliverySemantics, LaunchGuidance, ProcessId, WindowInfo};

use super::launch_arrival::Arrival;
use crate::system::process::running_app::Running;

/// What the focus request answered the launch: the window read after it, and
/// whether an activation request was really sent. A window already active
/// satisfies `--activate` without a send, and the launch must not claim a
/// dispatch it never made.
pub(super) struct FocusAnswer {
    pub(super) window: Option<WindowInfo>,
    pub(super) dispatched: bool,
}

/// The result for an owner and its window. A window is brought forward once,
/// only when asked, and a refusal is reported in the suggestion because the
/// launch itself succeeded. A successful `focus` returns the window read after
/// the request, or `None` when no such read is available, and that window
/// replaces the one read before it.
pub(super) fn conclude(
    owner: &Running,
    window: Option<WindowInfo>,
    options: &LaunchOptions,
    arrival: Arrival,
    focus: impl FnOnce(&WindowInfo) -> Result<FocusAnswer, AdapterError>,
) -> LaunchResult {
    let mut notes = Vec::new();
    let mut window = window;
    let mut facts = arrival.facts();
    if !arrival.created() && (!options.env.is_empty() || options.cwd.is_some()) {
        notes.push(
            "A running instance was used, so the environment and working directory of this launch were not applied.".to_owned(),
        );
    }
    if options.activate
        && let Some(target) = &window
    {
        match focus(target) {
            Ok(answer) => {
                facts.activation_dispatched = answer.dispatched;
                facts.postcondition_observed = true;
                window = answer.window.or(window);
            }
            Err(error) => {
                facts.activation_dispatched =
                    error.disposition != DeliverySemantics::not_delivered();
                notes.push(format!(
                    "The window could not be brought forward: {}",
                    error.message
                ));
            }
        }
    }
    LaunchResult {
        app: owner.name.clone(),
        pid: ProcessId::new(owner.pid),
        process_instance: Some(owner.instance.clone()),
        window,
        guidance: LaunchGuidance {
            suggestion: (!notes.is_empty()).then(|| notes.join(" ")),
            ..LaunchGuidance::default()
        },
        disposition: facts.disposition(),
    }
}
