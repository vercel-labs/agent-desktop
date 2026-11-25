use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::ExecutableIdentity;
use crate::system::launch::launch_attach::{Decision, decide};
use crate::system::process::running_app::Running;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("adx-script-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn script(dir: &Path, name: &str) -> PathBuf {
    let file = dir.join(name);
    std::fs::write(&file, "#!/bin/sh\n").expect("script");
    std::fs::canonicalize(file).expect("canonical script")
}

fn launch_of(script: &Path) -> ExecutableIdentity {
    ExecutableIdentity {
        executable: script.to_owned(),
        argv: Some(vec![script.as_os_str().to_owned()]),
    }
}

fn interpreter_running(words: &[&Path]) -> Running {
    let interpreter = std::fs::canonicalize("/bin/sh").expect("sh");
    Running {
        executable: Some(ExecutableIdentity {
            executable: interpreter,
            argv: Some(
                std::iter::once(OsString::from("/bin/sh"))
                    .chain(words.iter().map(|word| word.as_os_str().to_owned()))
                    .collect(),
            ),
        }),
        pid: 1,
        instance: "fixture:1".into(),
        name: "sh".into(),
    }
}

#[test]
fn an_interpreter_running_the_launched_script_is_the_running_instance() {
    let dir = scratch("same");
    let app = script(&dir, "app");
    let running = interpreter_running(&[&app]);
    let wanted = launch_of(&app);

    assert!(wanted.is_running_as(running.executable.as_ref().unwrap()));
    assert!(wanted.same_program(running.executable.as_ref().unwrap()));
    assert_eq!(
        decide(
            &Some(wanted.clone()),
            std::slice::from_ref(&running),
            false,
            false
        ),
        Decision::Refuse(vec![1])
    );
    assert_eq!(
        decide(&Some(wanted), std::slice::from_ref(&running), true, false),
        Decision::Attach(running)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_same_interpreter_running_another_script_is_not_the_running_instance() {
    let dir = scratch("other");
    let app = script(&dir, "app");
    let other = script(&dir, "other");
    let running = interpreter_running(&[&other]);

    assert!(!launch_of(&app).same_program(running.executable.as_ref().unwrap()));
    assert_eq!(
        decide(&Some(launch_of(&app)), &[running], false, false),
        Decision::Spawn
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_interpreter_that_only_opens_the_script_is_not_running_it() {
    let dir = scratch("opened");
    let app = script(&dir, "app");
    let running = interpreter_running(&[Path::new("-c"), Path::new("true"), &app]);

    assert!(!launch_of(&app).same_program(running.executable.as_ref().unwrap()));
    let _ = std::fs::remove_dir_all(&dir);
}
