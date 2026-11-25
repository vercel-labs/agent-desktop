use std::ffi::OsString;

use super::ExecutableIdentity;

fn identity(words: &[&str]) -> ExecutableIdentity {
    ExecutableIdentity {
        executable: std::path::PathBuf::from("/apps/tool"),
        argv: Some(words.iter().map(OsString::from).collect()),
    }
}

#[test]
fn a_short_switch_the_process_carries_does_not_stop_a_bare_launch_naming_it() {
    let wanted = identity(&["/apps/tool"]);

    assert!(wanted.is_running_as(&identity(&["/apps/tool", "-b"])));
    assert!(wanted.is_running_as(&identity(&["/apps/tool", "--gui"])));
    assert!(!wanted.is_running_as(&identity(&["/apps/tool", "file"])));
    assert!(!wanted.is_running_as(&identity(&["/apps/tool", "-bx"])));
}
