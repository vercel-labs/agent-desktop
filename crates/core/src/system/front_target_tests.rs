use super::*;

fn window(pid: u32) -> FrontWindow {
    FrontWindow {
        id: "w".into(),
        pid,
    }
}

fn process(image: &str, started_ms: u64) -> Option<FrontProcess> {
    Some(FrontProcess {
        image: image.into(),
        started_ms,
    })
}

#[test]
fn the_target_process_owns_its_windows_new_or_old() {
    let target = FrontTarget {
        pid: Some(7),
        image: None,
        forwarded: false,
    };
    assert!(target.owns(&window(7), false, || None, 0));
    assert!(target.owns(&window(7), true, || None, 0));
    assert!(!target.owns(&window(8), true, || process("x", 9_000), 0));
}

#[test]
fn a_new_process_of_the_launched_program_owns_a_new_window() {
    let target = FrontTarget::program("kate");
    assert!(target.owns(&window(50), true, || process("kate", 10_000), 9_500));
    assert!(target.owns(
        &window(50),
        true,
        || process("/usr/bin/Kate", 10_000),
        9_500
    ));
}

#[test]
fn a_launch_that_a_running_instance_took_owns_the_window_that_instance_opens() {
    let target = FrontTarget::program("nautilus");
    assert!(target.owns(&window(50), true, || process("nautilus", 1_000), 9_500));
}

#[test]
fn a_process_that_existed_before_the_capture_is_not_a_command_on_a_process_target() {
    let target = FrontTarget {
        pid: Some(7),
        image: Some("kate".into()),
        forwarded: false,
    };
    assert!(!target.owns(&window(50), true, || process("kate", 1_000), 9_500));
    assert!(target.owns(&window(50), true, || process("kate", 10_000), 9_500));
}

#[test]
fn a_running_process_of_another_program_is_the_users_even_for_a_launch() {
    let target = FrontTarget::program("nautilus");
    assert!(!target.owns(&window(50), true, || process("firefox", 1_000), 9_500));
}

#[test]
fn another_program_started_after_the_capture_is_the_users() {
    let target = FrontTarget::program("kate");
    assert!(!target.owns(&window(50), true, || process("firefox", 10_000), 9_500));
}

#[test]
fn an_old_window_of_another_process_is_never_owned_by_program_match() {
    let target = FrontTarget::program("kate");
    assert!(!target.owns(&window(50), false, || process("kate", 10_000), 9_500));
}

#[test]
fn a_process_nobody_can_describe_is_the_users() {
    let target = FrontTarget::program("kate");
    assert!(!target.owns(&window(50), true, || None, 0));
}

#[test]
fn desktop_ids_and_executables_name_the_same_program() {
    assert!(same_program("org.kde.kate", "kate"));
    assert!(same_program("NOTEPAD.EXE", "C:\\Windows\\notepad"));
    assert!(!same_program("", ""));
}
