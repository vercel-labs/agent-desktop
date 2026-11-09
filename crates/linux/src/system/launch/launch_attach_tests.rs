use super::*;

fn app(pid: u32, name: &str) -> Running {
    Running {
        executable: keys(&[name]),
        pid,
        instance: format!("boot:{pid}"),
        name: name.to_owned(),
    }
}

fn keys(items: &[&str]) -> Option<crate::system::process::executable_identity::ExecutableIdentity> {
    Some(
        crate::system::process::executable_identity::ExecutableIdentity {
            executable: std::path::PathBuf::from(format!("/apps/{}", items[0])),
            argv: Some(vec![format!("/apps/{}", items[0]).into()]),
        },
    )
}

#[test]
fn no_match_spawns() {
    let running = [app(1, "nautilus")];
    let decision = decide(&keys(&["gnome-calculator"]), &running, true, false);
    assert_eq!(decision, Decision::Spawn);
}

#[test]
fn one_match_without_arguments_attaches_and_spawns_nothing() {
    let owner = app(7, "gnome-text-editor");
    let decision = decide(
        &keys(&["gnome-text-editor"]),
        std::slice::from_ref(&owner),
        true,
        false,
    );
    assert_eq!(decision, Decision::Attach(owner));
}

#[test]
fn one_match_with_arguments_spawns_and_reports_the_owner() {
    let owner = app(7, "gnome-text-editor");
    let decision = decide(
        &keys(&["gnome-text-editor"]),
        std::slice::from_ref(&owner),
        true,
        true,
    );
    assert_eq!(decision, Decision::Forward);
}

#[test]
fn a_match_with_attach_off_refuses_naming_the_pid() {
    let owner = app(7, "gnome-calculator");
    let decision = decide(&keys(&["gnome-calculator"]), &[owner], false, false);
    assert_eq!(decision, Decision::Refuse(vec![7]));
}

#[test]
fn two_matches_are_ambiguous_and_two_with_attach_off_still_refuse() {
    let running = [app(7, "nautilus"), app(8, "nautilus")];
    let wanted = keys(&["nautilus"]);
    assert_eq!(
        decide(&wanted, &running, true, false),
        Decision::Ambiguous(vec![7, 8])
    );
    assert_eq!(
        decide(&wanted, &running, false, false),
        Decision::Refuse(vec![7, 8])
    );
}

fn started_with(pid: u32, name: &str, arguments: &[&str]) -> Running {
    let mut running = app(pid, name);
    if let Some(identity) = running.executable.as_mut() {
        let argv = identity.argv.get_or_insert_with(Vec::new);
        argv.extend(arguments.iter().map(std::ffi::OsString::from));
    }
    running
}

#[test]
fn a_bare_launch_attaches_to_the_running_service_started_with_options() {
    let service = started_with(7, "nautilus", &["--gapplication-service"]);

    assert_eq!(
        decide(
            &keys(&["nautilus"]),
            std::slice::from_ref(&service),
            true,
            false
        ),
        Decision::Attach(service)
    );
}

#[test]
fn a_bare_launch_does_not_attach_to_an_instance_running_another_file_or_code() {
    let editor = started_with(8, "kate", &["-n", "/tmp/notes.txt"]);
    let script = started_with(9, "python3", &["-c", "import time"]);

    assert_eq!(
        decide(&keys(&["kate"]), &[editor], true, false),
        Decision::Spawn
    );
    assert_eq!(
        decide(&keys(&["python3"]), &[script], true, false),
        Decision::Spawn
    );
}

#[test]
fn a_launch_that_asks_for_a_mode_does_not_attach_to_a_process_in_another_mode() {
    let mut gui = keys(&["tool"]);
    if let Some(identity) = gui.as_mut() {
        identity
            .argv
            .get_or_insert_with(Vec::new)
            .push("--gui".into());
    }
    let daemon = started_with(7, "tool", &["--daemon"]);

    assert_eq!(decide(&gui, &[daemon], true, false), Decision::Spawn);
}

#[test]
fn a_launch_that_asks_for_a_mode_attaches_to_the_process_running_in_that_mode() {
    let mut gui = keys(&["tool"]);
    if let Some(identity) = gui.as_mut() {
        identity
            .argv
            .get_or_insert_with(Vec::new)
            .push("--gui".into());
    }
    let running = started_with(7, "tool", &["--gui"]);

    assert_eq!(
        decide(&gui, std::slice::from_ref(&running), true, false),
        Decision::Attach(running)
    );
}
