use super::*;
use crate::system::process::process_facts::ProcessFacts;
use crate::system::process::toolkit_id::ToolkitId;
use crate::system::registry::registry_app::assemble;
use crate::system::window::window_listing::collect;
use crate::tree::live::test_support::{read_of, words};
use crate::tree::{AtspiElement, Field};

fn rect(x: i32, y: i32, width: i32, height: i32) -> Extents {
    Extents {
        x,
        y,
        width,
        height,
    }
}

fn listed(titles: &[&str]) -> Vec<ListedWindow> {
    let app = assemble(
        AtspiElement::new(":1.9", "/org/a11y/atspi/accessible/root", 321),
        ProcessFacts {
            executable: None,
            comm: "x".to_owned(),
            exe_basename: Some("x".to_owned()),
            instance: "boot:42".to_owned(),
        },
        Some(Some("App".to_owned())),
        ToolkitId {
            name: Some("GTK".to_owned()),
            version: Some("3.24.41".to_owned()),
        },
    );
    let nodes = titles
        .iter()
        .enumerate()
        .map(|(index, title)| {
            let mut node = read_of(23, title, &[]).node;
            node.element = AtspiElement::new(":1.9", format!("/frame/{index}"), 321);
            node.traits.interfaces =
                Field::Present(vec![crate::tree::COMPONENT_INTERFACE.to_owned()]);
            node.traits.states = Field::Present(words(&[25, 1]));
            node.extents = Field::Present(rect(0, 0, 472, 389));
            node
        })
        .collect();
    collect(&[(0, &app)], &[nodes])
}

fn shell(title: &str, x: i32) -> ShellWindow {
    ShellWindow {
        pid: 321,
        title: title.to_owned(),
        wayland: true,
        minimized: false,
        frame: rect(x, 187, 420, 337),
        buffer: rect(x - 26, 164, 472, 389),
        protocol: None,
    }
}

#[test]
fn a_native_wayland_window_takes_the_visible_frame_the_shell_reports() {
    let mut windows = listed(&["Doc"]);
    place_shell_windows(&mut windows, &[shell("Doc", 311)]);
    assert_eq!(
        windows[0].info.bounds,
        Some(Rect {
            x: 311.0,
            y: 187.0,
            width: 420.0,
            height: 337.0
        })
    );
}

#[test]
fn windows_the_shell_cannot_tell_apart_keep_their_bounds() {
    let mut windows = listed(&["Doc", "Doc"]);
    let before: Vec<_> = windows.iter().map(|window| window.info.bounds).collect();
    place_shell_windows(&mut windows, &[shell("Doc", 311), shell("Doc", 900)]);
    let after: Vec<_> = windows.iter().map(|window| window.info.bounds).collect();
    assert_eq!(before, after);
}

#[test]
fn a_window_the_shell_does_not_list_keeps_its_bounds() {
    let mut windows = listed(&["Doc"]);
    let before = windows[0].info.bounds;
    place_shell_windows(&mut windows, &[shell("Other", 311)]);
    assert_eq!(windows[0].info.bounds, before);
}

#[test]
fn an_untitled_xwayland_window_moves_from_x11_coordinates_onto_the_stage() {
    let mut windows = listed(&["Doc"]);
    let xwayland = ShellWindow {
        wayland: false,
        frame: rect(387, 252, 236, 195),
        buffer: rect(387, 252, 236, 195),
        protocol: Some(rect(774, 504, 472, 389)),
        ..shell("Main", 0)
    };
    windows[0].frame.extents = Some(rect(774, 504, 472, 389));
    place_shell_windows(&mut windows, &[xwayland]);
    assert_eq!(
        windows[0].info.bounds,
        Some(Rect {
            x: 387.0,
            y: 252.0,
            width: 236.0,
            height: 195.0
        })
    );
}

#[test]
fn an_xwayland_window_its_title_names_takes_the_visible_frame() {
    let mut windows = listed(&["Doc"]);
    let xwayland = ShellWindow {
        wayland: false,
        frame: rect(311, 187, 483, 575),
        buffer: rect(300, 177, 505, 598),
        protocol: Some(rect(622, 374, 966, 1150)),
        ..shell("Doc", 0)
    };
    windows[0].frame.extents = Some(rect(600, 354, 1010, 1196));
    place_shell_windows(&mut windows, std::slice::from_ref(&xwayland));
    assert_eq!(
        windows[0].info.bounds,
        Some(Rect {
            x: 311.0,
            y: 187.0,
            width: 483.0,
            height: 575.0
        })
    );
    let mut untitled = listed(&["Popup"]);
    untitled[0].frame.extents = Some(rect(600, 354, 1010, 1196));
    place_shell_windows(&mut untitled, &[xwayland]);
    assert_eq!(
        untitled[0].info.bounds,
        Some(Rect {
            x: 300.0,
            y: 177.0,
            width: 505.0,
            height: 598.0
        })
    );
}
