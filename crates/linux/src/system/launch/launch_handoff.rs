use std::collections::HashSet;

use agent_desktop_core::{Deadline, app_name_matches};

use super::baseline::{Baseline, Resident, capture, same_executable};
use super::launch_step::Step;
use crate::system::SharedBus;
use crate::system::process::running_app::Running;

fn select_owner(
    before: &[Resident],
    after: &[(Running, HashSet<String>)],
    requested: &[String],
) -> Step {
    let mut owners = after.iter().filter_map(|(owner, windows)| {
        before.iter().find_map(|resident| {
            (resident.owner.identity() == owner.identity()
                && windows.iter().any(|id| !resident.windows.contains(id)))
            .then_some(owner)
        })
    });
    match (owners.next(), owners.next()) {
        (None, _) => forwarded_resident(before, after, requested),
        (Some(owner), None) => Step::Owner(owner.clone()),
        (Some(first), Some(second)) => Step::Ambiguous(
            [first.pid, second.pid]
                .into_iter()
                .chain(owners.map(|owner| owner.pid))
                .collect(),
        ),
    }
}

/// The launcher exited cleanly without a window of its own: it forwarded to a
/// single-instance application that was already running and only raised its
/// existing window, so no window id is new. When exactly one resident of the
/// executable is still running and it answers to a name the launch resolved
/// to, it is the instance the launch reached; several residents, none, or a
/// resident the request does not name stay unproven, because the executable
/// path alone never proves the forward arrived.
fn forwarded_resident(
    before: &[Resident],
    after: &[(Running, HashSet<String>)],
    requested: &[String],
) -> Step {
    let mut residents = after.iter().filter(|(owner, _)| {
        before
            .iter()
            .any(|resident| resident.owner.identity() == owner.identity())
    });
    match (residents.next(), residents.next()) {
        (Some((owner, _)), None) => {
            let named = before
                .iter()
                .find(|resident| resident.owner.identity() == owner.identity())
                .is_some_and(|resident| shares_identity(&resident.aliases, requested));
            if named {
                Step::Owner(owner.clone())
            } else {
                Step::Wait
            }
        }
        _ => Step::Wait,
    }
}

fn shares_identity(aliases: &[String], requested: &[String]) -> bool {
    aliases
        .iter()
        .any(|alias| requested.iter().any(|want| app_name_matches(alias, want)))
}

pub(super) fn observe(
    bus: &SharedBus,
    baseline: &Baseline,
    apps: &[Running],
    requested: &[String],
    deadline: Deadline,
) -> Step {
    let after: Vec<_> = apps
        .iter()
        .filter(|app| {
            baseline.residents.iter().any(|resident| {
                resident.owner.identity() == app.identity()
                    && same_executable(resident.owner.executable.as_ref(), app)
            })
        })
        .filter_map(|owner| {
            capture(bus, owner, deadline)
                .ok()
                .map(|windows| (owner.clone(), windows))
        })
        .collect();
    select_owner(&baseline.residents, &after, requested)
}

#[cfg(test)]
#[path = "launch_handoff_tests.rs"]
mod tests;
