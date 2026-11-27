use agent_desktop_core::ErrorCode;

use super::StreamGeometry;

#[test]
fn translates_global_coordinates_using_nonzero_stream_origin() {
    let stream = StreamGeometry {
        node_id: 42,
        origin_x: 1920.0,
        origin_y: 120.0,
        width: Some(2560.0),
        height: Some(1440.0),
    };
    let (x, y) = stream.to_local(2120.0, 420.0).expect("within stream");
    assert_eq!((x, y), (200.0, 300.0));
}

#[test]
fn rejects_coordinates_outside_stream_bounds() {
    let stream = StreamGeometry {
        node_id: 42,
        origin_x: 1920.0,
        origin_y: 0.0,
        width: Some(1920.0),
        height: Some(1080.0),
    };
    let err = stream.to_local(500.0, 500.0).expect_err("left of monitor");
    assert_eq!(err.code, ErrorCode::ActionFailed);
    let err = stream
        .to_local(4000.0, 500.0)
        .expect_err("right of monitor");
    assert_eq!(err.code, ErrorCode::ActionFailed);
}
