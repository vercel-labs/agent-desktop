use super::parse_outputs;

#[test]
fn parses_single_kde_output() {
    let raw = r#"{
        "outputs": [
            {
                "name": "eDP-1",
                "connected": true,
                "enabled": true,
                "priority": 1,
                "scale": 1.0,
                "rotation": 1,
                "pos": { "x": 0, "y": 0 },
                "size": { "width": 1920, "height": 1080 }
            }
        ]
    }"#;
    let monitors = parse_outputs(raw).expect("single output");
    assert_eq!(monitors.len(), 1);
    assert_eq!(monitors[0].connectors, vec!["eDP-1"]);
    assert_eq!(monitors[0].width, 1920);
    assert_eq!(monitors[0].height, 1080);
    assert_eq!(monitors[0].scale, 1.0);
    assert!(monitors[0].primary);
}

#[test]
fn applies_scale_preserves_rotated_size_and_filters_disabled_outputs() {
    let raw = r#"{
        "outputs": [
            {
                "name": "HDMI-1",
                "connected": true,
                "enabled": false,
                "priority": 0,
                "scale": 1.0,
                "rotation": 1,
                "pos": { "x": 0, "y": 0 },
                "size": { "width": 1920, "height": 1080 }
            },
            {
                "name": "DP-1",
                "connected": true,
                "enabled": true,
                "priority": 2,
                "scale": 1.5,
                "rotation": 2,
                "pos": { "x": 1920, "y": 0 },
                "size": { "width": 1500, "height": 3000 }
            },
            {
                "name": "eDP-1",
                "connected": true,
                "enabled": true,
                "priority": 1,
                "scale": 1.0,
                "rotation": 1,
                "pos": { "x": 0, "y": 0 },
                "size": { "width": 1920, "height": 1080 }
            }
        ]
    }"#;
    let monitors = parse_outputs(raw).expect("two enabled outputs");
    assert_eq!(monitors.len(), 2);
    assert_eq!(monitors[0].connectors, vec!["DP-1"]);
    assert_eq!(monitors[0].width, 1000);
    assert_eq!(monitors[0].height, 2000);
    assert!(!monitors[0].primary);
    assert_eq!(monitors[1].connectors, vec!["eDP-1"]);
    assert!(monitors[1].primary);
}

#[test]
fn returns_none_on_invalid_or_empty_json() {
    assert!(parse_outputs("not json").is_none());
    assert!(parse_outputs(r#"{"outputs":[]}"#).is_none());
}
