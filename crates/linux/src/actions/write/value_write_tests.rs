use super::*;

fn names(list: &[&str]) -> Vec<String> {
    list.iter()
        .map(|name| format!("org.a11y.atspi.{name}"))
        .collect()
}

#[test]
fn a_slider_with_the_value_interface_writes_a_number() {
    let interfaces = names(&["Value"]);
    assert_eq!(
        route(Some(SLIDER), Some(&interfaces)),
        Route::Numeric { spin_button: false }
    );
}

#[test]
fn a_spin_button_with_both_interfaces_takes_the_numeric_path() {
    let interfaces = names(&["Text", "EditableText", "Value"]);
    assert_eq!(
        route(Some(SPIN_BUTTON), Some(&interfaces)),
        Route::Numeric { spin_button: true }
    );
}

#[test]
fn a_scroll_bar_and_a_dial_take_the_numeric_path() {
    let interfaces = names(&["Value"]);
    assert!(matches!(
        route(Some(SCROLL_BAR), Some(&interfaces)),
        Route::Numeric { .. }
    ));
    assert!(matches!(
        route(Some(DIAL), Some(&interfaces)),
        Route::Numeric { .. }
    ));
}

#[test]
fn an_entry_with_editable_text_takes_the_text_path() {
    let interfaces = names(&["Text", "EditableText"]);
    assert_eq!(route(Some(79), Some(&interfaces)), Route::Text);
}

#[test]
fn a_slider_without_the_value_interface_has_no_route() {
    let interfaces = names(&["Component"]);
    assert_eq!(route(Some(SLIDER), Some(&interfaces)), Route::Unsupported);
}

#[test]
fn a_node_without_editable_text_or_value_is_unsupported() {
    let interfaces = names(&["Text"]);
    assert_eq!(route(Some(79), Some(&interfaces)), Route::Unsupported);
}

#[test]
fn unreadable_interfaces_still_try_the_text_path() {
    assert_eq!(route(Some(79), None), Route::Text);
    assert!(text_advertised(None));
}

#[test]
fn an_unsupported_answer_is_action_not_supported_and_not_delivered() {
    let error = unsupported("clear");
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn a_table_cell_with_the_value_interface_takes_the_cell_route() {
    let interfaces = names(&["Text", "Value"]);
    assert_eq!(route(Some(TABLE_CELL), Some(&interfaces)), Route::Cell);
    let without = names(&["Text"]);
    assert_eq!(route(Some(TABLE_CELL), Some(&without)), Route::Unsupported);
}

#[test]
fn text_for_a_cell_is_refused_before_anything_is_sent() {
    let error = cell_takes_a_number();
    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert!(error.message.contains("takes a number"));
    assert!(error.message.contains("no background route"));
}

#[test]
fn text_for_a_cell_that_also_has_editable_text_takes_the_text_route() {
    let both = names(&["Text", "Value", "EditableText"]);
    assert_eq!(
        set_value_route(Some(TABLE_CELL), Some(&both), "abc"),
        Route::Text
    );
    assert_eq!(
        set_value_route(Some(TABLE_CELL), Some(&both), "42"),
        Route::Cell
    );
    let value_only = names(&["Text", "Value"]);
    assert_eq!(
        set_value_route(Some(TABLE_CELL), Some(&value_only), "abc"),
        Route::Cell
    );
}
