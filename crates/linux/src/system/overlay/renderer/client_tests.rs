use std::thread;

use super::*;

fn serve_once(stream: UnixStream, reply: Value) -> thread::JoinHandle<Value> {
    thread::spawn(move || {
        let request = wire::read_line(&stream).expect("request");
        wire::write_line(&stream, &reply).expect("reply");
        request
    })
}

#[test]
fn a_request_is_one_line_and_its_value_comes_back() {
    let (client, server) = UnixStream::pair().expect("pair");
    let served = serve_once(
        server,
        wire::success(json!({"x": 1.0, "y": 2.0, "moves": 3})),
    );
    let deadline = Deadline::after(2_000).expect("deadline");
    let value = exchange(&client, "locate", &json!({"session": "s1"}), deadline).expect("answered");
    assert_eq!(value, json!({"x": 1.0, "y": 2.0, "moves": 3}));
    assert_eq!(
        served.join().expect("served"),
        json!({"method": "locate", "body": {"session": "s1"}})
    );
}

#[test]
fn a_refused_request_is_invalid_arguments_and_a_failed_one_is_an_action_failure() {
    let deadline = Deadline::after(2_000).expect("deadline");
    let (client, server) = UnixStream::pair().expect("pair");
    let served = serve_once(server, wire::refusal("Invalid cursor request: size"));
    let error = exchange(&client, "update", &json!({}), deadline).unwrap_err();
    assert_eq!(
        (error.code, error.message.as_str()),
        (ErrorCode::InvalidArgs, "Invalid cursor request: size")
    );
    served.join().expect("served");
    let (client, server) = UnixStream::pair().expect("pair");
    let served = serve_once(
        server,
        json!({"ok": false, "invalid": false, "message": "broken"}),
    );
    assert_eq!(
        exchange(&client, "update", &json!({}), deadline)
            .unwrap_err()
            .code,
        ErrorCode::ActionFailed
    );
    served.join().expect("served");
}

#[test]
fn a_renderer_that_closes_without_answering_is_a_failure() {
    let (client, server) = UnixStream::pair().expect("pair");
    drop(server);
    let deadline = Deadline::after(2_000).expect("deadline");
    assert_eq!(
        exchange(&client, "update", &json!({}), deadline)
            .unwrap_err()
            .code,
        ErrorCode::ActionFailed
    );
}

#[test]
fn only_requests_that_draw_a_cursor_start_the_renderer() {
    for op in ["enable", "travel", "effect"] {
        assert!(starts_renderer(&json!({"op": op})), "{op}");
    }
    assert!(starts_renderer(&json!({"op": "caret", "follow": false})));
    for request in [
        json!({"op": "caret", "follow": true}),
        json!({"op": "hide"}),
        json!({"op": "show"}),
        json!({"op": "disable"}),
        json!({"op": "conceal"}),
        json!({"op": "reveal"}),
    ] {
        assert!(!starts_renderer(&request), "{request}");
    }
}
