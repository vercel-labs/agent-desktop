use crate::SnapshotOwner;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BaselineOwner {
    pub(crate) owner_kind: String,
    pub(crate) app: String,
    pub(crate) pid: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) process_instance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) window_id: Option<String>,
}

impl BaselineOwner {
    pub(crate) fn from_snapshot_owner(owner: &SnapshotOwner) -> Self {
        match owner {
            SnapshotOwner::Application(app) => Self {
                owner_kind: "application".to_owned(),
                app: app.name.clone(),
                pid: app.pid.get(),
                process_instance: app.process_instance.clone(),
                window_id: None,
            },
            SnapshotOwner::Window(window) if window.is_process_owner() => Self {
                owner_kind: "process".to_owned(),
                app: window.app.clone(),
                pid: window.pid.get(),
                process_instance: window.process_instance.clone(),
                window_id: None,
            },
            SnapshotOwner::Window(window) => Self {
                owner_kind: "window".to_owned(),
                app: window.app.clone(),
                pid: window.pid.get(),
                process_instance: window.process_instance.clone(),
                window_id: Some(window.id.clone()),
            },
        }
    }
}
