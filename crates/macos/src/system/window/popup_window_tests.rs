use super::holds_unlisted_surface;
use rustc_hash::FxHashSet;

#[test]
fn only_a_shown_window_the_accessibility_list_leaves_out_can_hold_a_surface() {
    let listed: FxHashSet<i64> = [7].into_iter().collect();
    assert!(
        holds_unlisted_surface(0, 1.0, 9, &listed),
        "a popover window"
    );
    assert!(
        holds_unlisted_surface(101, 1.0, 9, &listed),
        "a menu or picker"
    );
    assert!(
        !holds_unlisted_surface(0, 1.0, 7, &listed),
        "a listed window"
    );
    assert!(
        !holds_unlisted_surface(0, 0.0, 9, &listed),
        "a closed popover"
    );
    assert!(
        !holds_unlisted_surface(25, 1.0, 9, &listed),
        "a status item"
    );
    assert!(
        !holds_unlisted_surface(-2147483603, 1.0, 9, &listed),
        "the desktop"
    );
}
