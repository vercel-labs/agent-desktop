use crate::system::process::running_app::Running;
/// What a launch does about the applications already running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Decision {
    Spawn,
    Attach(Running),
    Forward,
    Refuse(Vec<u32>),
    Ambiguous(Vec<u32>),
}

/// Only verified canonical executable identity can establish attachment.
/// Bare long options are not compared, so `launch nautilus` attaches to
/// `nautilus --gapplication-service`.
pub(crate) fn matches(
    identity: &Option<crate::system::process::executable_identity::ExecutableIdentity>,
    app: &Running,
) -> bool {
    matches_with(identity, app, false)
}

fn matches_with(
    identity: &Option<crate::system::process::executable_identity::ExecutableIdentity>,
    app: &Running,
    documents: bool,
) -> bool {
    identity
        .as_ref()
        .zip(app.executable.as_ref())
        .is_some_and(|(wanted, actual)| wanted.is_running_as_with(actual, documents))
}

/// Decides before anything is spawned. No match spawns. One match is attached
/// to, and spawns only when arguments have to reach it, in which case the
/// spawned process forwards them or starts a second instance. With
/// `attach_if_running` false, any match refuses; otherwise several matches
/// are ambiguous.
#[cfg(test)]
pub(crate) fn decide(
    identity: &Option<crate::system::process::executable_identity::ExecutableIdentity>,
    running: &[Running],
    attach_if_running: bool,
    has_arguments: bool,
) -> Decision {
    decide_by(
        &[identity],
        false,
        running,
        attach_if_running,
        has_arguments,
    )
}

/// `decide` for a resolved launch: a process is the running instance when it
/// is the launch as requested, or the application the launch starts without
/// the caller's arguments and the launcher's own switches. The second name
/// finds an instance that was started plainly when the launch carries
/// arguments or the desktop entry adds a switch (`--no-attach` then refuses
/// it); it never matches a process that carries other words, so another
/// script or another `-c` code of the same interpreter stays a different
/// program.
pub(crate) fn decide_plan(
    plan: &super::launch_target::Plan,
    running: &[Running],
    attach_if_running: bool,
    has_arguments: bool,
) -> Decision {
    decide_by(
        &[&plan.identity, &plan.launcher],
        plan.takes_documents,
        running,
        attach_if_running,
        has_arguments,
    )
}

fn decide_by(
    identities: &[&Option<crate::system::process::executable_identity::ExecutableIdentity>],
    documents: bool,
    running: &[Running],
    attach_if_running: bool,
    has_arguments: bool,
) -> Decision {
    let found: Vec<&Running> = running
        .iter()
        .filter(|app| {
            identities
                .iter()
                .any(|identity| matches_with(identity, app, documents))
        })
        .collect();
    let pids = || found.iter().map(|app| app.pid).collect::<Vec<_>>();
    match found.as_slice() {
        [] => Decision::Spawn,
        _ if !attach_if_running => Decision::Refuse(pids()),
        [_, ..] if has_arguments => Decision::Forward,
        [one] => Decision::Attach((*one).clone()),
        _ => Decision::Ambiguous(pids()),
    }
}

#[cfg(test)]
#[path = "launch_attach_tests.rs"]
mod tests;
