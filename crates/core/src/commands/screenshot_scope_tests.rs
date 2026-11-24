use super::*;
use std::sync::Mutex;

struct ScopedWindowAdapter {
    unfiltered_windows: Vec<WindowInfo>,
    scoped_windows: Vec<WindowInfo>,
    observed_filters: Mutex<Vec<Option<String>>>,
    captured: Mutex<Option<ScreenshotTarget>>,
}

impl ScopedWindowAdapter {
    fn phantom_only() -> Self {
        let mut phantoms = Vec::new();
        for id in [105843, 105844, 105845, 105846, 105847, 105849] {
            let mut win = window(&format!("w-{id}"), 700);
            win.app = "TextEdit".into();
            win.state.visible = Some(false);
            win.state.accessible = false;
            phantoms.push(win);
        }
        Self {
            unfiltered_windows: phantoms,
            scoped_windows: Vec::new(),
            observed_filters: Mutex::new(Vec::new()),
            captured: Mutex::new(None),
        }
    }

    fn take_captured(&self) -> Option<ScreenshotTarget> {
        self.captured.lock().expect("captured lock").take()
    }
}

impl ObservationOps for ScopedWindowAdapter {
    fn list_apps(&self, _deadline: crate::Deadline) -> Result<Vec<crate::AppInfo>, AdapterError> {
        Ok(vec![crate::AppInfo {
            name: "TextEdit".into(),
            pid: crate::ProcessId::new(700),
            bundle_id: Some("com.apple.TextEdit".into()),
            process_instance: Some("instance-700".into()),
            presentation: None,
        }])
    }

    fn list_windows(
        &self,
        filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        self.observed_filters
            .lock()
            .expect("filters lock")
            .push(filter.app.clone());
        if filter.app.is_none() {
            return Ok(self.unfiltered_windows.clone());
        }
        Ok(self.scoped_windows.clone())
    }
}

impl ActionOps for ScopedWindowAdapter {}
impl InputOps for ScopedWindowAdapter {}

impl SystemOps for ScopedWindowAdapter {
    fn screenshot(
        &self,
        target: ScreenshotTarget,
        _deadline: crate::Deadline,
    ) -> Result<ImageBuffer, AdapterError> {
        *self.captured.lock().expect("captured lock") = Some(target);
        Ok(ImageBuffer {
            data: vec![1, 2, 3],
            format: ImageFormat::Png,
            width: 640,
            height: 480,
            scale_factor: 2.0,
        })
    }
}

#[test]
fn app_target_uses_scoped_window_filter_and_returns_window_not_found_when_no_real_window_exists() {
    let adapter = ScopedWindowAdapter::phantom_only();
    let mut request = args();
    request.app = Some("TextEdit".into());

    let error = screenshot::execute(request, &adapter)
        .expect_err("app without targetable windows must not capture or report ambiguity");

    assert_eq!(error.code(), "WINDOW_NOT_FOUND");
    let crate::AppError::Adapter(adapter_error) = error else {
        panic!("expected adapter error");
    };
    assert!(adapter_error.suggestion.is_some());
    assert_eq!(
        adapter_error.disposition,
        crate::DeliverySemantics::not_delivered()
    );
    assert_eq!(
        *adapter.observed_filters.lock().expect("filters lock"),
        vec![Some("TextEdit".to_string())]
    );
    assert!(adapter.take_captured().is_none());
}
