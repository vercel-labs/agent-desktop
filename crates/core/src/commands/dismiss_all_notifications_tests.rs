use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{AdapterError, DeliverySemantics, ErrorCode, InteractionLease, NotificationInfo};
use serde_json::json;

struct DismissAllAdapter {
    dismissed: usize,
    failures: Vec<AdapterError>,
}

impl ObservationOps for DismissAllAdapter {}
impl ActionOps for DismissAllAdapter {}
impl InputOps for DismissAllAdapter {}

impl SystemOps for DismissAllAdapter {
    crate::adapter::guarded_interaction_lease!();

    fn dismiss_all_notifications(
        &self,
        _request: crate::DismissAllNotificationsRequest<'_>,
        _lease: &InteractionLease,
    ) -> Result<(Vec<NotificationInfo>, Vec<AdapterError>), AdapterError> {
        let dismissed = (0..self.dismissed)
            .map(|index| NotificationInfo {
                index,
                app_name: "Mail".into(),
                title: "Inbox".into(),
                body: None,
                actions: Vec::new(),
            })
            .collect();
        Ok((dismissed, self.failures.clone()))
    }
}

fn run(dismissed: usize, failures: &[(&str, DeliverySemantics)]) -> Value {
    let adapter = DismissAllAdapter {
        dismissed,
        failures: failures
            .iter()
            .map(|(message, disposition)| {
                AdapterError::new(ErrorCode::ActionFailed, *message).with_disposition(*disposition)
            })
            .collect(),
    };
    execute(
        DismissAllNotificationsArgs { app: None },
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap()
}

#[test]
fn nothing_to_dismiss_is_not_delivered_and_safe_to_retry() {
    assert_eq!(
        run(0, &[]),
        json!({
            "disposition": { "delivery": "not_delivered", "retry": "safe" },
            "dismissed_count": 0,
        })
    );
}

#[test]
fn every_entry_dismissed_is_delivered_verified() {
    assert_eq!(
        run(2, &[]),
        json!({
            "disposition": { "delivery": "delivered_verified", "retry": "unsafe" },
            "dismissed_count": 2,
        })
    );
}

#[test]
fn unsent_failures_without_any_dismissal_are_not_delivered_and_safe() {
    assert_eq!(
        run(
            0,
            &[(
                "#0: entry has no dismiss control",
                DeliverySemantics::not_delivered()
            )]
        ),
        json!({
            "disposition": { "delivery": "not_delivered", "retry": "safe" },
            "dismissed_count": 0,
            "failures": ["#0: entry has no dismiss control"],
            "failed_count": 1,
        })
    );
}

#[test]
fn unsent_failures_beside_dismissals_are_verified_and_unsafe_to_rerun() {
    assert_eq!(
        run(
            1,
            &[
                (
                    "#2: entry has no dismiss control",
                    DeliverySemantics::not_delivered()
                ),
                (
                    "#3: entry has no dismiss control",
                    DeliverySemantics::not_delivered()
                ),
            ]
        ),
        json!({
            "disposition": { "delivery": "delivered_verified", "retry": "unsafe" },
            "dismissed_count": 1,
            "failures": [
                "#2: entry has no dismiss control",
                "#3: entry has no dismiss control",
            ],
            "failed_count": 2,
        })
    );
}

#[test]
fn a_dispatched_failure_without_any_dismissal_is_unverified_and_unsafe() {
    assert_eq!(
        run(
            0,
            &[(
                "#0: still present after dismiss",
                DeliverySemantics::delivered_unverified()
            )]
        ),
        json!({
            "disposition": { "delivery": "delivered_unverified", "retry": "unsafe" },
            "dismissed_count": 0,
            "failures": ["#0: still present after dismiss"],
            "failed_count": 1,
        })
    );
}

#[test]
fn an_uncertain_failure_makes_the_whole_batch_unsafe() {
    assert_eq!(
        run(
            1,
            &[("#2: outcome unknown", DeliverySemantics::uncertain())]
        )["disposition"],
        json!({ "delivery": "delivered_unverified", "retry": "unsafe" })
    );
}

#[test]
fn one_dispatched_failure_overrides_unsent_failures() {
    assert_eq!(
        run(
            1,
            &[
                (
                    "#2: entry has no dismiss control",
                    DeliverySemantics::not_delivered()
                ),
                (
                    "#3: still present after dismiss",
                    DeliverySemantics::delivered_unverified()
                ),
            ]
        )["disposition"],
        json!({ "delivery": "delivered_unverified", "retry": "unsafe" })
    );
}
