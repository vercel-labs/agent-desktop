use super::*;

fn surface(kind: &str, title: Option<&str>) -> SurfaceInfo {
    SurfaceInfo {
        id: "focused-window".into(),
        kind: kind.into(),
        title: title.map(str::to_owned),
        item_count: None,
        unclassified: Vec::new(),
    }
}

#[test]
fn only_a_sheet_or_an_alert_holds_a_close_request() {
    let surfaces = [
        surface("menu", None),
        surface("popover", Some("Info")),
        surface("sheet", Some("Save")),
    ];

    assert_eq!(
        pending_modal(&surfaces).map(|s| s.kind.as_str()),
        Some("sheet")
    );
    assert!(pending_modal(&surfaces[..2]).is_none());
    assert!(pending_modal(&[]).is_none());
}

#[test]
fn a_prompt_seen_before_the_request_means_nothing_was_sent() {
    let error = modal_pending("TextEdit", 42, &surface("alert", Some("Not opened")), false);

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(error.message.contains("alert 'Not opened'"));
    let details = error.details.expect("details");
    assert_eq!(details["cause"], "modal_pending");
    assert_eq!(details["pid"], 42);
    assert!(
        error
            .suggestion
            .as_deref()
            .is_some_and(|text| text.contains("--force"))
    );
}

#[test]
fn a_prompt_the_request_raised_is_delivered_but_unverified() {
    let error = modal_pending("TextEdit", 42, &surface("sheet", None), true);

    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert!(error.message.contains("open sheet"));
}
