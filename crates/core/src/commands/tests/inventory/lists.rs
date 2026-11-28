use super::stub::Stub;
use crate::CommandContext;
use crate::commands::{list_apps, list_displays, list_notifications, list_surfaces, list_windows};
use serde_json::{Value, json};

fn apps() -> Value {
    json!([
        {"name": "Notes", "pid": 100, "bundle_id": "org.example.notes", "process_instance": "i-100", "presentation": "background"},
        {"name": "Term", "pid": 200}
    ])
}

fn list_apps_of(data: Value) -> Value {
    let args = list_apps::ListAppsArgs {
        app: None,
        timeout_ms: None,
    };
    list_apps::execute(args, &Stub::with(data)).unwrap()
}

golden_test!(
    list_apps_complete_inventory,
    include_str!(
        "../../../../../../tests/fixtures/contract/inventory-list_apps_complete_inventory.json"
    ),
    &[],
    { list_apps_of(json!({"apps": apps()})) }
);

golden_test!(
    list_apps_partial_inventory_reports_skipped_records,
    include_str!(
        "../../../../../../tests/fixtures/contract/inventory-list_apps_partial_inventory_reports_skipped_records.json"
    ),
    &[],
    {
        list_apps_of(json!({"apps": apps(), "skipped": [{"pid": 11, "field": "application_name"}]}))
    }
);

golden_test!(
    list_windows_with_every_optional_field_and_none,
    include_str!(
        "../../../../../../tests/fixtures/contract/inventory-list_windows_with_every_optional_field_and_none.json"
    ),
    &[],
    {
        let full = json!({"id": "w-1", "title": "One", "app_name": "Notes", "pid": 100, "process_instance": "i-100",
            "bounds": {"x": 10.5, "y": -20.0, "width": 800.0, "height": 600.0},
            "is_focused": true, "accessible": false, "minimized": true, "visible": false});
        let bare = json!({"id": "w-2", "title": "Two", "app_name": "Notes", "pid": 100, "is_focused": false});
        let args = list_windows::ListWindowsArgs {
            app: None,
            timeout_ms: None,
        };
        list_windows::execute(args, &Stub::with(json!({"windows": [full, bare]}))).unwrap()
    }
);

golden_test!(
    list_displays_primary_and_secondary,
    include_str!(
        "../../../../../../tests/fixtures/contract/inventory-list_displays_primary_and_secondary.json"
    ),
    &[],
    {
        let bounds = |x: f64| json!({"x": x, "y": 0.0, "width": 1440.0, "height": 900.5});
        let displays = json!([
            {"id": "1", "bounds": bounds(0.0), "is_primary": true, "scale": 2.0},
            {"id": "2", "bounds": bounds(-1920.0), "is_primary": false, "scale": 1.0}
        ]);
        list_displays::execute(&Stub::with(json!({"displays": displays}))).unwrap()
    }
);

golden_test!(
    list_surfaces_with_every_optional_field_and_none,
    include_str!(
        "../../../../../../tests/fixtures/contract/inventory-list_surfaces_with_every_optional_field_and_none.json"
    ),
    &[],
    {
        let surfaces = json!([
            {"id": "s-1", "type": "sheet", "title": "Save", "item_count": 3, "unclassified": ["menu"]},
            {"id": "s-2", "type": "alert"}
        ]);
        let args = list_surfaces::ListSurfacesArgs {
            app: Some("Notes".into()),
        };
        list_surfaces::execute(
            args,
            &Stub::with(json!({"apps": apps(), "surfaces": surfaces})),
        )
        .unwrap()
    }
);

golden_test!(
    list_notifications_with_and_without_entries,
    include_str!(
        "../../../../../../tests/fixtures/contract/inventory-list_notifications_with_and_without_entries.json"
    ),
    &[],
    {
        let notes = json!([
            {"index": 1, "app_name": "Mail", "title": "New message", "body": "Hello", "actions": ["Reply"]},
            {"index": 2, "app_name": "Mail", "title": "Reminder"}
        ]);
        let list = |data| {
            let args = list_notifications::ListNotificationsArgs {
                app: None,
                text: None,
                limit: None,
            };
            list_notifications::execute(args, &Stub::with(data), &CommandContext::default())
                .unwrap()
        };
        json!([list(json!({"notifications": notes})), list(json!({}))])
    }
);
