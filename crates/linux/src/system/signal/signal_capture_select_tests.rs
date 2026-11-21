use agent_desktop_core::{ProcessIdentity, SignalFilter};

use super::selector;
use crate::system::process::process_facts::ProcessFacts;
use crate::system::process::toolkit_id::ToolkitId;
use crate::system::registry::registry_app::{RegistryApp, assemble};
use crate::tree::AtspiElement;

fn registry(apps: Vec<RegistryApp>) -> crate::system::registry::Registry {
    crate::system::registry::Registry {
        apps,
        skipped: Vec::new(),
    }
}

fn unanswering_app(pid: u32, comm: &str) -> RegistryApp {
    assemble(
        AtspiElement::new(":1.9", "/org/a11y/atspi/accessible/root", pid),
        ProcessFacts {
            executable: None,
            comm: comm.to_owned(),
            exe_basename: Some(comm.to_owned()),
            instance: "boot:1".to_owned(),
        },
        None,
        ToolkitId::default(),
    )
}

#[test]
fn a_pinned_process_is_selected_by_its_pid_whatever_name_the_app_now_has() {
    let filter = SignalFilter {
        app: Some("busy.py".to_owned()),
        process: Some(ProcessIdentity::new(7u32, "boot:1")),
    };
    let registry = registry(vec![
        unanswering_app(7, "python3"),
        unanswering_app(8, "python3"),
    ]);
    let select = selector(&filter, &registry);
    assert!(select(&registry.apps[0]));
    assert!(!select(&registry.apps[1]));
}

#[test]
fn an_unpinned_filter_selects_by_the_app_query() {
    let filter = SignalFilter {
        app: Some("python3".to_owned()),
        process: None,
    };
    let registry = registry(vec![
        unanswering_app(7, "python3"),
        unanswering_app(8, "other"),
    ]);
    let select = selector(&filter, &registry);
    assert!(select(&registry.apps[0]));
    assert!(!select(&registry.apps[1]));
}

#[test]
fn an_app_named_by_the_query_outranks_one_that_only_has_it_as_an_entry_name() {
    let filter = SignalFilter {
        app: Some("Calculator".to_owned()),
        process: None,
    };
    let mut aliased = unanswering_app(8, "python3");
    aliased.aliases = vec!["Calculator".to_owned()];
    let registry = registry(vec![unanswering_app(7, "calculator"), aliased]);

    let select = selector(&filter, &registry);

    assert!(select(&registry.apps[0]));
    assert!(!select(&registry.apps[1]));
}
