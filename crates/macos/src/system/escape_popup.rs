//! Closing the open menu or popover of an application that does not hold the
//! keyboard. Core asks for it after a plain Escape that the key route refused
//! for focus: the surface can stay open in a window that does not.

use std::time::Duration;

use agent_desktop_core::{
    Action, ActionResult, ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode,
    InteractionLease, InteractionPolicy, KeyCombo, ProcessIdentity, StepMechanism, WindowInfo,
};

use crate::system::process::process_identity::{self, ProcessIdentity as Generation};
use crate::tree::AXElement;

pub(crate) const CANCEL: &str = "AXCancel";
const SURFACE_CLOSE_WAIT: Duration = Duration::from_millis(500);
const SURFACE_CLOSE_STEP: Duration = Duration::from_millis(10);

/// The reads and the one action a close needs, so the routing between a named
/// window and the whole process is testable without a live application. The
/// guards run before discovery and again immediately before the cancel.
pub(crate) trait TransientSurface {
    type Identity: Copy;
    type Surface;

    fn verify_process(&self, process: &ProcessIdentity) -> Result<Self::Identity, AdapterError>;

    fn require_generation(&self, identity: Self::Identity) -> Result<(), AdapterError>;

    fn in_window(
        &self,
        identity: Self::Identity,
        window: &WindowInfo,
    ) -> Result<Option<Self::Surface>, AdapterError>;

    fn in_process(&self, identity: Self::Identity) -> Result<Option<Self::Surface>, AdapterError>;

    fn verify_surface(
        &self,
        identity: Self::Identity,
        surface: &Self::Surface,
    ) -> Result<(), AdapterError>;

    fn cancel(&self, surface: &Self::Surface) -> Result<bool, AdapterError>;

    fn closed(&self, surface: &Self::Surface) -> bool;
}

struct AxSurfaces {
    deadline: Deadline,
}

impl TransientSurface for AxSurfaces {
    type Identity = Generation;
    type Surface = AXElement;

    fn verify_process(&self, process: &ProcessIdentity) -> Result<Generation, AdapterError> {
        process_identity::require_core(process)
    }

    fn require_generation(&self, identity: Generation) -> Result<(), AdapterError> {
        crate::tree::app_menu::require_generation(identity)
    }

    fn in_window(
        &self,
        identity: Generation,
        window: &WindowInfo,
    ) -> Result<Option<AXElement>, AdapterError> {
        crate::tree::surfaces::transient_surface_in_window(identity, window, self.deadline)
    }

    fn in_process(&self, identity: Generation) -> Result<Option<AXElement>, AdapterError> {
        crate::tree::surfaces::transient_surface_for_process(identity, self.deadline)
    }

    fn verify_surface(
        &self,
        identity: Generation,
        surface: &AXElement,
    ) -> Result<(), AdapterError> {
        verify_ax_surface(identity, surface, self.deadline)
    }

    fn cancel(&self, surface: &AXElement) -> Result<bool, AdapterError> {
        crate::actions::ax_helpers::try_ax_action_or_err(surface, CANCEL, self.deadline)
    }

    fn closed(&self, surface: &AXElement) -> bool {
        surface_closed(surface, self.deadline)
    }
}

/// Closes the surface `window` shows, or the one the process shows anywhere
/// when `window` shows none or is absent. `None` when none is open.
pub(crate) fn close_transient_surface(
    process: &ProcessIdentity,
    window: Option<&WindowInfo>,
    _policy: InteractionPolicy,
    lease: &InteractionLease,
) -> Result<Option<ActionResult>, AdapterError> {
    close_with(
        &AxSurfaces {
            deadline: lease.deadline(),
        },
        process,
        window,
    )
}

/// The close of one surface through its own `AXCancel`, verified by reading it
/// gone. `None` when the surface does not advertise the action. The surface is
/// revalidated against the process immediately before the cancel is sent.
pub(crate) fn cancel_surface(
    identity: Generation,
    surface: &AXElement,
    deadline: Deadline,
) -> Result<Option<ActionStep>, AdapterError> {
    verify_ax_surface(identity, surface, deadline)?;
    if !crate::actions::ax_helpers::try_ax_action_or_err(surface, CANCEL, deadline)? {
        return Ok(None);
    }
    Ok(Some(action_step(surface_closed(surface, deadline))))
}

fn close_with<S: TransientSurface>(
    surfaces: &S,
    process: &ProcessIdentity,
    window: Option<&WindowInfo>,
) -> Result<Option<ActionResult>, AdapterError> {
    let Some((identity, surface)) = find_surface(surfaces, process, window)? else {
        return Ok(None);
    };
    surfaces
        .verify_surface(identity, &surface)
        .map_err(not_delivered)?;
    if !surfaces.cancel(&surface)? {
        return Ok(None);
    }
    Ok(Some(closed_result(action_step(surfaces.closed(&surface)))))
}

/// The process generation a surface was found under, with the surface itself.
type Located<S> = (
    <S as TransientSurface>::Identity,
    <S as TransientSurface>::Surface,
);

/// The surface inside the named window when that window shows one, otherwise
/// the surface the process shows. Only the process is read without a window. A
/// named window must belong to the same process instance, and the process
/// generation is rechecked before the wider search falls back to it.
fn find_surface<S: TransientSurface>(
    surfaces: &S,
    process: &ProcessIdentity,
    window: Option<&WindowInfo>,
) -> Result<Option<Located<S>>, AdapterError> {
    if let Some(window) = window {
        require_window_process(process, window)?;
    }
    let identity = surfaces.verify_process(process).map_err(not_delivered)?;
    let Some(window) = window else {
        return Ok(surfaces
            .in_process(identity)
            .map_err(not_delivered)?
            .map(|surface| (identity, surface)));
    };
    if let Some(surface) = surfaces
        .in_window(identity, window)
        .map_err(not_delivered)?
    {
        return Ok(Some((identity, surface)));
    }
    surfaces
        .require_generation(identity)
        .map_err(not_delivered)?;
    Ok(surfaces
        .in_process(identity)
        .map_err(not_delivered)?
        .map(|surface| (identity, surface)))
}

/// A named window is evidence about one process instance: a window from a
/// replaced process would otherwise be searched for this one's surface.
fn require_window_process(
    process: &ProcessIdentity,
    window: &WindowInfo,
) -> Result<(), AdapterError> {
    let same_process = window.pid == process.pid
        && window.process_instance.as_deref() == Some(process.instance.as_str());
    if same_process {
        return Ok(());
    }
    Err(not_delivered(AdapterError::stale_ref_because(
        "Window does not match the target process instance",
    )))
}

/// Confirms the process generation and the surface's owning process
/// immediately before an action is sent to it.
fn verify_ax_surface(
    identity: Generation,
    surface: &AXElement,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    crate::tree::app_menu::require_generation(identity).map_err(not_delivered)?;
    match crate::tree::properties::ax_ipc::pid(surface, deadline) {
        Ok(pid) if pid == identity.pid() => {
            crate::tree::app_menu::require_generation(identity).map_err(not_delivered)
        }
        Ok(_) => Err(not_delivered(AdapterError::stale_ref_because(
            "Surface no longer belongs to the target process",
        ))),
        Err(error) if error == accessibility_sys::kAXErrorInvalidUIElement => Err(not_delivered(
            AdapterError::stale_ref_because("Surface became invalid before cancel"),
        )),
        Err(error) => Err(not_delivered(crate::tree::surface_read::map_error(
            "AXPid", error,
        ))),
    }
}

fn not_delivered(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::not_delivered())
}

fn action_step(verified: bool) -> ActionStep {
    ActionStep::succeeded(CANCEL)
        .with_mechanism(StepMechanism::SemanticApi)
        .with_verified(verified)
}

fn closed_result(step: ActionStep) -> ActionResult {
    let escape = Action::PressKey(KeyCombo {
        key: "escape".to_owned(),
        modifiers: Vec::new(),
    });
    ActionResult::from_execution(&escape, vec![step])
}

/// A closed surface leaves the tree: a popover's element becomes invalid, and
/// a menu's element stays readable but is no longer a child of its parent.
pub(crate) fn surface_closed(surface: &AXElement, deadline: Deadline) -> bool {
    let gone = || match crate::tree::surface_read::element(surface, "AXParent", deadline) {
        Err(error) => error.code == ErrorCode::StaleRef,
        Ok(None) => true,
        Ok(Some(parent)) => crate::tree::surface_read::elements(&parent, "AXChildren", deadline)
            .is_ok_and(|children| {
                !children
                    .iter()
                    .any(|child| crate::tree::same_element(child, surface))
            }),
    };
    deadline.poll_until(SURFACE_CLOSE_WAIT, SURFACE_CLOSE_STEP, gone, |gone| *gone)
}

#[cfg(test)]
#[path = "escape_popup_tests.rs"]
mod tests;
