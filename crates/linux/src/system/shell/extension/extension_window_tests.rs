use super::*;

#[test]
fn a_listed_window_keeps_its_process_title_client_kind_and_both_rectangles() {
    let windows = parse_windows(
        r#"[{"pid":7,"title":"Calculator","wayland":true,"minimized":false,
            "frame":{"x":460,"y":108,"width":360,"height":616},
            "buffer":{"x":435,"y":83,"width":410,"height":666}}]"#,
    )
    .expect("well-formed list");
    assert_eq!(
        windows,
        vec![ShellWindow {
            pid: 7,
            title: "Calculator".to_owned(),
            wayland: true,
            minimized: false,
            frame: Extents {
                x: 460,
                y: 108,
                width: 360,
                height: 616
            },
            buffer: Extents {
                x: 435,
                y: 83,
                width: 410,
                height: 666
            },
        }]
    );
}

#[test]
fn a_list_missing_a_rectangle_is_refused_rather_than_guessed() {
    let error = parse_windows(r#"[{"pid":7,"title":"","wayland":true,"minimized":false}]"#)
        .expect_err("incomplete window");
    assert_eq!(error.code, agent_desktop_core::ErrorCode::Internal);
}
