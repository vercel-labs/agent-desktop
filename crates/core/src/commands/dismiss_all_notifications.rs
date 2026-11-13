use crate::{AppError, adapter::PlatformAdapter, context::CommandContext};
use serde_json::{Value, json};

pub struct DismissAllNotificationsArgs {
    pub app: Option<String>,
}

pub fn execute(
    args: DismissAllNotificationsArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let policy = super::notification_policy::mutation_policy(context)?;
    let lease = crate::commands::helpers::acquire_interaction_lease(adapter)?;
    let (dismissed, failures) = adapter.dismiss_all_notifications(
        crate::DismissAllNotificationsRequest {
            app_filter: args.app.as_deref(),
            policy,
        },
        &lease,
    )?;
    let dispositions: Vec<crate::DeliverySemantics> =
        failures.iter().map(|failure| failure.disposition).collect();
    let mut result = json!({
        "disposition": crate::DeliverySemantics::merged(dismissed.len(), &dispositions),
        "dismissed_count": dismissed.len(),
    });
    if !failures.is_empty() {
        result["failures"] = json!(
            failures
                .iter()
                .map(|failure| failure.message.as_str())
                .collect::<Vec<_>>()
        );
        result["failed_count"] = json!(failures.len());
    }
    Ok(result)
}

#[cfg(test)]
#[path = "dismiss_all_notifications_tests.rs"]
mod tests;
