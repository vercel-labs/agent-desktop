use std::collections::hash_map::RandomState;
use std::ffi::OsString;
use std::hash::{BuildHasher, Hasher};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use super::script_run;

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let base = std::env::temp_dir();
        for _ in 0..32 {
            let nonce = RandomState::new().build_hasher().finish();
            let dir = base.join(format!(
                "adx-exec-id-{label}-{}-{nonce:016x}",
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

    fn write_script(&self, name: &str, shebang: &str) -> PathBuf {
        let file = self.0.join(name);
        let mut handle = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o700)
            .open(&file)
            .expect("create script");
        handle
            .write_all(format!("{shebang}\n").as_bytes())
            .expect("write script");
        drop(handle);
        std::fs::canonicalize(file).expect("canonical script")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn words(items: &[&str]) -> Vec<OsString> {
    items.iter().map(OsString::from).collect()
}

fn sh() -> PathBuf {
    std::fs::canonicalize("/bin/sh").expect("/bin/sh")
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("utf-8 temp path")
}

#[test]
fn interpreter_option_operands_and_eval_flags_do_not_match_a_script() {
    let dir = TempDir::new("option-operand");
    let script = dir.write_script("app", "#!/bin/sh");
    let script_arg = path_str(&script);
    let interp = sh();

    assert_eq!(
        script_run(
            &script,
            &interp,
            &words(&["/bin/sh", "-X", script_arg, "-c", "evil"])
        ),
        None
    );
    assert_eq!(
        script_run(&script, &interp, &words(&["/bin/sh", "-c", script_arg])),
        None
    );
    assert_eq!(
        script_run(&script, &interp, &words(&["/bin/sh", "-m", script_arg])),
        None
    );
    assert_eq!(
        script_run(
            &script,
            &interp,
            &words(&["/bin/sh", "--", script_arg, "--flag"])
        ),
        None
    );
    let expected = words(&[script_arg, "--flag"]);
    assert_eq!(
        script_run(&script, &interp, &words(&["/bin/sh", script_arg, "--flag"])),
        Some(expected.as_slice())
    );
}

#[test]
fn direct_shebang_matches_only_its_declared_option_prefix() {
    let dir = TempDir::new("shebang-opt");
    let script = dir.write_script("app", "#!/bin/sh -u");
    let script_arg = path_str(&script);
    let interp = sh();

    let expected = words(&[script_arg, "--flag"]);
    assert_eq!(
        script_run(
            &script,
            &interp,
            &words(&["/bin/sh", "-u", script_arg, "--flag"])
        ),
        Some(expected.as_slice())
    );
    assert_eq!(
        script_run(&script, &interp, &words(&["/bin/sh", "-X", script_arg])),
        None
    );
    assert_eq!(
        script_run(
            &script,
            &interp,
            &words(&["/bin/sh", "-u", "-X", script_arg, "-c", "evil"])
        ),
        None
    );
}

#[test]
fn env_shebang_rejects_extra_options_and_supports_split_string() {
    let dir = TempDir::new("env-shebang");
    let plain = dir.write_script("plain", "#!/usr/bin/env sh");
    let split = dir.write_script("split", "#!/usr/bin/env -S sh -u");
    let unsplit = dir.write_script("unsplit", "#!/usr/bin/env sh -u");
    let env_opt = dir.write_script("env-opt", "#!/usr/bin/env -i sh");
    let interp = sh();

    let plain_arg = path_str(&plain);
    let plain_expected = words(&[plain_arg]);
    assert_eq!(
        script_run(&plain, &interp, &words(&["/bin/sh", plain_arg])),
        Some(plain_expected.as_slice())
    );
    assert_eq!(
        script_run(
            &plain,
            &interp,
            &words(&["/bin/sh", "-X", plain_arg, "-c", "evil"])
        ),
        None
    );

    let split_arg = path_str(&split);
    let split_expected = words(&[split_arg]);
    assert_eq!(
        script_run(&split, &interp, &words(&["/bin/sh", "-u", split_arg])),
        Some(split_expected.as_slice())
    );
    assert_eq!(
        script_run(&split, &interp, &words(&["/bin/sh", "-c", split_arg])),
        None
    );

    let unsplit_arg = path_str(&unsplit);
    assert_eq!(
        script_run(&unsplit, &interp, &words(&["/bin/sh", "-u", unsplit_arg])),
        None
    );

    let env_opt_arg = path_str(&env_opt);
    assert_eq!(
        script_run(&env_opt, &interp, &words(&["/bin/sh", env_opt_arg])),
        None
    );
}
