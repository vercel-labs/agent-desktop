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
        pid: if id == "launched" { 9 } else { 1 },
    };
    FrontView {
        focused: Some(window.clone()),
        top: Some(window),
        windows: windows.iter().map(|id| (*id).into()).collect(),
        ..Default::default()
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

    fn front_process(&self, pid: u32) -> Option<crate::FrontProcess> {
        (pid == 9).then(|| crate::FrontProcess {
            image: "Fixture".into(),
            started_ms: u64::MAX / 2,
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

mod attach {
    use std::sync::Arc;

    use super::*;

    struct Attached {
        now: Arc<Mutex<FrontView>>,
        gave_back_to: Mutex<Vec<String>>,
    }

    fn window(id: &str, pid: u32) -> FrontWindow {
        FrontWindow { id: id.into(), pid }
    }

    fn view(id: &str, pid: u32, windows: &[&str]) -> FrontView {
        FrontView {
            focused: Some(window(id, pid)),
            top: Some(window(id, pid)),
            windows: windows.iter().map(|id| (*id).into()).collect(),
            ..Default::default()
        }
    }

    impl ObservationOps for Attached {
        fn query_app_inventory(
            &self,
            query: crate::AppInventoryQuery<'_>,
            _deadline: crate::Deadline,
        ) -> Result<crate::AppInventory, AdapterError> {
            Ok(query.answer(Vec::new()))
        }
    }
    impl ActionOps for Attached {}
    impl InputOps for Attached {}

    impl SystemOps for Attached {
        crate::adapter::guarded_interaction_lease!();

        /// An attach: the running instance gets the request, the call returns
        /// at once with that instance's pid, and its new window shows later.
        fn launch_app(
            &self,
            _id: &str,
            _options: &LaunchOptions,
            _lease: InteractionLease,
        ) -> Result<crate::system::launch_result::LaunchResult, AdapterError> {
            let now = Arc::clone(&self.now);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(120));
                *now.lock().unwrap() = view("late", 77, &["user", "late"]);
            });
            Ok(crate::system::launch_result::LaunchResult {
                app: "Editor".into(),
                pid: ProcessId::new(77),
                process_instance: Some("77:1".into()),
                window: None,
                guidance: crate::LaunchGuidance::default(),
                disposition: crate::DeliverySemantics::not_delivered(),
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
            *self.now.lock().unwrap() = view(&window.id, 1, &["user", "late"]);
            Ok(())
        }
    }

    #[test]
    fn a_launch_that_attached_to_a_running_instance_is_guarded_and_the_instance_is_the_target() {
        let desk = Attached {
            now: Arc::new(Mutex::new(view("user", 1, &["user"]))),
            gave_back_to: Mutex::new(Vec::new()),
        };
        let value = execute(
            LaunchArgs {
                app: "org.example.EditorLauncher".into(),
                options: LaunchOptions::default(),
            },
            &desk,
            InteractionPolicy::headless(),
        )
        .unwrap();
        assert_eq!(value["disposition"]["delivery"], "not_delivered");
        assert_eq!(value["focus_returned"], true);
        assert_eq!(desk.gave_back_to.lock().unwrap().as_slice(), ["user"]);
    }
}

mod stub_launcher {
    use super::*;

    const LAUNCHED: u32 = 77;

    struct Stub {
        now: Mutex<FrontView>,
        gave_back_to: Mutex<Vec<String>>,
        window_pid: u32,
        spawned: Vec<u32>,
    }

    fn view(id: &str, pid: u32, windows: &[&str]) -> FrontView {
        let window = FrontWindow { id: id.into(), pid };
        FrontView {
            focused: Some(window.clone()),
            top: Some(window),
            windows: windows.iter().map(|id| (*id).into()).collect(),
            ..FrontView::default()
        }
    }

    impl ObservationOps for Stub {
        fn query_app_inventory(
            &self,
            query: crate::AppInventoryQuery<'_>,
            _deadline: crate::Deadline,
        ) -> Result<crate::AppInventory, AdapterError> {
            Ok(query.answer(Vec::new()))
        }
    }
    impl ActionOps for Stub {}
    impl InputOps for Stub {}

    impl SystemOps for Stub {
        crate::adapter::guarded_interaction_lease!();

        /// The window of the launch takes the focus while the call is still
        /// running, before the launched pid is known to anyone but the adapter.
        fn launch_app(
            &self,
            _id: &str,
            _options: &LaunchOptions,
            _lease: InteractionLease,
        ) -> Result<crate::system::launch_result::LaunchResult, AdapterError> {
            *self.now.lock().unwrap() = view("late", self.window_pid, &["user", "late"]);
            std::thread::sleep(Duration::from_millis(80));
            Ok(crate::system::launch_result::LaunchResult {
                app: "Viewer".into(),
                pid: ProcessId::new(LAUNCHED),
                process_instance: Some("77:1".into()),
                window: None,
                guidance: crate::LaunchGuidance::default(),
                disposition: crate::DeliverySemantics::delivered_verified(),
            })
        }

        fn spawned_descendants(&self, root: u32, _since_ms: u64) -> Option<Vec<u32>> {
            (root == LAUNCHED).then(|| self.spawned.clone())
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
            *self.now.lock().unwrap() = view(&window.id, 1, &["user", "late"]);
            Ok(())
        }
    }

    fn launch_with(window_pid: u32, spawned: Vec<u32>) -> (Value, Vec<String>) {
        let stub = Stub {
            now: Mutex::new(view("user", 1, &["user"])),
            gave_back_to: Mutex::new(Vec::new()),
            window_pid,
            spawned,
        };
        let value = execute(
            LaunchArgs {
                app: "eventvwr.exe".into(),
                options: LaunchOptions::default(),
            },
            &stub,
            InteractionPolicy::headless(),
        )
        .unwrap();
        let gave_back_to = stub.gave_back_to.lock().unwrap().clone();
        (value, gave_back_to)
    }

    #[test]
    fn a_launched_program_that_is_not_named_like_its_window_owner_is_given_back_once_known() {
        let (value, gave_back_to) = launch_with(LAUNCHED, Vec::new());
        assert_eq!(value["focus_returned"], true);
        assert_eq!(gave_back_to, ["user"]);
    }

    #[test]
    fn a_stub_launcher_that_exits_after_spawning_the_window_owner_is_given_back() {
        let (value, gave_back_to) = launch_with(300, vec![300]);
        assert_eq!(value["focus_returned"], true);
        assert_eq!(gave_back_to, ["user"]);
    }

    #[test]
    fn a_window_of_an_unrelated_process_during_a_launch_is_not_given_back() {
        let (value, gave_back_to) = launch_with(400, vec![300]);
        assert!(value.get("focus_returned").is_none());
        assert!(gave_back_to.is_empty());
    }
}
