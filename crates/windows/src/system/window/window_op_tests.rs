use super::*;

use agent_desktop_core::ProcessId;

fn window_with_id(id: &str, process_instance: Option<&str>) -> WindowInfo {
    WindowInfo {
        id: id.into(),
        title: "x".into(),
        app: "x".into(),
        pid: ProcessId::new(std::process::id()),
        process_instance: process_instance.map(str::to_string),
        bounds: None,
        state: Default::default(),
    }
}

#[test]
fn geometry_rejects_non_finite_and_extreme_values() {
    assert!(validate_size(f64::NAN, 100.0).is_err());
    assert!(validate_size(100.0, f64::INFINITY).is_err());
    assert!(validate_size(0.0, 100.0).is_err());
    assert!(validate_size(-10.0, 100.0).is_err());
    assert!(validate_size(1_000_001.0, 100.0).is_err());
    assert!(validate_position(f64::NEG_INFINITY, 0.0).is_err());
    assert!(validate_position(1_000_001.0, 0.0).is_err());
}

#[test]
fn geometry_accepts_negative_screen_coordinates() {
    assert!(validate_position(-1920.0, 0.0).is_ok());
    assert!(validate_position(0.0, -1080.0).is_ok());
    assert!(validate_size(1920.0, 1080.0).is_ok());
}

#[test]
fn an_expired_deadline_is_refused_before_the_window_id_is_read() {
    let win = window_with_id("not-a-window", None);
    let expired = Deadline::after(0).expect("a zero millisecond deadline");

    let error = window_op_impl(&win, WindowOp::Minimize, expired)
        .expect_err("an expired deadline is refused before anything is read");

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn an_id_that_is_not_a_window_handle_is_invalid_and_not_delivered() {
    let win = window_with_id("not-a-window", Some("windows-proc-v1:0:1"));
    let live = Deadline::after(5_000).expect("a five second deadline");

    let error = window_op_impl(&win, WindowOp::Minimize, live)
        .expect_err("an id with no HWND shape cannot name a window");

    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn a_null_window_handle_is_invalid_and_not_delivered() {
    let win = window_with_id("w-0", Some("windows-proc-v1:0:1"));
    let live = Deadline::after(5_000).expect("a five second deadline");

    let error = window_op_impl(&win, WindowOp::Minimize, live)
        .expect_err("the null handle names no window");

    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn a_stored_window_without_a_process_token_is_stale_and_not_delivered() {
    let win = window_with_id("w-4660", None);
    let live = Deadline::after(5_000).expect("a five second deadline");

    let error = window_op_impl(&win, WindowOp::Minimize, live)
        .expect_err("a window with no process instance token cannot be corroborated");

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn geometry_that_cannot_be_planned_is_invalid_before_any_write() {
    let ops = [
        WindowOp::Resize {
            width: f64::NAN,
            height: 100.0,
        },
        WindowOp::Resize {
            width: 0.0,
            height: 100.0,
        },
        WindowOp::Move {
            x: f64::INFINITY,
            y: 0.0,
        },
    ];

    for op in ops {
        let Err(error) = Placement::plan(&op) else {
            panic!("{op:?} must not produce a placement");
        };
        assert_eq!(error.code, ErrorCode::InvalidArgs);
        assert_eq!(
            before_write(error).disposition,
            DeliverySemantics::not_delivered()
        );
    }
}

#[cfg(target_os = "windows")]
#[test]
fn a_handle_that_names_no_window_of_the_recorded_process_fails_closed_before_any_write() {
    let win = window_with_id("w-2147483632", Some("not-the-running-instance"));
    let live = Deadline::after(5_000).expect("a five second deadline");

    let error = window_op_impl(&win, WindowOp::Minimize, live)
        .expect_err("a handle that names no window cannot pass identity verification");

    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}
