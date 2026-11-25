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

#[test]
fn an_existing_path_is_a_document_and_everything_else_stays_an_argument() {
    let known = |argument: &str| (argument == "doc.txt").then(|| "/abs/doc.txt".to_owned());

    let classified = LaunchArguments::classify_with(
        &["doc.txt".into(), "--new".into(), "missing.txt".into()],
        known,
    );

    assert_eq!(classified.documents, vec!["/abs/doc.txt"]);
    assert_eq!(classified.plain, vec!["--new", "missing.txt"]);
}

#[test]
fn a_dash_prefixed_value_is_never_a_document_even_when_a_file_has_that_name() {
    let everything_exists = |argument: &str| Some(format!("/abs/{argument}"));

    let classified = LaunchArguments::classify_with(&["-x".into()], everything_exists);

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
