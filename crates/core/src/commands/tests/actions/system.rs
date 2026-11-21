use super::super::golden::assert_golden;
use crate::adapter::{ActionOps, InputOps, NoopAdapter, ObservationOps, SystemOps};
use crate::commands::close_app::{self, CloseAppArgs};
use crate::commands::helpers::AppArgs;
use crate::commands::key_down::{self, KeyDownArgs};
use crate::commands::key_up::{self, KeyUpArgs};
use crate::commands::mouse_down::{self, MouseDownArgs};
use crate::commands::mouse_up::{self, MouseUpArgs};
use crate::commands::move_window::{self, MoveWindowArgs};
use crate::commands::press::{self, PressArgs};
use crate::commands::resize_window::{self, ResizeWindowArgs};
use crate::commands::{maximize, minimize, restore};
use crate::context::CommandContext;
use crate::{
    Action, ActionResult, ActionStep, AdapterError, AppError, AppInfo, Deadline, ErrorPayload,
    InteractionLease, InteractionPolicy, KeyCombo, MouseButton, OperationReport, StepMechanism,
    WindowInfo, WindowOp, WindowReadback,
};
use serde_json::{Value, json};

#[derive(Default)]
struct AppStub {
    already_gone: bool,
    protected: Option<&'static str>,
}

impl ObservationOps for AppStub {
    fn query_app_inventory(
        &self,
        query: crate::AppInventoryQuery<'_>,
        _: Deadline,
    ) -> Result<crate::AppInventory, AdapterError> {
        Ok(query.answer(vec![AppInfo {
            aliases: Vec::new(),
            name: "Test".into(),
            pid: crate::ProcessId::new(42),
            bundle_id: None,
            process_instance: Some("test-instance".into()),
            presentation: None,
        }]))
    }
    crate::adapter::saved_ref_scope!(@inventory);
}

impl ActionOps for AppStub {}
impl InputOps for AppStub {}

impl SystemOps for AppStub {
    crate::adapter::guarded_interaction_lease!();
    crate::adapter::exact_window_focus!();

    fn is_protected_process(&self, identifier: &str) -> bool {
        self.protected == Some(identifier)
    }

    fn close_app(
        &self,
        app: &AppInfo,
        _: bool,
        _: &InteractionLease,
    ) -> Result<OperationReport, AdapterError> {
        OperationReport::from_close_dispatch(app, Ok(!self.already_gone))
    }

    fn window_op(
        &self,
        win: &WindowInfo,
        op: WindowOp,
        _: &InteractionLease,
    ) -> Result<OperationReport, AdapterError> {
        let readback = Some(WindowReadback::for_window_op(&op, 2.0, "px"));
        OperationReport::from_window_dispatch(win, &op, readback, Ok(()))
    }

    fn press_key_for_window(
        &self,
        _: &WindowInfo,
        combo: &KeyCombo,
        _: InteractionPolicy,
        _: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        let step =
            ActionStep::succeeded("key_press").with_mechanism(StepMechanism::PhysicalSynthetic);
        Ok(ActionResult::from_execution(
            &Action::PressKey(combo.clone()),
            vec![step],
        ))
    }
}

fn payload(error: AppError) -> Value {
    serde_json::to_value(ErrorPayload::from_app_error(&error)).unwrap()
}

fn close(app: &str, force: bool, adapter: &AppStub) -> Result<Value, AppError> {
    let policy = InteractionPolicy::headless();
    close_app::execute(
        CloseAppArgs {
            app: app.into(),
            force,
            policy,
        },
        adapter,
    )
}

#[test]
fn window_commands_and_a_window_scoped_press_report_the_adapter_results() {
    let app = || AppArgs {
        app: Some("Test".into()),
        window_id: None,
    };
    let stub = AppStub::default();
    let move_args = MoveWindowArgs {
        app: None,
        window_id: Some("w-42".into()),
        x: 120.0,
        y: 80.5,
    };
    let resize_args = ResizeWindowArgs {
        app: Some("Test".into()),
        window_id: None,
        width: 800.0,
        height: 600.0,
    };

    let press_args = PressArgs {
        combo: "cmd+s".into(),
        app: Some("Test".into()),
        window_id: Some("w-42".into()),
        force: false,
    };

    let data = json!({
        "press": press::execute(press_args, &stub, &CommandContext::default()).unwrap(),
        "move": move_window::execute(move_args, &stub).unwrap(),
        "resize": resize_window::execute(resize_args, &stub).unwrap(),
        "minimize": minimize::execute(app(), &stub).unwrap(),
        "maximize": maximize::execute(app(), &stub).unwrap(),
        "restore": restore::execute(app(), &stub).unwrap(),
    });

    assert_golden(
        data,
        include_str!("../../../../../../tests/fixtures/contract/actions-window_ops.json"),
        &[],
    );
}

#[test]
fn close_app_reports_method_exit_evidence_and_refuses_a_protected_process() {
    let gone = AppStub {
        already_gone: true,
        ..Default::default()
    };

    let guarded = AppStub {
        protected: Some("CriticalThing"),
        ..Default::default()
    };

    let data = json!({
        "graceful": close("Test", false, &AppStub::default()).unwrap(),
        "force": close("Test", true, &AppStub::default()).unwrap(),
        "already_gone": close("Test", false, &gone).unwrap(),
        "protected": payload(close("CriticalThing", false, &guarded).unwrap_err()),
    });

    assert_golden(
        data,
        include_str!("../../../../../../tests/fixtures/contract/actions-close_app.json"),
        &[],
    );
}

#[test]
fn held_input_commands_are_refused_with_an_atomic_replacement() {
    let headed = CommandContext::default().with_headed(true);
    let mouse_down = MouseDownArgs {
        x: 1.0,
        y: 2.0,
        button: MouseButton::Left,
        modifiers: vec![],
    };
    let mouse_up = MouseUpArgs {
        x: 1.0,
        y: 2.0,
        button: MouseButton::Right,
        modifiers: vec![],
    };

    let data = json!({
        "mouse_down": payload(mouse_down::execute(mouse_down, &NoopAdapter, &headed).unwrap_err()),
        "mouse_up": payload(mouse_up::execute(mouse_up, &NoopAdapter, &headed).unwrap_err()),
        "key_down": payload(key_down::execute(KeyDownArgs { combo: "shift".into(), force: false }, &NoopAdapter).unwrap_err()),
        "key_up": payload(key_up::execute(KeyUpArgs { combo: "shift".into(), force: true }, &NoopAdapter).unwrap_err()),
    });

    assert_golden(
        data,
        include_str!("../../../../../../tests/fixtures/contract/actions-held_input.json"),
        &[],
    );
}
