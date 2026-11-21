use agent_desktop_core::{DeliverySemantics, ErrorCode};

use super::super::Request;
use super::super::fake::{Fake, dialog, window};
use super::close;
use crate::system::process::liveness::Liveness;
use crate::system::x11::control::ControlRead;

fn request() -> Request<'static> {
    Request {
        name: "Text Editor",
        pid: 42,
        instance: Some("t"),
        force: false,
    }
}

fn assert_no_signal(world: &Fake) {
    assert_eq!(world.count("term"), 0);
    assert_eq!(world.count("kill"), 0);
}

#[test]
fn an_open_dialog_is_reported_and_no_close_request_is_sent_to_anything() {
    let mut world = Fake::new(Liveness::Alive);
    world.windows = ControlRead::Ready(vec![window(1), dialog(2)]);
    world.exits_on_close = true;
    let error = close(&world, &request()).expect_err("a prompt is open");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    let details = error.details.expect("details");
    assert_eq!(details["cause"], "dialog_pending");
    assert_eq!(details["dialogs"], 1);
    assert_eq!(details["windows_remaining"], 1);
    assert_eq!(details["dialog"], "dialog 2");
    assert_eq!(world.count("close"), 0);
}

#[test]
fn a_dialog_that_opens_after_the_request_ends_the_wait_at_once_with_its_title() {
    let mut world = Fake::new(Liveness::Alive);
    world.dialog_after = Some(std::time::Duration::from_millis(100));
    let error = close(&world, &request()).expect_err("the save prompt appeared");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("details");
    assert_eq!(details["cause"], "dialog_pending");
    assert_eq!(details["dialog"], "dialog 99");
    assert_eq!(world.count("close"), 1);
    assert!(
        world.clock.get() < std::time::Duration::from_secs(1),
        "the wait did not run to its budget: {:?}",
        world.clock.get()
    );
}

#[test]
fn a_dialog_drawn_inside_a_window_is_reported_and_no_close_request_is_sent() {
    let mut world = Fake::new(Liveness::Alive);
    world.nested_after = Some(std::time::Duration::ZERO);
    world.exits_on_close = true;
    let error = close(&world, &request()).expect_err("a prompt is open");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    let details = error.details.expect("details");
    assert_eq!(details["cause"], "dialog_pending");
    assert_eq!(details["dialogs"], 1);
    assert_eq!(details["windows_remaining"], 1);
    assert_eq!(details["dialog"], "Save Changes?");
    assert_eq!(world.count("close"), 0);
}

#[test]
fn a_dialog_found_by_an_incomplete_scan_is_pending_not_unknown() {
    let mut world = Fake::new(Liveness::Alive);
    world.nested_after = Some(std::time::Duration::ZERO);
    world.nested_complete = false;
    world.exits_on_close = true;
    let error = close(&world, &request()).expect_err("a prompt is open");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    let details = error.details.expect("details");
    assert_eq!(details["cause"], "dialog_pending");
    assert_eq!(details["dialogs"], 1);
    assert_eq!(details["dialog"], "Save Changes?");
    assert_eq!(world.count("close"), 0);
    assert_no_signal(&world);
}

#[test]
fn an_unreadable_dialog_check_refuses_the_close_before_any_send_or_signal() {
    let mut world = Fake::new(Liveness::Alive);
    world.nested_complete = false;
    let error = close(&world, &request()).expect_err("the dialog state is unknown");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(error.details.expect("details")["cause"], "dialogs_unknown");
    assert_eq!(world.count("close"), 0);
    assert_no_signal(&world);
}

#[test]
fn a_dialog_scan_that_runs_out_the_deadline_is_a_timeout_before_any_send() {
    let mut world = Fake::new(Liveness::Alive);
    world.nested_times_out = true;
    let error = close(&world, &request()).expect_err("the deadline ran out");
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(world.count("close"), 0);
    assert_no_signal(&world);
}

#[test]
fn a_dialog_drawn_inside_a_window_after_the_request_ends_the_wait_at_once() {
    let mut world = Fake::new(Liveness::Alive);
    world.nested_after = Some(std::time::Duration::from_millis(100));
    let error = close(&world, &request()).expect_err("the save prompt appeared");
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("details");
    assert_eq!(details["cause"], "dialog_pending");
    assert_eq!(details["dialogs"], 1);
    assert_eq!(details["dialog"], "Save Changes?");
    assert_eq!(world.count("close"), 1);
    assert!(
        world.clock.get() < std::time::Duration::from_secs(1),
        "the wait did not run to its budget: {:?}",
        world.clock.get()
    );
}

#[test]
fn a_second_prompt_raised_by_the_same_request_is_counted_from_one_read() {
    let mut world = Fake::new(Liveness::Alive);
    world.dialog_after = Some(std::time::Duration::from_millis(100));
    world.second_dialog_after = Some(std::time::Duration::from_millis(125));
    let error = close(&world, &request()).expect_err("two save prompts opened");
    let details = error.details.expect("details");
    assert_eq!(details["cause"], "dialog_pending");
    assert_eq!(details["dialogs"], 2);
    assert_eq!(details["dialog_titles"].as_array().map(Vec::len), Some(2));
    assert_eq!(details["windows_remaining"], 1);
    assert!(
        error.message.contains("2 open dialog(s)"),
        "{}",
        error.message
    );
}

#[test]
fn a_prompt_answered_while_the_result_settles_is_an_exit_not_a_refusal() {
    let mut world = Fake::new(Liveness::Alive);
    world.dialog_after = Some(std::time::Duration::from_millis(100));
    world.gone_after = Some(std::time::Duration::from_millis(125));
    assert!(close(&world, &request()).expect("it left meanwhile"));
}

#[test]
fn a_process_that_left_while_a_dialog_was_read_is_ok() {
    let mut world = Fake::new(Liveness::Alive);
    world.windows = ControlRead::Ready(vec![window(1), dialog(2)]);
    world.state_after_toplevels = Some(Liveness::Gone);
    close(&world, &request()).expect("it left meanwhile");
}
