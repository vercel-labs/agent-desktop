use super::*;

#[test]
fn canonical_json_roundtrips() {
    for semantics in [
        DeliverySemantics::unknown(),
        DeliverySemantics::not_delivered(),
        DeliverySemantics::uncertain(),
        DeliverySemantics::delivered_unverified(),
        DeliverySemantics::delivered_verified(),
    ] {
        let value = serde_json::to_value(semantics).unwrap();
        assert_eq!(
            serde_json::from_value::<DeliverySemantics>(value).unwrap(),
            semantics
        );
    }
}

#[test]
fn impossible_retry_safe_delivery_is_rejected() {
    for disposition in [
        serde_json::json!({
            "delivery": "delivered_unverified",
            "retry": "safe",
        }),
        serde_json::json!({
            "delivery": "delivered_verified",
            "retry": "safe",
        }),
    ] {
        let error = serde_json::from_value::<DeliverySemantics>(disposition).unwrap_err();
        assert!(error.to_string().contains("inconsistent"));
    }
}

#[test]
fn merged_reports_not_delivered_when_nothing_was_sent() {
    assert_eq!(
        DeliverySemantics::merged(0, &[DeliverySemantics::not_delivered()]),
        DeliverySemantics::not_delivered()
    );
    assert_eq!(
        DeliverySemantics::merged(0, &[]),
        DeliverySemantics::not_delivered()
    );
}

#[test]
fn merged_reports_unverified_when_any_failure_may_have_been_sent() {
    for failure in [
        DeliverySemantics::unknown(),
        DeliverySemantics::uncertain(),
        DeliverySemantics::delivered_unverified(),
        DeliverySemantics::delivered_verified(),
    ] {
        assert_eq!(
            DeliverySemantics::merged(0, &[failure]),
            DeliverySemantics::delivered_unverified()
        );
        assert_eq!(
            DeliverySemantics::merged(
                1,
                &[
                    DeliverySemantics::not_delivered(),
                    DeliverySemantics::not_delivered(),
                    failure,
                ]
            ),
            DeliverySemantics::delivered_unverified()
        );
    }
}

#[test]
fn merged_reports_verified_when_every_entry_was_dismissed() {
    assert_eq!(
        DeliverySemantics::merged(2, &[]),
        DeliverySemantics::delivered_verified()
    );
}

#[test]
fn merged_reports_verified_for_partial_batch_with_unsent_failures() {
    let merged = DeliverySemantics::merged(
        1,
        &[
            DeliverySemantics::not_delivered(),
            DeliverySemantics::not_delivered(),
        ],
    );
    assert_eq!(merged, DeliverySemantics::delivered_verified());
    assert_eq!(
        merged.retry(),
        crate::RetryDisposition::Unsafe,
        "a re-run captures a fresh listing and can dismiss new arrivals"
    );
}
