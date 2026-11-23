use super::*;

fn rect(x: i32, y: i32, width: i32, height: i32) -> Extents {
    Extents {
        x,
        y,
        width,
        height,
    }
}

fn window(pid: u32, title: &str, frame: Extents, buffer: Extents) -> ShellWindow {
    ShellWindow {
        pid,
        title: title.to_owned(),
        wayland: true,
        minimized: false,
        frame,
        buffer,
        protocol: None,
    }
}

fn gtk4_editor() -> ShellWindow {
    window(
        40,
        "Notes",
        rect(311, 187, 420, 300),
        rect(297, 175, 448, 329),
    )
}

fn gtk3_editor() -> ShellWindow {
    window(
        41,
        "Notes",
        rect(311, 187, 420, 337),
        rect(285, 164, 472, 389),
    )
}

#[test]
fn a_frame_the_size_of_the_visible_window_starts_at_the_frame_rectangle() {
    let windows = [gtk4_editor()];
    assert_eq!(
        matching_window(&windows, 40, "Notes", (420, 300)).map(|(_, space)| space),
        Some(rect(311, 187, 420, 300))
    );
}

#[test]
fn a_frame_the_size_of_the_whole_surface_starts_at_the_buffer_rectangle() {
    let windows = [gtk3_editor()];
    assert_eq!(
        matching_window(&windows, 41, "Notes", (472, 389)).map(|(_, space)| space),
        Some(rect(285, 164, 472, 389))
    );
}

#[test]
fn a_frame_matching_neither_rectangle_is_not_placed() {
    let windows = [gtk3_editor()];
    assert_eq!(matching_window(&windows, 41, "Notes", (420, 300)), None);
}

#[test]
fn two_windows_of_one_process_with_the_same_title_and_size_place_neither() {
    let twin = window(
        40,
        "Notes",
        rect(900, 100, 420, 300),
        rect(886, 88, 448, 329),
    );
    let windows = [gtk4_editor(), twin];
    assert_eq!(matching_window(&windows, 40, "Notes", (420, 300)), None);
}

#[test]
fn the_title_tells_two_windows_of_one_process_apart() {
    let other = window(
        40,
        "Draft",
        rect(900, 100, 420, 300),
        rect(886, 88, 448, 329),
    );
    let windows = [gtk4_editor(), other];
    assert_eq!(
        matching_window(&windows, 40, "Draft", (420, 300)).map(|(_, space)| space),
        Some(rect(900, 100, 420, 300))
    );
}

#[test]
fn another_process_window_with_the_same_title_and_size_is_never_used() {
    let windows = [gtk4_editor()];
    assert_eq!(matching_window(&windows, 99, "Notes", (420, 300)), None);
}

#[test]
fn an_xwayland_window_is_left_to_the_toolkits_own_global_position() {
    let mut xwayland = gtk4_editor();
    xwayland.wayland = false;
    assert_eq!(matching_window(&[xwayland], 40, "Notes", (420, 300)), None);
}

#[test]
fn a_minimized_window_is_not_placed() {
    let mut minimized = gtk4_editor();
    minimized.minimized = true;
    assert_eq!(matching_window(&[minimized], 40, "Notes", (420, 300)), None);
}

#[test]
fn a_minimized_twin_of_the_same_title_and_size_places_neither_window() {
    let mut minimized = gtk4_editor();
    minimized.minimized = true;
    let mut visible = gtk4_editor();
    visible.frame = rect(600, 120, 420, 300);
    visible.buffer = rect(586, 108, 448, 329);
    assert_eq!(
        matching_window(&[minimized, visible], 40, "Notes", (420, 300)),
        None
    );
}

#[test]
fn an_empty_frame_size_is_not_placed() {
    let windows = [window(40, "Notes", rect(0, 0, 0, 0), rect(0, 0, 0, 0))];
    assert_eq!(matching_window(&windows, 40, "Notes", (0, 0)), None);
}

fn xwayland(pid: u32, title: &str, protocol: Extents, frame: Extents) -> ShellWindow {
    ShellWindow {
        wayland: false,
        protocol: Some(protocol),
        ..window(pid, title, frame, frame)
    }
}

#[test]
fn an_xwayland_process_maps_its_x11_coordinates_by_its_windows_frames() {
    let windows = [
        gtk4_editor(),
        xwayland(
            52,
            "Probe",
            rect(774, 504, 500, 338),
            rect(387, 252, 250, 169),
        ),
        xwayland(52, "", rect(900, 600, 200, 100), rect(450, 300, 100, 50)),
    ];
    let map = xwayland_map(&windows, 52).expect("map");
    assert_eq!(
        map.to_stage(rect(854, 582, 420, 34)),
        Some(rect(427, 291, 210, 17))
    );
}

#[test]
fn a_native_wayland_process_has_no_xwayland_map() {
    let mut mixed = gtk4_editor();
    mixed.pid = 52;
    let windows = [
        mixed,
        xwayland(
            52,
            "Probe",
            rect(774, 504, 500, 338),
            rect(387, 252, 250, 169),
        ),
    ];
    assert_eq!(xwayland_map(&windows, 52), None);
    assert_eq!(xwayland_map(&[gtk4_editor()], 40), None);
    assert_eq!(xwayland_map(&windows, 99), None);
}

#[test]
fn xwayland_windows_that_disagree_or_lack_a_protocol_frame_give_no_map() {
    let disagreeing = [
        xwayland(
            52,
            "Probe",
            rect(774, 504, 500, 338),
            rect(387, 252, 250, 169),
        ),
        xwayland(
            52,
            "Other",
            rect(100, 100, 200, 100),
            rect(100, 100, 200, 100),
        ),
    ];
    assert_eq!(xwayland_map(&disagreeing, 52), None);
    let mut unreported = xwayland(52, "Probe", rect(0, 0, 1, 1), rect(387, 252, 250, 169));
    unreported.protocol = None;
    assert_eq!(xwayland_map(&[unreported], 52), None);
}

#[test]
fn a_titled_xwayland_window_with_a_minimized_twin_is_not_chosen() {
    let visible = xwayland(
        52,
        "Probe",
        rect(774, 504, 500, 338),
        rect(387, 252, 250, 169),
    );
    let mut minimized = visible.clone();
    minimized.minimized = true;
    assert_eq!(
        titled_xwayland(std::slice::from_ref(&visible), 52, "Probe"),
        Some(&visible)
    );
    assert_eq!(titled_xwayland(&[minimized.clone()], 52, "Probe"), None);
    assert_eq!(titled_xwayland(&[minimized, visible], 52, "Probe"), None);
}
