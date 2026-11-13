use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{AdapterError, InteractionLease, NotificationInfo};
use serde_json::json;

struct DismissAdapter;

impl ObservationOps for DismissAdapter {}
impl ActionOps for DismissAdapter {}
impl InputOps for DismissAdapter {}

impl SystemOps for DismissAdapter {
    crate::adapter::guarded_interaction_lease!();

    fn dismiss_notification(
        &self,
        request: crate::DismissNotificationRequest<'_>,
        _lease: &InteractionLease,
    ) -> Result<NotificationInfo, AdapterError> {
        Ok(NotificationInfo {
            index: request.index,
            app_name: "Mail".into(),
            title: "Inbox".into(),
            body: None,
            actions: Vec::new(),
        })
    }
}

#[test]
fn dismissed_notification_is_reported_with_verified_delivery() {
    let value = execute(
        DismissNotificationArgs {
            index: 3,
            app: None,
            expected_app: Some("Mail".into()),
            expected_title: None,
        },
        &DismissAdapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap();

    assert_eq!(
        value,
        json!({
            "disposition": { "delivery": "delivered_verified", "retry": "unsafe" },
            "dismissed": { "index": 3, "app_name": "Mail", "title": "Inbox" },
        })
    );
}
