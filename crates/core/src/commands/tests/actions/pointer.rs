use super::super::golden::assert_golden;
use crate::adapter::hit_test::HitTestResult;
use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps};
use crate::commands::drag::{self, DragArgs, DragEndpoint, WaitForScope};
use crate::commands::hover::{self, HoverArgs};
use crate::commands::mouse_click::{self, MouseClickArgs};
use crate::commands::mouse_move::{self, MouseMoveArgs};
use crate::commands::mouse_wheel::{self, MouseWheelArgs};
use crate::context::CommandContext;
use crate::refs::refs_store::RefStore;
use crate::refs::refs_test_support::HomeGuard;
use crate::refs::{RefEntry, RefMap};
use crate::{
    AdapterError, Deadline, DragParams, InteractionLease, Modifier, MouseButton, MouseEvent,
    MouseEventKind, OperationReport, Point, Rect,
};
use serde_json::json;

struct PointerStub;

impl ObservationOps for PointerStub {
    fn resolve_element_strict(
        &self,
        e: &RefEntry,
        _: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::new(
            e.identity.name.clone().unwrap_or_default(),
        ))
    }

    fn get_element_bounds(
        &self,
        h: &NativeHandle,
        _: Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        let far = h
            .downcast_ref::<String>()
            .is_some_and(|name| name == "Destination");
        let (x, y) = if far { (400.0, 300.0) } else { (100.0, 200.0) };
        Ok(Some(Rect {
            x,
            y,
            width: 20.0,
            height: 10.0,
        }))
    }

    fn hit_test(
        &self,
        _: &NativeHandle,
        _: Point,
        _: Deadline,
    ) -> Result<HitTestResult, AdapterError> {
        Ok(HitTestResult::ReachesTarget)
    }

    crate::adapter::saved_ref_scope!(nodes: &[
        ("button", Some("Source"), None),
        ("button", Some("Destination"), None),
    ]);
}

impl ActionOps for PointerStub {}

impl InputOps for PointerStub {
    fn mouse_event(
        &self,
        event: MouseEvent,
        _: &InteractionLease,
    ) -> Result<OperationReport, AdapterError> {
        let moved = matches!(event.kind, MouseEventKind::Move);
        Ok(OperationReport::pointer_dispatched(&event, moved))
    }

    fn drag(
        &self,
        params: DragParams,
        _: &InteractionLease,
    ) -> Result<OperationReport, AdapterError> {
        Ok(OperationReport::drag_dispatched(&params))
    }
}

impl SystemOps for PointerStub {
    crate::adapter::guarded_interaction_lease!();
    crate::adapter::exact_window_focus!();
}

fn save_refs() -> String {
    let mut refmap = RefMap::new();
    for name in ["Source", "Destination"] {
        let mut entry = crate::adapter::minimal_ref_entry("button", Some(name));
        entry.process.pid = crate::ProcessId::new(42);
        entry.source.source_window_id = Some("w-42".into());
        refmap.allocate(entry);
    }
    RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap()
}

#[test]
fn hover_and_drag_by_ref_report_pointer_targets_and_pickup_focus() {
    let _home = HomeGuard::new();
    let snapshot = save_refs();
    let headed = CommandContext::default().with_headed(true);
    let endpoint = |n: u8| DragEndpoint {
        ref_id: Some(format!("@{snapshot}:e{n}")),
        xy: None,
    };
    let hover = HoverArgs {
        ref_id: endpoint(1).ref_id,
        xy: None,
        duration_ms: None,
        timeout_ms: None,
    };
    let drag = DragArgs {
        from: endpoint(1),
        to: endpoint(2),
        duration_ms: Some(300),
        drop_delay_ms: Some(750),
        timeout_ms: None,
        wait_for_scope: WaitForScope::default(),
    };

    let data = json!({
        "hover": hover::execute(hover, &PointerStub, &headed).unwrap(),
        "drag": drag::execute(drag, &PointerStub, &headed).unwrap(),
    });

    assert_golden(
        data,
        include_str!("../../../../../../tests/fixtures/contract/actions-pointer_ref.json"),
        &[],
    );
}

#[test]
fn mouse_click_move_and_wheel_echo_only_their_own_request_fields() {
    let headed = CommandContext::default().with_headed(true);
    let click = MouseClickArgs {
        x: 10.0,
        y: 20.0,
        button: MouseButton::Right,
        count: 2,
        modifiers: vec![Modifier::Shift],
    };
    let wheel = MouseWheelArgs {
        x: 50.0,
        y: 60.0,
        dy: -3.0,
        dx: 1.5,
        modifiers: Vec::new(),
    };

    let data = json!({
        "click": mouse_click::execute(click, &PointerStub, &headed).unwrap(),
        "move": mouse_move::execute(MouseMoveArgs { x: 5.5, y: 6.5 }, &PointerStub, &headed).unwrap(),
        "wheel": mouse_wheel::execute(wheel, &PointerStub, &headed).unwrap(),
    });

    assert_golden(
        data,
        include_str!("../../../../../../tests/fixtures/contract/actions-mouse_xy.json"),
        &[],
    );
}
