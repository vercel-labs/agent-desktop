use std::collections::hash_map::RandomState;
use std::ffi::OsString;
use std::hash::{BuildHasher, Hasher};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use super::ExecutableIdentity;
use crate::system::launch::launch_attach::{Decision, decide};
use crate::system::process::running_app::Running;

struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new(name: &str) -> Self {
        let base = std::env::temp_dir();
        for _ in 0..32 {
            let nonce = RandomState::new().build_hasher().finish();
            let dir = base.join(format!(
                "adx-script-{name}-{}-{nonce:016x}",
                std::process::id()
            ));
            match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
                Ok(()) => return Self(dir),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("scratch dir: {error}"),
            }
        }
        panic!("could not create unique scratch dir");
    }

    fn script(&self, name: &str) -> PathBuf {
        let file = self.0.join(name);
        let mut handle = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o700)
            .open(&file)
            .expect("create script");
        handle.write_all(b"#!/bin/sh\n").expect("write script");
        drop(handle);
        std::fs::canonicalize(file).expect("canonical script")
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
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
    let dir = ScratchDir::new("same");
    let app = dir.script("app");
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
}

#[test]
fn the_same_interpreter_running_another_script_is_not_the_running_instance() {
    let dir = ScratchDir::new("other");
    let app = dir.script("app");
    let other = dir.script("other");
    let running = interpreter_running(&[&other]);

    assert!(!launch_of(&app).same_program(running.executable.as_ref().unwrap()));
    assert_eq!(
        decide(&Some(launch_of(&app)), &[running], false, false),
        Decision::Spawn
    );
}

#[test]
fn an_interpreter_that_only_opens_the_script_is_not_running_it() {
    let dir = ScratchDir::new("opened");
    let app = dir.script("app");
    let running = interpreter_running(&[Path::new("-c"), Path::new("true"), &app]);

    assert!(!launch_of(&app).same_program(running.executable.as_ref().unwrap()));
}
