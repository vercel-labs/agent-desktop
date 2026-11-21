use super::*;
use crate::{RendererKind, system::cdp_endpoint::CdpEndpoint};
use serde_json::json;

fn bare() -> LaunchResult {
    LaunchResult {
        app: "Fixture".into(),
        pid: ProcessId::new(42),
        process_instance: None,
        window: None,
        guidance: LaunchGuidance::default(),
        disposition: DeliverySemantics::not_delivered(),
    }
}

#[test]
fn absent_guidance_leaves_its_keys_out_of_the_wire() {
    let value = serde_json::to_value(bare()).unwrap();

    assert_eq!(
        value,
        json!({
            "app": "Fixture",
            "pid": 42,
            "disposition": { "delivery": "not_delivered", "retry": "safe" },
        })
    );
}

#[test]
fn present_guidance_sits_flat_beside_the_launch_keys() {
    let mut launched = bare();
    launched.guidance = LaunchGuidance {
        cdp: Some(CdpEndpoint {
            port: 9222,
            http_endpoint: "http://127.0.0.1:9222".into(),
            websocket_url: "ws://127.0.0.1:9222/devtools/browser/x".into(),
            product: None,
        }),
        renderer: Some(RendererKind::Chromium),
        suggestion: Some("guidance".into()),
    };
    launched.disposition = DeliverySemantics::delivered_verified();

    let value = serde_json::to_value(&launched).unwrap();

    assert_eq!(value["cdp"]["port"], 9222);
    assert_eq!(value["renderer"], "chromium");
    assert_eq!(value["suggestion"], "guidance");
    assert!(value.get("guidance").is_none());
    assert_eq!(
        value["disposition"],
        json!({ "delivery": "delivered_verified", "retry": "unsafe" })
    );
}

#[test]
fn a_serialized_launch_round_trips_through_the_flattened_guidance() {
    let mut launched = bare();
    launched.guidance.renderer = Some(RendererKind::Chromium);
    launched.guidance.suggestion = Some("guidance".into());
    launched.disposition = DeliverySemantics::delivered_unverified();

    let back: LaunchResult =
        serde_json::from_value(serde_json::to_value(&launched).unwrap()).unwrap();

    assert_eq!(back.guidance.renderer, Some(RendererKind::Chromium));
    assert_eq!(back.guidance.suggestion.as_deref(), Some("guidance"));
    assert!(back.guidance.cdp.is_none());
    assert_eq!(back.disposition, DeliverySemantics::delivered_unverified());
}

#[test]
fn a_payload_written_before_disposition_existed_deserializes_as_unknown() {
    let back: LaunchResult = serde_json::from_value(json!({
        "app": "Fixture",
        "pid": 42,
    }))
    .unwrap();

    assert_eq!(back.disposition, DeliverySemantics::unknown());
    assert_eq!(back.disposition.retry(), crate::RetryDisposition::Unknown);
}
