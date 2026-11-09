use crate::adapter::*;
use crate::*;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Mutex;

pub(super) const PNG_BYTES: [u8; 4] = [137, 80, 78, 71];

type Reply<T> = Result<T, AdapterError>;
type Content = Option<ClipboardContent>;

#[derive(Default)]
pub(super) struct Stub(Value, Mutex<Content>);

impl Stub {
    pub(super) fn with(data: Value) -> Self {
        Stub(data, Mutex::default())
    }

    pub(super) fn holding(content: Content) -> Self {
        Stub(Value::Null, Mutex::new(content))
    }

    fn rows<T: DeserializeOwned>(&self, key: &str) -> Vec<T> {
        serde_json::from_value(self.0[key].clone()).unwrap_or_default()
    }
}

impl ObservationOps for Stub {
    fn query_app_inventory(
        &self,
        query: AppInventoryQuery<'_>,
        _: Deadline,
    ) -> Reply<AppInventory> {
        Ok(AppInventory {
            skipped: self.rows("skipped"),
            ..query.answer(self.rows("apps"))
        })
    }
    fn list_windows(&self, _: &WindowFilter, _: Deadline) -> Reply<Vec<WindowInfo>> {
        Ok(self.rows("windows"))
    }
    fn list_surfaces(&self, _: ProcessIdentity, _: Deadline) -> Reply<Vec<SurfaceInfo>> {
        Ok(self.rows("surfaces"))
    }
}

impl ActionOps for Stub {}

impl InputOps for Stub {
    fn get_clipboard_content(&self, _: ClipboardFormat, _: Deadline) -> Reply<Content> {
        Ok(self.1.lock().unwrap().take())
    }
    fn set_clipboard_content(
        &self,
        _: &ClipboardContent,
        _: &InteractionLease,
    ) -> Reply<OperationReport> {
        Ok(OperationReport::delivered_unverified(
            "clipboard-set",
            OperationTarget::Clipboard,
        ))
    }
    fn clear_clipboard(&self, _: &InteractionLease) -> Reply<OperationReport> {
        Ok(OperationReport::delivered_unverified(
            "clipboard-clear",
            OperationTarget::Clipboard,
        ))
    }
}

impl SystemOps for Stub {
    guarded_interaction_lease!();
    fn list_displays(&self, _: Deadline) -> Reply<Vec<DisplayInfo>> {
        Ok(self.rows("displays"))
    }
    fn list_notifications(
        &self,
        _: &NotificationFilter,
        _: InteractionPolicy,
        _: Deadline,
        _: Option<&InteractionLease>,
    ) -> Reply<Vec<NotificationInfo>> {
        Ok(self.rows("notifications"))
    }
    fn screenshot(&self, _: ScreenshotTarget, _: Deadline) -> Reply<ImageBuffer> {
        let (width, height, scale_factor) = (640, 480, 2.0);
        let format = ImageFormat::Png;
        Ok(ImageBuffer {
            data: PNG_BYTES.to_vec(),
            format,
            width,
            height,
            scale_factor,
        })
    }
    fn start_permission_guide(&self) -> Reply<&'static str> {
        Ok("started")
    }
    fn supported_surfaces(&self) -> Vec<SnapshotSurface> {
        vec![SnapshotSurface::Window, SnapshotSurface::Menu]
    }
}
