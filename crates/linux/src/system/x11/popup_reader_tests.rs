use super::{PopupWindow, matching_popup, unique_popup};
use crate::tree::Extents;

fn bounds(width: i32, height: i32) -> Extents {
    Extents {
        x: 0,
        y: 0,
        width,
        height,
    }
}

#[test]
fn gtk3_menu_items_match_their_popup_origin_instead_of_the_bar_label() {
    let popup = PopupWindow {
        id: 7,
        bounds: Extents {
            x: 479,
            y: 185,
            width: 118,
            height: 54,
        },
    };
    let screen = Extents {
        x: 479,
        y: 212,
        width: 118,
        height: 27,
    };
    let local = Extents {
        x: 0,
        y: 27,
        width: 118,
        height: 27,
    };
    let decoy = PopupWindow {
        id: 8,
        bounds: Extents {
            x: 251,
            y: 51,
            ..popup.bounds
        },
    };
    assert_eq!(matching_popup(&[decoy, popup], screen, local), Some(popup));
    assert_eq!(matching_popup(&[decoy], screen, local), None);
    assert_eq!(matching_popup(&[popup, popup], screen, local), None);
    for width in [-1, 0, 117] {
        assert_eq!(
            matching_popup(&[popup], screen, Extents { width, ..local }),
            None
        );
    }
    assert_eq!(
        matching_popup(&[popup], screen, Extents { y: 60, ..local }),
        None
    );
}

#[test]
fn a_popup_is_aimable_only_when_its_override_redirect_origin_is_unique() {
    let one = PopupWindow {
        id: 7,
        bounds: bounds(280, 190),
    };
    let other = PopupWindow {
        id: 8,
        bounds: bounds(280, 190),
    };
    assert_eq!(unique_popup(&[one]), Some(one));
    assert_eq!(unique_popup(&[]), None);
    assert_eq!(unique_popup(&[one, other]), None);
}

#[test]
fn a_silent_popup_setup_releases_its_socket_and_reader() {
    use std::io::Read;
    use std::net::TcpListener;
    use std::sync::mpsc::channel;
    use std::time::Duration;

    use agent_desktop_core::Deadline;

    use super::super::bounded::start;

    let listener = TcpListener::bind("127.0.0.1:0").expect("private listener");
    let port = listener.local_addr().expect("address").port();
    let display = format!("127.0.0.1:{}", port - 6000);
    let (setup, received) = channel();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("popup connection");
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("read timeout");
        let mut header = [0; 12];
        socket.read_exact(&mut header).expect("setup request");
        setup.send(()).expect("setup observed");
        let mut bytes = [0; 1024];
        loop {
            match socket.read(&mut bytes) {
                Ok(0) => return true,
                Ok(_) => {}
                Err(_) => return false,
            }
        }
    });
    let (ended, retired) = channel();
    let pending = start(move || {
        let answer = super::read_popup_window(&display, 42, (280, 190));
        let _ = ended.send(());
        answer
    });
    received
        .recv_timeout(Duration::from_secs(5))
        .expect("popup setup started");
    let answer = pending.finish(Deadline::after(1).expect("deadline"));
    let closed = server.join().expect("silent server exited");
    retired
        .recv_timeout(Duration::from_secs(5))
        .expect("popup reader retired");
    assert_eq!(answer, None);
    assert!(closed, "popup timeout must close its setup socket");
    assert_eq!(
        start(|| 42).finish(Deadline::after(5_000).expect("deadline")),
        Some(42)
    );
}
