use std::sync::Mutex;

use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{FrontView, FrontWindow, InteractionLease, InteractionPolicy, ProcessId};

struct Desk {
    now: Mutex<FrontView>,
    gave_back_to: Mutex<Vec<String>>,
}

fn front(id: &str, windows: &[&str]) -> FrontView {
    let window = FrontWindow {
        id: id.into(),
        pid: 1,
    };
    FrontView {
        focused: Some(window.clone()),
        top: Some(window),
        windows: windows.iter().map(|id| (*id).into()).collect(),
    }
}

impl ObservationOps for Desk {
    fn query_app_inventory(
        &self,
        query: crate::AppInventoryQuery<'_>,
        _deadline: crate::Deadline,
    ) -> Result<crate::AppInventory, AdapterError> {
        Ok(query.answer(Vec::new()))
    }
}
impl ActionOps for Desk {}
impl InputOps for Desk {}

impl SystemOps for Desk {
    crate::adapter::guarded_interaction_lease!();

    fn launch_app(
        &self,
        _id: &str,
        _options: &LaunchOptions,
        _lease: InteractionLease,
    ) -> Result<crate::system::launch_result::LaunchResult, AdapterError> {
        *self.now.lock().unwrap() = front("launched", &["user", "launched"]);
        std::thread::sleep(Duration::from_millis(80));
        Ok(crate::system::launch_result::LaunchResult {
            app: "Fixture".into(),
            pid: ProcessId::new(42),
            process_instance: Some("42:1".into()),
            window: None,
            guidance: crate::LaunchGuidance::default(),
            disposition: crate::DeliverySemantics::delivered_verified(),
        })
    }

    fn read_front(&self, _deadline: crate::Deadline) -> Result<FrontView, AdapterError> {
        Ok(self.now.lock().unwrap().clone())
    }

    fn give_back(
        &self,
        window: &FrontWindow,
        _lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        self.gave_back_to.lock().unwrap().push(window.id.clone());
        *self.now.lock().unwrap() = front(&window.id, &["user", "launched"]);
        Ok(())
    }
}

fn run(policy: InteractionPolicy) -> (Value, Vec<String>) {
    let desk = Desk {
        now: Mutex::new(front("user", &["user"])),
        gave_back_to: Mutex::new(Vec::new()),
    };
    let value = execute(
        LaunchArgs {
            app: "Fixture".into(),
            options: LaunchOptions::default(),
        },
        &desk,
        policy,
    )
    .unwrap();
    (value, desk.gave_back_to.into_inner().unwrap())
}

#[test]
fn a_headless_launch_whose_application_takes_the_focus_gives_it_back_and_says_so() {
    let (value, gave_back_to) = run(InteractionPolicy::headless());
    assert_eq!(value["focus_returned"], true);
    assert_eq!(gave_back_to, ["user"]);
}

#[test]
fn a_launch_that_may_take_the_focus_is_not_guarded() {
    let (value, gave_back_to) = run(InteractionPolicy::headed());
    assert!(value.get("focus_returned").is_none());
    assert!(gave_back_to.is_empty());
}
