use super::stub::{PNG_BYTES, Stub};
use crate::commands::{permissions, screenshot, version};
use crate::refs::refs_test_support::TempDir;
use crate::{PermissionReport, PermissionState};
use serde_json::json;

golden_test!(
    version_reports_package_version_target_and_os,
    include_str!(
        "../../../../../../tests/fixtures/contract/inventory-version_reports_package_version_target_and_os.json"
    ),
    &["version", "target", "os"],
    {
        let value = version::execute().unwrap();
        assert_eq!(value["os"], std::env::consts::OS);
        value
    }
);

golden_test!(
    permissions_without_and_with_the_guide,
    include_str!(
        "../../../../../../tests/fixtures/contract/inventory-permissions_without_and_with_the_guide.json"
    ),
    &[],
    {
        let report = PermissionReport {
            accessibility: PermissionState::Denied {
                suggestion: "Allow it".into(),
            },
            screen_recording: PermissionState::Unknown,
            automation: PermissionState::Granted,
        };
        let run = |guide| {
            let args = permissions::PermissionsArgs {
                request: false,
                guide,
            };
            permissions::execute_with_report(args, &Stub::default(), &report).unwrap()
        };
        json!([run(false), run(true)])
    }
);

golden_test!(
    screenshot_inline_and_saved_to_a_path,
    include_str!(
        "../../../../../../tests/fixtures/contract/inventory-screenshot_inline_and_saved_to_a_path.json"
    ),
    &["path"],
    {
        let dir = TempDir::new();
        let path = dir.path().join("shot.png");
        let run = |output_path| {
            let args = screenshot::ScreenshotArgs {
                app: None,
                window_id: None,
                screen: None,
                output_path,
            };
            screenshot::execute(args, &Stub::default()).unwrap()
        };
        let value = json!([run(None), run(Some(path.clone()))]);
        assert_eq!(std::fs::read(&path).unwrap(), PNG_BYTES);
        value
    }
);
