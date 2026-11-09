use super::*;

#[test]
fn geometry_rejects_non_finite_and_extreme_values() {
    assert!(validate_size(f64::NAN, 100.0).is_err());
    assert!(validate_size(100.0, f64::INFINITY).is_err());
    assert!(validate_size(0.0, 100.0).is_err());
    assert!(validate_size(-10.0, 100.0).is_err());
    assert!(validate_size(1_000_001.0, 100.0).is_err());
    assert!(validate_position(f64::NEG_INFINITY, 0.0).is_err());
    assert!(validate_position(1_000_001.0, 0.0).is_err());
}

#[test]
fn geometry_accepts_negative_screen_coordinates() {
    assert!(validate_position(-1920.0, 0.0).is_ok());
    assert!(validate_position(0.0, -1080.0).is_ok());
    assert!(validate_size(1920.0, 1080.0).is_ok());
}
