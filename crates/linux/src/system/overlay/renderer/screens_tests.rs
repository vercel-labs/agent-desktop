use super::*;

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn a_point_off_every_monitor_falls_back_to_the_primary_one() {
    let screens = Screens::new(
        vec![
            (rect(0.0, 0.0, 800.0, 600.0), false),
            (rect(800.0, 0.0, 1024.0, 768.0), true),
        ],
        None,
    );
    let root = rect(0.0, 0.0, 1824.0, 768.0);
    assert_eq!(
        screens.monitor_at(&Point { x: 100.0, y: 100.0 }, &root),
        rect(0.0, 0.0, 800.0, 600.0)
    );
    assert_eq!(
        screens.monitor_at(&Point { x: 100.0, y: 700.0 }, &root),
        rect(800.0, 0.0, 1024.0, 768.0)
    );
}

#[test]
fn without_randr_monitors_the_root_window_is_the_screen() {
    let screens = Screens::default();
    let root = rect(0.0, 0.0, 1280.0, 1024.0);
    assert_eq!(screens.monitor_at(&Point { x: 5.0, y: 5.0 }, &root), root);
}

#[test]
fn the_work_area_is_cut_to_the_monitor_that_holds_the_point() {
    let screens = Screens::new(
        vec![
            (rect(0.0, 0.0, 800.0, 600.0), true),
            (rect(800.0, 0.0, 800.0, 600.0), false),
        ],
        Some(rect(0.0, 32.0, 1600.0, 568.0)),
    );
    let root = rect(0.0, 0.0, 1600.0, 600.0);
    assert_eq!(
        screens.work_area_at(&Point { x: 900.0, y: 300.0 }, &root),
        rect(800.0, 32.0, 800.0, 568.0)
    );
}
