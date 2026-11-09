use super::*;
use crate::system::process::executable_identity::ExecutableIdentity;

fn file(id: &str, name: &str, exec: &str) -> (String, String) {
    (
        id.into(),
        format!("[Desktop Entry]\nType=Application\nName={name}\nExec={exec}\n"),
    )
}

#[test]
fn every_shown_entry_whose_launcher_names_the_process_is_an_alias() {
    let actual = ExecutableIdentity::read(std::process::id()).expect("own identity");
    let command = actual
        .argv
        .as_ref()
        .unwrap()
        .iter()
        .map(|word| {
            format!(
                "\"{}\"",
                word.to_string_lossy()
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('%', "%%")
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    let executable = command.as_str();
    let entry = file("owned", "Owned App", executable);
    let index = AliasIndex::build(std::slice::from_ref(&entry), &[]);
    assert_eq!(index.names_for(&actual), ["Owned App"]);
    assert_eq!(index.ids_for(&actual), ["owned"]);
    let mut path_launched = actual.clone();
    let invocation = &mut path_launched.argv.as_mut().unwrap()[0];
    *invocation = std::path::Path::new(invocation).file_name().unwrap().into();
    assert_eq!(index.names_for(&path_launched), ["Owned App"]);
    let index = AliasIndex::build(&[entry, file("duplicate", "Another App", executable)], &[]);
    assert_eq!(index.names_for(&actual), ["Owned App", "Another App"]);
    assert_eq!(index.ids_for(&actual), ["owned", "duplicate"]);
    let index = AliasIndex::build(&[file("wrong", "Wrong App", "/missing/elsewhere")], &[]);
    assert!(index.names_for(&actual).is_empty());
}

#[test]
fn a_sole_desktop_interpreter_never_aliases_another_script() {
    let mut child = std::process::Command::new("/usr/bin/python3")
        .args(["-c", "import time; time.sleep(30)"])
        .spawn()
        .unwrap();
    let actual = loop {
        let actual = ExecutableIdentity::read(child.id()).unwrap();
        if actual
            .executable
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("python")
        {
            break actual;
        }
        std::thread::yield_now();
    };
    let index = AliasIndex::build(
        &[file("one", "App One", "/usr/bin/python3 /opt/one.py")],
        &[],
    );
    let names = index.names_for(&actual);
    let ids = index.ids_for(&actual);
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(names.is_empty());
    assert!(ids.is_empty());
}

#[test]
fn a_desktop_name_another_executable_also_uses_stays_an_alias() {
    let actual = ExecutableIdentity::read(std::process::id()).unwrap();
    let command = actual
        .argv
        .as_ref()
        .unwrap()
        .iter()
        .map(|word| {
            format!(
                "\"{}\"",
                word.to_string_lossy()
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('%', "%%")
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    let executable = command.as_str();
    let index = AliasIndex::build(
        &[
            file("one", "Editor", executable),
            file("two", "EDITOR", "/bin/true"),
        ],
        &[],
    );
    assert_eq!(index.names_for(&actual), ["Editor"]);
    assert_eq!(index.ids_for(&actual), ["one"]);
}

#[test]
fn an_in_place_desktop_edit_invalidates_the_previous_executable_alias() {
    use std::sync::Arc;
    use std::time::{Duration, SystemTime};
    let dir = std::env::temp_dir().join(crate::tree::live::live_launch::nonce("alias-cache"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("owned.desktop");
    let command = "/bin/sleep 30";
    let actual = ExecutableIdentity::requested(&["/bin/sleep".into(), "30".into()], &[]).unwrap();
    std::fs::write(&path, file("owned", "Before", command).1).unwrap();
    let cache = AliasCache::new();
    let dirs = [dir.clone()];
    let before = cache.index(&dirs);
    assert_eq!(before.names_for(&actual), ["Before"]);
    assert!(Arc::ptr_eq(&before, &cache.index(&dirs)));
    let length_before = std::fs::metadata(&path).unwrap().len();
    let directory_before = std::fs::metadata(&dir).unwrap().modified().unwrap();
    std::fs::write(&path, file("owned", "After!", "/bin/false 00").1).unwrap();
    std::fs::File::open(&path)
        .unwrap()
        .set_modified(SystemTime::now() + Duration::from_secs(60))
        .unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().len(), length_before);
    let after = cache.index(&dirs);
    assert_eq!(
        std::fs::metadata(&dir).unwrap().modified().unwrap(),
        directory_before
    );
    assert!(
        after.names_for(&actual).is_empty(),
        "old executable loses the edited alias"
    );
    let replacement =
        ExecutableIdentity::requested(&["/bin/false".into(), "00".into()], &[]).unwrap();
    assert_eq!(after.names_for(&replacement), ["After!"]);
    assert!(!Arc::ptr_eq(&before, &after));
    std::fs::remove_dir_all(dir).unwrap();
}

fn running(argv: &[&str]) -> ExecutableIdentity {
    ExecutableIdentity {
        executable: std::fs::canonicalize("/bin/sleep").unwrap(),
        argv: Some(argv.iter().map(std::ffi::OsString::from).collect()),
    }
}

#[test]
fn a_launcher_names_its_service_whatever_options_either_passes() {
    let index = AliasIndex::build(
        &[file(
            "org.example.Files",
            "Files",
            "/bin/sleep --new-window %U",
        )],
        &[],
    );

    assert_eq!(
        index.names_for(&running(&["sleep", "--gapplication-service"])),
        ["Files"]
    );
    assert_eq!(
        index.names_for(&running(&["/bin/sleep", "-n", "notes.txt"])),
        ["Files"]
    );
}

#[test]
fn a_launcher_argument_must_lead_the_process_arguments() {
    let index = AliasIndex::build(&[file("panel", "Wi-Fi", "/bin/sleep wifi")], &[]);

    assert_eq!(
        index.names_for(&running(&["sleep", "wifi", "--verbose"])),
        ["Wi-Fi"]
    );
    assert!(
        index
            .names_for(&running(&["sleep", "--gapplication-service"]))
            .is_empty()
    );
    assert!(
        index
            .names_for(&running(&["sleep", "bluetooth"]))
            .is_empty()
    );
}

#[test]
fn only_entries_shown_in_this_desktop_name_a_process() {
    let entry = |extra: &str| {
        crate::system::launch::desktop_entry::parse(
            "e",
            &format!("[Desktop Entry]\nType=Application\nName=E\nExec=/bin/sleep\n{extra}\n"),
        )
        .unwrap()
        .traits
    };
    let gnome = ["ubuntu", "GNOME"];

    assert!(entry("").shown_in(&gnome));
    assert!(!entry("NoDisplay=true").shown_in(&gnome));
    assert!(!entry("Hidden=true").shown_in(&gnome));
    assert!(entry("OnlyShowIn=GNOME;Unity;").shown_in(&gnome));
    assert!(!entry("OnlyShowIn=KDE;").shown_in(&gnome));
    assert!(!entry("NotShowIn=KDE;GNOME;").shown_in(&gnome));
    assert!(entry("NotShowIn=KDE;").shown_in(&gnome));
}

#[test]
fn an_option_that_carries_a_payload_is_compared_like_an_argument() {
    let index = AliasIndex::build(&[file("idle", "IDLE", "/bin/sleep -midlelib")], &[]);

    assert!(
        index
            .names_for(&running(&["sleep", "/opt/unrelated.py"]))
            .is_empty()
    );
    assert_eq!(
        index.names_for(&running(&["sleep", "-midlelib", "-n"])),
        ["IDLE"]
    );
}

#[test]
fn an_interpreter_running_the_entry_script_is_named_by_it() {
    let dir = std::env::temp_dir().join(crate::tree::live::live_launch::nonce("alias-script"));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("updater");
    std::fs::write(&script, "#!/bin/sleep\n").unwrap();
    let other = dir.join("other");
    std::fs::write(&other, "#!/bin/sleep\n").unwrap();
    let index = AliasIndex::build(&[file("updater", "Updater", script.to_str().unwrap())], &[]);

    let named = index.names_for(&running(&[
        "/bin/sleep",
        script.to_str().unwrap(),
        "--no-focus",
    ]));
    let unnamed = index.names_for(&running(&["/bin/sleep", other.to_str().unwrap()]));
    let mut opened = running(&["gedit", script.to_str().unwrap()]);
    opened.executable = std::fs::canonicalize("/bin/true").unwrap();
    let opened_by_editor = index.names_for(&opened);
    std::fs::write(&script, "#!/usr/bin/env sleep\n").unwrap();
    let through_env = index.names_for(&running(&["/bin/sleep", script.to_str().unwrap()]));
    std::fs::remove_dir_all(dir).unwrap();

    assert_eq!(named, ["Updater"]);
    assert!(unnamed.is_empty());
    assert!(opened_by_editor.is_empty());
    assert_eq!(through_env, ["Updater"]);
}

fn modes() -> AliasIndex {
    AliasIndex::build(
        &[
            file("cal", "Calendar", "/bin/sleep --calendar"),
            file("pick", "File Selection", "/bin/sleep --file-selection"),
        ],
        &[],
    )
}

#[test]
fn entries_that_differ_only_in_a_bare_option_name_only_their_own_process() {
    let index = modes();

    assert_eq!(
        index.names_for(&running(&["sleep", "--calendar"])),
        ["Calendar"]
    );
    assert_eq!(
        index.ids_for(&running(&["sleep", "--file-selection", "--title=x"])),
        ["pick"]
    );
}

#[test]
fn a_process_two_different_entries_could_be_gets_neither_alias() {
    let index = modes();

    assert!(
        index
            .names_for(&running(&["sleep", "--calendar", "--file-selection"]))
            .is_empty()
    );
    assert!(index.names_for(&running(&["sleep", "--info"])).is_empty());
}

#[test]
fn the_entry_with_the_most_arguments_in_the_process_wins_over_a_bare_entry() {
    let index = AliasIndex::build(
        &[
            file("plain", "Plain", "/bin/sleep"),
            file("cal", "Calendar", "/bin/sleep --calendar"),
        ],
        &[],
    );

    assert_eq!(
        index.names_for(&running(&["sleep", "--calendar"])),
        ["Calendar"]
    );
    assert_eq!(index.names_for(&running(&["sleep", "--info"])), ["Plain"]);
}
