use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use agent_desktop_core::{ErrorCode, Modifier, ProcessId, ProcessIdentity, WindowState};

use super::{
    ActionResult, AdapterError, CANCEL, Deadline, DeliverySemantics, InteractionLease,
    InteractionPolicy, KeyCombo, SystemOps, WindowInfo, WindowSurfaceClose,
    answer_with_surface_close, answers_with_surface_close, close_window_surface_with,
    delivery_result, is_focus_refusal, press_for_window, semantic_step,
};
use crate::system::process::process_identity::ProcessIdentity as NativeProcessIdentity;
use crate::tree::AXElement;

struct WindowRoute {
    focused: Option<WindowInfo>,
    dispatched: AtomicUsize,
}

impl SystemOps for WindowRoute {
    fn focused_window(&self, _deadline: Deadline) -> Result<Option<WindowInfo>, AdapterError> {
        Ok(self.focused.clone())
    }

    fn press_key_for_app(
        &self,
        _process: ProcessIdentity,
        _combo: &KeyCombo,
        _policy: InteractionPolicy,
        _lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.dispatched.fetch_add(1, Ordering::SeqCst);
        Ok(ActionResult::delivered_unverified("press_key"))
    }
}

fn window(id: &str) -> WindowInfo {
    WindowInfo {
        id: id.into(),
        title: format!("Window {id}"),
        app: "Fixture".into(),
        pid: ProcessId::new(std::process::id()),
        process_instance: Some("generation-1".into()),
        bounds: None,
        state: WindowState::default(),
    }
}

fn route(focused: &str) -> WindowRoute {
    WindowRoute {
        focused: Some(window(focused)),
        dispatched: AtomicUsize::new(0),
    }
}

fn key(name: &str, modifiers: Vec<Modifier>) -> KeyCombo {
    KeyCombo {
        key: name.into(),
        modifiers,
    }
}

fn lease() -> InteractionLease {
    InteractionLease::guarded(Deadline::standard().unwrap(), ()).unwrap()
}

fn focus_refusal(adapter: &WindowRoute) -> AdapterError {
    agent_desktop_core::press_key_in_focused_window(
        adapter,
        &window("w-1"),
        &key("a", Vec::new()),
        InteractionPolicy::headless(),
        &lease(),
    )
    .unwrap_err()
}

#[test]
fn a_window_that_holds_the_focus_takes_the_key_itself() {
    let adapter = route("w-1");
    let result = press_for_window(
        &adapter,
        &window("w-1"),
        &key("escape", Vec::new()),
        InteractionPolicy::headless(),
        &lease(),
    )
    .unwrap();

    assert_eq!(result.action, "press_key");
    assert_eq!(adapter.dispatched.load(Ordering::SeqCst), 1);
}

#[test]
fn only_a_plain_escape_answers_the_window_focus_refusal() {
    let refusal = focus_refusal(&route("w-2"));

    assert!(is_focus_refusal(&refusal));
    assert!(answers_with_surface_close(
        &key("escape", Vec::new()),
        &refusal
    ));
    assert!(answers_with_surface_close(
        &key("esc", Vec::new()),
        &refusal
    ));
    assert!(!answers_with_surface_close(&key("a", Vec::new()), &refusal));
    assert!(!answers_with_surface_close(
        &key("escape", vec![Modifier::Shift]),
        &refusal
    ));
}

#[test]
fn a_window_press_that_is_not_a_plain_escape_keeps_its_refusal() {
    let adapter = route("w-2");
    let error = press_for_window(
        &adapter,
        &window("w-1"),
        &key("a", Vec::new()),
        InteractionPolicy::headless(),
        &lease(),
    )
    .unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.details.unwrap()["check"], "window_active");
    assert_eq!(adapter.dispatched.load(Ordering::SeqCst), 0);
}

#[test]
fn a_closed_surface_answers_the_refused_window_escape() {
    let closed = delivery_result(semantic_step(CANCEL).with_verified(true));
    assert_eq!(
        closed.disposition(),
        DeliverySemantics::delivered_verified()
    );

    let answer =
        answer_with_surface_close(focus_refusal(&route("w-2")), || Ok(Some(closed))).unwrap();
    assert_eq!(
        answer.disposition(),
        DeliverySemantics::delivered_verified()
    );

    let unread = delivery_result(semantic_step(CANCEL).with_verified(false));
    let answer =
        answer_with_surface_close(focus_refusal(&route("w-2")), || Ok(Some(unread))).unwrap();
    assert_eq!(
        answer.disposition(),
        DeliverySemantics::delivered_unverified()
    );
}

#[test]
fn a_refused_window_escape_keeps_the_refusal_when_no_surface_is_open() {
    let answer = answer_with_surface_close(focus_refusal(&route("w-2")), || Ok(None)).unwrap_err();

    assert!(is_focus_refusal(&answer));
    assert_eq!(answer.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn a_surface_close_that_failed_keeps_its_own_error() {
    let answer = answer_with_surface_close(focus_refusal(&route("w-2")), || {
        Err(AdapterError::new(ErrorCode::ActionFailed, "cancel failed"))
    })
    .unwrap_err();

    assert_eq!(answer.message, "cancel failed");
}

#[derive(Default)]
struct SurfaceCalls {
    discovered: Cell<usize>,
    generations: Cell<usize>,
    cancelled: Cell<usize>,
}

struct SurfaceFixture {
    discovery: Result<Option<AXElement>, AdapterError>,
    surface_pid: Result<i32, i32>,
    fail_generation_at: Option<usize>,
    calls: SurfaceCalls,
}

impl SurfaceFixture {
    fn new(surface_pid: i32) -> Self {
        Self {
            discovery: Ok(Some(AXElement(std::ptr::null_mut()))),
            surface_pid: Ok(surface_pid),
            fail_generation_at: None,
            calls: SurfaceCalls::default(),
        }
    }

    fn with_surface_pid(mut self, surface_pid: Result<i32, i32>) -> Self {
        self.surface_pid = surface_pid;
        self
    }

    fn failing_generation_at(mut self, call: usize) -> Self {
        self.fail_generation_at = Some(call);
        self
    }
}

impl WindowSurfaceClose for SurfaceFixture {
    fn expected_identity(
        &self,
        window: &WindowInfo,
    ) -> Result<NativeProcessIdentity, AdapterError> {
        super::expected_window_identity(window)
    }

    fn discover(
        &self,
        _identity: NativeProcessIdentity,
        _window: &WindowInfo,
        _deadline: Deadline,
    ) -> Result<Option<AXElement>, AdapterError> {
        self.calls.discovered.set(self.calls.discovered.get() + 1);
        self.discovery.clone()
    }

    fn require_generation(&self, _identity: NativeProcessIdentity) -> Result<(), AdapterError> {
        let call = self.calls.generations.get() + 1;
        self.calls.generations.set(call);
        if self.fail_generation_at == Some(call) {
            return Err(AdapterError::stale_ref_because(
                "Application process instance changed during observation",
            ));
        }
        Ok(())
    }

    fn surface_pid(&self, _surface: &AXElement, _deadline: Deadline) -> Result<i32, i32> {
        self.surface_pid
    }

    fn cancel(&self, _surface: &AXElement, _deadline: Deadline) -> Result<bool, AdapterError> {
        self.calls.cancelled.set(self.calls.cancelled.get() + 1);
        Ok(true)
    }

    fn closed(&self, _surface: &AXElement, _deadline: Deadline) -> bool {
        true
    }
}

fn current_identity() -> NativeProcessIdentity {
    let pid = i32::try_from(std::process::id()).expect("test pid fits macOS pid_t");
    NativeProcessIdentity::capture(pid)
        .expect("capture current process identity")
        .expect("current process identity is present")
}

fn window_with_instance(instance: Option<&str>) -> WindowInfo {
    WindowInfo {
        process_instance: instance.map(str::to_owned),
        ..window("w-1")
    }
}

fn live_window() -> WindowInfo {
    let token = current_identity().token();
    window_with_instance(Some(&token))
}

fn standard_deadline() -> Deadline {
    Deadline::standard().unwrap()
}

#[test]
fn an_unprovable_window_identity_fails_before_discovery() {
    let cases = [
        (
            window_with_instance(None),
            ErrorCode::StaleRef,
            "Window has no process-instance identity",
        ),
        (
            window_with_instance(Some("")),
            ErrorCode::StaleRef,
            "Window has no process-instance identity",
        ),
        (
            window_with_instance(Some("macos-proc-v1:1:0")),
            ErrorCode::AppUnresponsive,
            "Target process instance is no longer running",
        ),
    ];
    for (window, code, message) in cases {
        let fixture = SurfaceFixture::new(0);
        let error = close_window_surface_with(&fixture, &window, standard_deadline())
            .expect_err("an unprovable identity must fail closed");

        assert_eq!(error.code, code);
        assert_eq!(error.message, message);
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
        assert_eq!(fixture.calls.discovered.get(), 0);
        assert_eq!(fixture.calls.cancelled.get(), 0);
    }
}

#[test]
fn a_stale_window_during_discovery_propagates_without_cancelling() {
    let identity = current_identity();
    let mut fixture = SurfaceFixture::new(identity.pid());
    fixture.discovery = Err(AdapterError::stale_ref_because(
        "Window 'w-1' identity mismatch",
    ));
    let error = close_window_surface_with(&fixture, &live_window(), standard_deadline())
        .expect_err("a stale window must not read as an absent surface");

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(fixture.calls.discovered.get(), 1);
    assert_eq!(fixture.calls.cancelled.get(), 0);
}

#[test]
fn a_generation_change_after_discovery_stops_the_cancel() {
    for call in 1..=2 {
        let identity = current_identity();
        let fixture = SurfaceFixture::new(identity.pid()).failing_generation_at(call);
        let error = close_window_surface_with(&fixture, &live_window(), standard_deadline())
            .expect_err("a replaced process must stop the cancel");

        assert_eq!(error.code, ErrorCode::StaleRef);
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
        assert_eq!(fixture.calls.discovered.get(), 1);
        assert_eq!(fixture.calls.generations.get(), call);
        assert_eq!(fixture.calls.cancelled.get(), 0);
    }
}

#[test]
fn a_surface_owned_by_another_process_is_never_cancelled() {
    let identity = current_identity();
    let fixture = SurfaceFixture::new(identity.pid() + 1);
    let error = close_window_surface_with(&fixture, &live_window(), standard_deadline())
        .expect_err("a foreign surface must fail closed");

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(
        error.message,
        "Surface no longer belongs to the target process"
    );
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(fixture.calls.cancelled.get(), 0);
}

#[test]
fn an_invalid_surface_before_the_cancel_fails_closed() {
    let identity = current_identity();
    let fixture = SurfaceFixture::new(identity.pid())
        .with_surface_pid(Err(accessibility_sys::kAXErrorInvalidUIElement));
    let error = close_window_surface_with(&fixture, &live_window(), standard_deadline())
        .expect_err("an invalid surface must fail closed");

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(error.message, "Surface became invalid before cancel");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(fixture.calls.cancelled.get(), 0);
}

#[test]
fn another_surface_read_failure_keeps_its_mapped_code() {
    let identity = current_identity();
    let fixture = SurfaceFixture::new(identity.pid())
        .with_surface_pid(Err(accessibility_sys::kAXErrorCannotComplete));
    let error = close_window_surface_with(&fixture, &live_window(), standard_deadline())
        .expect_err("a failed surface read must fail closed");

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.details.as_ref().unwrap()["attribute"], "AXPid");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(fixture.calls.cancelled.get(), 0);
}

#[test]
fn an_original_generation_surface_is_cancelled_and_verified() {
    let identity = current_identity();
    let fixture = SurfaceFixture::new(identity.pid());
    let closed = close_window_surface_with(&fixture, &live_window(), standard_deadline())
        .unwrap()
        .expect("a surface of the original generation closes");

    assert_eq!(closed.action, "press_key");
    assert_eq!(
        closed.disposition(),
        DeliverySemantics::delivered_verified()
    );
    assert_eq!(fixture.calls.generations.get(), 2);
    assert_eq!(fixture.calls.cancelled.get(), 1);
}
