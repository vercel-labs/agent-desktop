use super::{hwnd_of, surfaces_from};

#[test]
fn an_element_shows_a_surface_only_through_its_expand_collapse_state() {
    assert_eq!(surfaces_from(None, Some(1)), None);
    assert_eq!(surfaces_from(Some(false), Some(1)), None);
    assert_eq!(surfaces_from(Some(true), Some(1)), Some(1));
    assert_eq!(surfaces_from(Some(true), Some(0)), Some(0));
    assert_eq!(surfaces_from(Some(true), None), None);
}

#[test]
fn only_a_native_window_runtime_id_names_a_window_handle() {
    assert_eq!(hwnd_of(&[42, 7_340_032, 1]), Some(7_340_032));
    assert_eq!(hwnd_of(&[7, 3]), None);
    assert_eq!(hwnd_of(&[]), None);
}
