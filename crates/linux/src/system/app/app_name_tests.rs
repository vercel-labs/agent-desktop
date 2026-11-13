use super::*;
use crate::system::process::process_facts::ProcessFacts;

fn facts(comm: &str, exe: Option<&str>) -> ProcessFacts {
    ProcessFacts {
        executable: None,
        comm: comm.to_owned(),
        exe_basename: exe.map(str::to_owned),
        instance: "boot:1".to_owned(),
    }
}

#[test]
fn the_name_is_the_executable_basename_whatever_the_root_would_say() {
    let process = facts("org.gnome.Nauti", Some("org.gnome.Nautilus"));
    assert_eq!(app_name(&process), "org.gnome.Nautilus");
}

#[test]
fn the_name_prefers_the_executable_over_the_cut_comm() {
    let process = facts("gnome-terminal-", Some("gnome-terminal-server"));
    assert_eq!(app_name(&process), "gnome-terminal-server");
}

#[test]
fn a_missing_or_blank_executable_falls_back_to_comm() {
    assert_eq!(app_name(&facts("python", None)), "python");
    assert_eq!(app_name(&facts("code", Some("  "))), "code");
}

#[test]
fn an_executable_deleted_while_it_runs_keeps_its_name() {
    let replaced = facts("calc", Some("gnome-calculator (deleted)"));
    assert_eq!(app_name(&replaced), "gnome-calculator");
    assert_eq!(app_name(&facts("x", Some(" (deleted)"))), "x");
}

#[test]
fn aliases_of_identity_reads_the_desktop_names_an_executable_answers_to() {
    let index = AliasIndex::build(
        &[(
            "owned".to_owned(),
            "[Desktop Entry]\nType=Application\nName=Owned App\nExec=/bin/sleep owned.py\n"
                .to_owned(),
        )],
        &[],
    );
    let sleep = std::fs::canonicalize("/bin/sleep").expect("sleep");
    let identity = |script: &str| ExecutableIdentity {
        executable: sleep.clone(),
        argv: Some(vec!["/bin/sleep".into(), script.into()]),
    };
    assert_eq!(
        aliases_of_identity(&index, Some(&identity("owned.py"))),
        ["Owned App", "owned"]
    );
    assert!(aliases_of_identity(&index, Some(&identity("other.py"))).is_empty());
    assert!(aliases_of_identity(&index, None).is_empty());
}
