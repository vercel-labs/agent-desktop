use super::tests::fixture;
use super::{launcher_words, resolve};
use crate::system::launch::launch_attach::{Decision, decide};
use crate::system::process::executable_identity::ExecutableIdentity;
use crate::system::process::running_app::Running;

fn words(items: &[&str]) -> Vec<String> {
    items.iter().map(|word| (*word).to_owned()).collect()
}

#[test]
fn a_single_letter_switch_of_the_entry_is_not_part_of_the_identity() {
    assert_eq!(
        launcher_words(&words(&["/usr/bin/kate", "-b"])),
        words(&["/usr/bin/kate"])
    );
}

#[test]
fn long_options_and_positional_words_still_name_the_application() {
    let entry = words(&["/usr/bin/python3", "/opt/app.py", "--gui", "--mode=x", "-"]);

    assert_eq!(launcher_words(&entry), entry);
}

#[test]
fn the_program_word_is_never_dropped() {
    assert_eq!(launcher_words(&words(&["-x", "-b"])), words(&["-x"]));
}

fn running_without_arguments(program: &std::path::Path) -> Running {
    Running {
        executable: Some(ExecutableIdentity {
            executable: std::fs::canonicalize(program).expect("program"),
            argv: Some(vec![program.as_os_str().to_owned()]),
        }),
        pid: 1,
        instance: "fixture:1".into(),
        name: "fixture".into(),
    }
}

#[test]
fn an_entry_with_a_launcher_switch_still_finds_the_instance_that_runs_without_it() {
    let fixture = fixture(&[], &["editor"]);
    let program = fixture.search.path_dirs[0].join("editor");
    std::fs::write(
        fixture.search.data_dirs[0].join("editor.desktop"),
        format!(
            "[Desktop Entry]\nType=Application\nName=Editor\nExec={} -b %U\n",
            program.display()
        ),
    )
    .expect("entry");
    let index = crate::system::app::alias_cache::AliasCache::new().index(&fixture.search.data_dirs);
    let running = running_without_arguments(&program);

    let plan = resolve("editor", &[], &fixture.search, &index).expect("resolve");

    assert_eq!(
        decide(&plan.identity, std::slice::from_ref(&running), false, false),
        Decision::Refuse(vec![1])
    );
    let with_file = resolve(
        "editor",
        &["/tmp/note.txt".to_owned()],
        &fixture.search,
        &index,
    )
    .expect("resolve");
    assert_eq!(
        decide(&with_file.identity, &[running], false, true),
        Decision::Refuse(vec![1]),
        "the caller's arguments are not part of the question whether the app runs"
    );
}
