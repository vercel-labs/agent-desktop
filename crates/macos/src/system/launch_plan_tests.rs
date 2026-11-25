use super::*;

fn arguments(documents: &[&str], plain: &[&str]) -> LaunchArguments {
    LaunchArguments {
        documents: documents.iter().map(|path| (*path).to_owned()).collect(),
        plain: plain.iter().map(|word| (*word).to_owned()).collect(),
    }
}

fn options(attach: bool, activate: bool) -> LaunchOptions {
    LaunchOptions {
        attach_if_running: attach,
        activate,
        ..LaunchOptions::default()
    }
}

fn everything_exists(path: &str) -> Option<String> {
    Some(path.to_owned())
}

#[test]
fn a_bare_word_stays_an_argument_even_when_a_file_of_that_name_exists() {
    let classified = LaunchArguments::classify_with(
        &["build".into(), "doc.txt".into()],
        None,
        everything_exists,
    );

    assert!(classified.documents.is_empty());
    assert_eq!(classified.plain, vec!["build", "doc.txt"]);
}

#[test]
fn an_explicit_path_to_an_existing_file_is_a_document() {
    let existing = |path: &str| (path != "/missing").then(|| path.to_owned());

    let classified = LaunchArguments::classify_with(
        &["/tmp/a.txt".into(), "./b.txt".into(), "/missing".into()],
        None,
        existing,
    );

    assert_eq!(classified.documents, vec!["/tmp/a.txt", "./b.txt"]);
    assert_eq!(classified.plain, vec!["/missing"]);
}

#[test]
fn a_file_url_is_a_document_with_its_escapes_decoded() {
    let classified = LaunchArguments::classify_with(
        &[
            "file:///tmp/my%20doc.txt".into(),
            "file://localhost/tmp/b.txt".into(),
        ],
        None,
        everything_exists,
    );

    assert_eq!(classified.documents, vec!["/tmp/my doc.txt", "/tmp/b.txt"]);
}

#[test]
fn a_home_relative_path_needs_a_home_and_a_relative_url_is_not_a_path() {
    let with_home = LaunchArguments::classify_with(
        &["~/notes.txt".into()],
        Some("/Users/me/"),
        everything_exists,
    );
    let without_home =
        LaunchArguments::classify_with(&["~/notes.txt".into()], None, everything_exists);
    let relative_url =
        LaunchArguments::classify_with(&["file://tmp/x".into()], None, everything_exists);

    assert_eq!(with_home.documents, vec!["/Users/me/notes.txt"]);
    assert_eq!(without_home.plain, vec!["~/notes.txt"]);
    assert_eq!(relative_url.plain, vec!["file://tmp/x"]);
}

#[test]
fn a_dash_prefixed_value_is_never_a_document() {
    let classified = LaunchArguments::classify_with(&["-x".into()], None, everything_exists);

    assert!(classified.documents.is_empty());
    assert_eq!(classified.plain, vec!["-x"]);
}

#[test]
fn nothing_running_starts_a_fresh_instance_whatever_the_arguments() {
    for attach in [true, false] {
        assert_eq!(
            plan(0, &options(attach, false), &arguments(&["/a"], &["x"])),
            LaunchPlan::Fresh
        );
    }
}

#[test]
fn no_attach_refuses_whenever_an_instance_runs() {
    for running in [1, 2] {
        assert_eq!(
            plan(running, &options(false, false), &arguments(&[], &[])),
            LaunchPlan::RefuseRunning
        );
    }
}

#[test]
fn plain_arguments_cannot_reach_a_running_instance_so_the_launch_refuses() {
    assert_eq!(
        plan(1, &options(true, false), &arguments(&[], &["--flag"])),
        LaunchPlan::RefuseArguments
    );
    assert_eq!(
        plan(1, &options(true, false), &arguments(&["/a"], &["--flag"])),
        LaunchPlan::RefuseArguments
    );
}

#[test]
fn documents_go_to_the_running_instance_through_launch_services() {
    assert_eq!(
        plan(1, &options(true, false), &arguments(&["/a"], &[])),
        LaunchPlan::Forward
    );
}

#[test]
fn a_bare_attach_only_reports_unless_it_is_asked_to_activate() {
    assert_eq!(
        plan(1, &options(true, false), &arguments(&[], &[])),
        LaunchPlan::Attach
    );
    assert_eq!(
        plan(1, &options(true, true), &arguments(&[], &[])),
        LaunchPlan::Activate
    );
}

#[test]
fn several_running_instances_are_ambiguous_when_attaching() {
    assert_eq!(
        plan(2, &options(true, false), &arguments(&[], &[])),
        LaunchPlan::Ambiguous
    );
}

#[test]
fn a_word_that_names_a_folder_of_the_working_directory_stays_an_argument() {
    let classified = LaunchArguments::classify(&["src".into(), "Cargo.toml".into()]);

    assert!(classified.documents.is_empty());
    assert_eq!(classified.plain, vec!["src", "Cargo.toml"]);
}
