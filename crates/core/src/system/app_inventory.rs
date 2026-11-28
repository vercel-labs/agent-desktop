use crate::{AdapterError, AppInfo};

#[derive(Debug, Clone)]
pub struct AppInventory {
    pub apps: Vec<AppInfo>,
    pub skipped: Vec<serde_json::Value>,
}

impl AppInventory {
    /// The refusal for a lookup that matched nothing while some records were
    /// skipped: the skipped record may be the application asked for, so its
    /// absence is not proven. Nothing was sent.
    pub fn incomplete_error(&self, subject: &str) -> AdapterError {
        AdapterError::app_unresponsive(subject)
            .with_details(serde_json::json!({
                "kind": "app_inventory_incomplete",
                "skipped": self.skipped,
            }))
            .unsent()
    }
}
