use super::resolve;
use super::tests::fixture;
use crate::system::launch::launch_attach::{Decision, decide_plan};
use crate::system::process::executable_identity::ExecutableIdentity;
use crate::system::process::running_app::Running;

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
        decide_plan(&plan, std::slice::from_ref(&running), false, false),
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
        decide_plan(&with_file, &[running], false, true),
        Decision::Refuse(vec![1]),
        "the caller's arguments are not part of the question whether the app runs"
    );
}

#[test]
fn another_invocation_of_the_same_interpreter_is_never_the_running_instance() {
    let fixture = fixture(&[], &[]);
    let index = crate::system::app::alias_cache::AliasCache::new().index(&fixture.search.data_dirs);
    let other_code = running_with("/usr/bin/python3", &["-c", "import time; time.sleep(30)"]);
    let other_script = running_with("/usr/bin/python3", &["/opt/other.py"]);
    let wanted = ["-c".to_owned(), "print(1)".to_owned()];
    let plan = resolve("/usr/bin/python3", &wanted, &fixture.search, &index).expect("resolve");

    for running in [other_code, other_script] {
        for attach in [true, false] {
            assert_eq!(
                decide_plan(&plan, std::slice::from_ref(&running), attach, true),
                Decision::Spawn
            );
        }
    }
    let bare = resolve("/usr/bin/python3", &[], &fixture.search, &index).expect("resolve");
    assert_eq!(
        decide_plan(
            &bare,
            &[running_with("/usr/bin/python3", &["/opt/other.py"])],
            false,
            false
        ),
        Decision::Spawn
    );
}

fn running_with(program: &str, args: &[&str]) -> Running {
    Running {
        executable: Some(ExecutableIdentity {
            executable: std::fs::canonicalize(program).expect("program"),
            argv: Some(
                std::iter::once(std::ffi::OsString::from(program))
                    .chain(args.iter().map(std::ffi::OsString::from))
                    .collect(),
            ),
        }),
        pid: 1,
        instance: "fixture:1".into(),
        name: "python3".into(),
    }
}

fn running_with_words(program: &std::path::Path, words: &[&str]) -> Running {
    Running {
        executable: Some(ExecutableIdentity {
            executable: std::fs::canonicalize(program).expect("program"),
            argv: Some(
                std::iter::once(program.as_os_str().to_owned())
                    .chain(words.iter().map(std::ffi::OsString::from))
                    .collect(),
            ),
        }),
        pid: 1,
        instance: "fixture:1".into(),
        name: "fixture".into(),
    }
}

fn entry_for(exec_tail: &str) -> (super::tests::Fixture, std::path::PathBuf, super::Plan) {
    let fixture = fixture(&[], &["files"]);
    let program = fixture.search.path_dirs[0].join("files");
    std::fs::write(
        fixture.search.data_dirs[0].join("files.desktop"),
        format!(
            "[Desktop Entry]\nType=Application\nName=Files\nExec={} {exec_tail}\n",
            program.display()
        ),
    )
    .expect("entry");
    let index = crate::system::app::alias_cache::AliasCache::new().index(&fixture.search.data_dirs);
    let plan = resolve("files", &[], &fixture.search, &index).expect("resolve");
    (fixture, program, plan)
}

#[test]
fn an_instance_that_opened_a_document_is_the_running_instance_when_the_entry_takes_documents() {
    let (_fixture, program, plan) = entry_for("--new-window %U");
    let running = running_with_words(&program, &["--new-window", "/home/user/dir"]);

    assert_eq!(
        decide_plan(&plan, std::slice::from_ref(&running), false, false),
        Decision::Refuse(vec![1])
    );
    assert_eq!(
        decide_plan(&plan, std::slice::from_ref(&running), true, false),
        Decision::Attach(running)
    );
}

#[test]
fn a_path_is_not_a_document_for_an_entry_that_takes_none() {
    let (_fixture, program, plan) = entry_for("--new-window");
    let running = running_with_words(&program, &["--new-window", "/opt/other-script"]);

    assert_eq!(
        decide_plan(&plan, &[running], false, false),
        Decision::Spawn
    );
}
