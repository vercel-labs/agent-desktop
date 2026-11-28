use std::cell::RefCell;
use std::collections::VecDeque;

use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, ErrorCode, ProcessId, ProcessIdentity, WindowInfo,
};

use super::world::PopupWorld;
use super::{close_popup_with, verify_live_process, verify_window_identity};
use crate::actions::write::mutation::WriteReply;
use crate::system::process::process_facts::ProcessFacts;
use crate::system::process::process_lookup::Lookup;
use crate::system::window::window_id;
use crate::tree::test_support::helpers::{read_of, words};
use crate::tree::vocabulary::role_numbers::{COMBO_BOX, FRAME, MENU};
use crate::tree::vocabulary::state_bits::SHOWING;
use crate::tree::{AtspiElement, Field, ObservedRead, WalkedNode};

const PID: u32 = 4242;
const INSTANCE: &str = "boot:100";

fn node(
    role: u32,
    states: &[u32],
    children: Vec<WalkedNode<ObservedRead>>,
) -> WalkedNode<ObservedRead> {
    let mut read = read_of(role, "n", &[]);
    read.node.traits.states = Field::Present(words(states));
    WalkedNode {
        payload: read,
        children,
        truncated: false,
        child_count: None,
        source_index: 0,
        in_web_document: false,
    }
}

fn facts(instance: &str) -> ProcessFacts {
    ProcessFacts {
        executable: None,
        comm: "app".to_owned(),
        exe_basename: None,
        instance: instance.to_owned(),
    }
}

fn process() -> ProcessIdentity {
    ProcessIdentity::new(ProcessId::new(PID), INSTANCE)
}

fn deadline() -> Deadline {
    Deadline::standard().expect("deadline")
}

fn window(instance: Option<&str>) -> WindowInfo {
    WindowInfo {
        id: window_id::encode(":1.42", "/org/a11y/atspi/accessible/root").expect("window id"),
        title: "target".into(),
        app: "app".into(),
        pid: ProcessId::new(PID),
        process_instance: instance.map(str::to_owned),
        bounds: None,
        state: Default::default(),
    }
}

fn popup_frame() -> WalkedNode<ObservedRead> {
    node(
        FRAME,
        &[SHOWING],
        vec![node(
            COMBO_BOX,
            &[],
            vec![node(MENU, &[SHOWING], Vec::new())],
        )],
    )
}

fn refusal() -> AdapterError {
    AdapterError::stale_ref_because("another owner")
}

struct World {
    live: RefCell<VecDeque<Lookup>>,
    windows: Result<Vec<WindowInfo>, AdapterError>,
    owner_checks: RefCell<VecDeque<Result<(), AdapterError>>>,
    walk: WalkedNode<ObservedRead>,
    calls: RefCell<Vec<&'static str>>,
}

impl World {
    fn matched() -> Self {
        Self {
            live: RefCell::new(VecDeque::new()),
            windows: Ok(vec![window(Some(INSTANCE))]),
            owner_checks: RefCell::new(VecDeque::new()),
            walk: popup_frame(),
            calls: RefCell::new(Vec::new()),
        }
    }

    fn record(&self, name: &'static str) {
        self.calls.borrow_mut().push(name);
    }

    fn count(&self, name: &str) -> usize {
        self.calls
            .borrow()
            .iter()
            .filter(|call| **call == name)
            .count()
    }
}

impl PopupWorld for World {
    fn live(&self, _pid: u32) -> Lookup {
        self.live
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| Lookup::Found(facts(INSTANCE)))
    }

    fn windows(&self, _pid: u32, _deadline: Deadline) -> Result<Vec<WindowInfo>, AdapterError> {
        self.record("windows");
        self.windows.clone()
    }

    fn verify_owner(
        &self,
        _expected: &ProcessIdentity,
        _element: &AtspiElement,
        _deadline: Deadline,
    ) -> Result<(), AdapterError> {
        self.record("owner");
        self.owner_checks.borrow_mut().pop_front().unwrap_or(Ok(()))
    }

    fn walk(
        &self,
        _frame: &AtspiElement,
        _deadline: Deadline,
    ) -> Result<WalkedNode<ObservedRead>, AdapterError> {
        self.record("walk");
        Ok(self.walk.clone())
    }

    fn verbs(
        &self,
        _owner: &AtspiElement,
        _role: Option<u32>,
        _deadline: Deadline,
    ) -> Result<(Vec<String>, &'static [&'static str]), AdapterError> {
        Ok((vec!["collapse".to_owned()], &["collapse"]))
    }

    fn dismiss(
        &self,
        _owner: &AtspiElement,
        _names: &[String],
        _verbs: &'static [&'static str],
        _deadline: Deadline,
    ) -> Result<WriteReply, AdapterError> {
        self.record("dismiss");
        Ok(WriteReply::Done)
    }

    fn popup_still_open(&self, _owner: &AtspiElement, _deadline: Deadline) -> Option<bool> {
        Some(false)
    }
}

#[test]
fn a_live_process_must_keep_its_instance() {
    let expected = process();
    assert!(verify_live_process(&expected, &Lookup::Found(facts(INSTANCE))).is_ok());
    for live in [
        Lookup::Gone,
        Lookup::Unreadable,
        Lookup::Found(facts("boot:other")),
    ] {
        let error = verify_live_process(&expected, &live).expect_err("refused");
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    }
}

#[test]
fn a_window_must_carry_the_original_instance() {
    let expected = process();
    assert!(verify_window_identity(&expected, &window(Some(INSTANCE))).is_ok());
    for instance in [None, Some("recycled")] {
        let error = verify_window_identity(&expected, &window(instance)).expect_err("refused");
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    }
}

#[test]
fn a_process_that_changed_before_discovery_fails_closed() {
    for live in [
        Lookup::Gone,
        Lookup::Unreadable,
        Lookup::Found(facts("boot:other")),
    ] {
        let world = World::matched();
        *world.live.borrow_mut() = VecDeque::from([live]);
        let error = close_popup_with(&world, &process(), None, deadline()).expect_err("refused");
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
        assert_eq!(world.count("windows"), 0);
        assert_eq!(world.count("owner"), 0);
        assert_eq!(world.count("dismiss"), 0);
    }
}

#[test]
fn a_failed_discovery_is_not_a_delivery() {
    let mut world = World::matched();
    world.windows = Err(AdapterError::new(ErrorCode::ActionFailed, "unreadable"));
    let error = close_popup_with(&world, &process(), None, deadline()).expect_err("refused");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(world.count("walk"), 0);
    assert_eq!(world.count("dismiss"), 0);
}

#[test]
fn a_window_of_another_generation_fails_closed_before_discovery() {
    for instance in [None, Some("recycled")] {
        let world = World::matched();
        let target = window(instance);
        let error =
            close_popup_with(&world, &process(), Some(&target), deadline()).expect_err("refused");
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
        assert_eq!(world.count("windows"), 0);
        assert_eq!(world.count("walk"), 0);
        assert_eq!(world.count("dismiss"), 0);
    }
}

#[test]
fn a_process_that_changed_after_discovery_fails_closed() {
    let world = World::matched();
    *world.live.borrow_mut() = VecDeque::from([
        Lookup::Found(facts(INSTANCE)),
        Lookup::Found(facts("boot:other")),
    ]);
    let error = close_popup_with(&world, &process(), None, deadline()).expect_err("refused");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(world.count("windows"), 1);
    assert_eq!(world.count("walk"), 0);
    assert_eq!(world.count("dismiss"), 0);
}

#[test]
fn a_foreign_bus_owner_before_the_action_fails_closed() {
    let world = World::matched();
    *world.owner_checks.borrow_mut() = VecDeque::from([Ok(()), Ok(()), Err(refusal())]);
    let error = close_popup_with(&world, &process(), None, deadline()).expect_err("refused");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(world.count("owner"), 3);
    assert_eq!(world.count("dismiss"), 0);
}

#[test]
fn matching_identity_and_bus_ownership_reach_the_action() {
    let world = World::matched();
    let settled = close_popup_with(&world, &process(), None, deadline())
        .expect("closed")
        .expect("a popup answered");
    assert_eq!(settled.action, "press");
    assert_eq!(world.count("owner"), 3);
    assert_eq!(world.count("dismiss"), 1);
}
