//! The measured UI Automation calls, the two controls, and element discovery.
//!
//! Element names are content, so no name is recorded: an element is described
//! by control type, class, automation id, framework id and runtime id.

use std::time::Instant;

use serde_json::{Value, json};
use uiautomation::patterns::{UIScrollItemPattern, UIScrollPattern};
use uiautomation::types::{ScrollAmount, TreeScope, UIProperty};
use uiautomation::variants::Variant;
use uiautomation::{UIAutomation, UIElement};
use windows::Win32::UI::Accessibility::{
    IUIAutomationElement, IUIAutomationItemContainerPattern, IUIAutomationVirtualizedItemPattern,
    UIA_ItemContainerPatternId, UIA_NamePropertyId, UIA_VirtualizedItemPatternId,
};

pub const NO_SCROLL: f64 = -1.0;
const WALK_NODE_LIMIT: usize = 6000;

pub struct Outcome {
    pub ok: bool,
    pub hresult: Option<String>,
    pub sentinel: Option<i32>,
    pub note: Option<String>,
    pub ms: f64,
}

impl Outcome {
    pub fn to_json(&self) -> Value {
        json!({
            "ok": self.ok,
            "hresult": if self.ok { Some("S_OK".to_string()) } else { self.hresult.clone() },
            "crate_sentinel": self.sentinel,
            "note": self.note,
            "ms": (self.ms * 100.0).round() / 100.0,
        })
    }
}

fn hex(code: i32) -> String {
    format!("0x{:08X}", code as u32)
}

fn timed(call: impl FnOnce() -> uiautomation::Result<()>) -> Outcome {
    let started = Instant::now();
    let result = call();
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    match result {
        Ok(()) => Outcome {
            ok: true,
            hresult: None,
            sentinel: None,
            note: None,
            ms,
        },
        Err(error) => Outcome {
            ok: false,
            hresult: error.result().map(|result| hex(result.0)),
            sentinel: error.result().is_none().then(|| error.code()),
            note: None,
            ms,
        },
    }
}

pub fn scroll_small(pattern: &UIScrollPattern, toward_end: bool) -> Outcome {
    let amount = if toward_end {
        ScrollAmount::SmallIncrement
    } else {
        ScrollAmount::SmallDecrement
    };
    timed(|| pattern.scroll(ScrollAmount::NoAmount, amount))
}

pub fn set_percent(pattern: &UIScrollPattern, vertical: f64) -> Outcome {
    timed(|| pattern.set_scroll_percent(NO_SCROLL, vertical))
}

pub fn scroll_item(element: &UIElement) -> Option<UIScrollItemPattern> {
    element.get_pattern::<UIScrollItemPattern>().ok()
}

pub fn scroll_into_view(pattern: &UIScrollItemPattern) -> Outcome {
    timed(|| pattern.scroll_into_view())
}

pub fn set_focus(element: &UIElement) -> Outcome {
    timed(|| element.set_focus())
}

pub fn read_name(element: &UIElement) -> Outcome {
    timed(|| element.get_name().map(|_| ()))
}

pub fn item_container(element: &UIElement) -> Option<IUIAutomationItemContainerPattern> {
    let raw: &IUIAutomationElement = element.as_ref();
    unsafe {
        raw.GetCurrentPatternAs::<IUIAutomationItemContainerPattern>(UIA_ItemContainerPatternId)
    }
    .ok()
}

/// `FindItemByProperty(null, UIA_NamePropertyId, name)`. An `S_OK` answer with
/// no element is the container saying "no such item", reported as a note.
pub fn find_item(
    container: &IUIAutomationItemContainerPattern,
    name: &str,
) -> (Outcome, Option<IUIAutomationElement>) {
    let value = Variant::from(name);
    let started = Instant::now();
    let result = unsafe {
        container.FindItemByProperty(None::<&IUIAutomationElement>, UIA_NamePropertyId, value.as_ref())
    };
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    match result {
        Ok(item) => (
            Outcome {
                ok: true,
                hresult: None,
                sentinel: None,
                note: Some("item returned".into()),
                ms,
            },
            Some(item),
        ),
        Err(error) if error.code().0 == 0 => (
            Outcome {
                ok: true,
                hresult: None,
                sentinel: None,
                note: Some("S_OK with no element".into()),
                ms,
            },
            None,
        ),
        Err(error) => (
            Outcome {
                ok: false,
                hresult: Some(hex(error.code().0)),
                sentinel: None,
                note: None,
                ms,
            },
            None,
        ),
    }
}

pub fn virtualized(item: &IUIAutomationElement) -> Option<IUIAutomationVirtualizedItemPattern> {
    unsafe {
        item.GetCurrentPatternAs::<IUIAutomationVirtualizedItemPattern>(UIA_VirtualizedItemPatternId)
    }
    .ok()
}

pub fn realize(pattern: &IUIAutomationVirtualizedItemPattern) -> Outcome {
    let started = Instant::now();
    let result = unsafe { pattern.Realize() };
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    match result {
        Ok(()) => Outcome {
            ok: true,
            hresult: None,
            sentinel: None,
            note: None,
            ms,
        },
        Err(error) => Outcome {
            ok: false,
            hresult: Some(hex(error.code().0)),
            sentinel: None,
            note: None,
            ms,
        },
    }
}

fn flag(element: &UIElement, property: UIProperty) -> Option<bool> {
    let variant = element.get_property_value(property).ok()?;
    match variant.get_value() {
        Ok(uiautomation::variants::Value::BOOL(value)) => Some(value),
        _ => None,
    }
}

pub fn vertically_scrollable(element: &UIElement) -> bool {
    flag(element, UIProperty::IsScrollPatternAvailable) == Some(true)
        && element
            .get_pattern::<UIScrollPattern>()
            .ok()
            .and_then(|pattern| pattern.is_vertically_scrollable().ok())
            == Some(true)
}

pub fn describe(element: &UIElement) -> Value {
    let pattern = element.get_pattern::<UIScrollPattern>().ok();
    let rect = element.get_bounding_rectangle().ok();
    json!({
        "control_type": element.get_control_type().ok().map(|kind| format!("{kind:?}")),
        "class": element.get_classname().ok(),
        "automation_id": element.get_automation_id().ok(),
        "framework_id": element.get_framework_id().ok(),
        "runtime_id": element.get_runtime_id().ok(),
        "pid": element.get_process_id().ok(),
        "native_window_handle": element
            .get_native_window_handle()
            .ok()
            .map(|handle| { let value: isize = handle.into(); value }),
        "size": rect.map(|rect| [rect.get_width(), rect.get_height()]),
        "keyboard_focusable": element.is_keyboard_focusable().ok(),
        "offscreen": element.is_offscreen().ok(),
        "scroll_pattern": pattern.as_ref().map(|pattern| json!({
            "vertically_scrollable": pattern.is_vertically_scrollable().ok(),
            "horizontally_scrollable": pattern.is_horizontally_scrollable().ok(),
            "vertical_percent": pattern.get_vertical_scroll_percent().ok(),
            "vertical_view_size": pattern.get_vertical_view_size().ok(),
        })),
        "item_container_pattern": item_container(element).is_some(),
    })
}

fn walk(automation: &UIAutomation, root: &UIElement) -> Vec<UIElement> {
    let Ok(walker) = automation.get_raw_view_walker() else {
        return Vec::new();
    };
    let mut out = vec![root.clone()];
    let mut index = 0;
    while index < out.len() && out.len() < WALK_NODE_LIMIT {
        let parent = out[index].clone();
        index += 1;
        let mut child = walker.get_first_child(&parent).ok();
        while let Some(current) = child {
            child = walker.get_next_sibling(&current).ok();
            out.push(current);
            if out.len() >= WALK_NODE_LIMIT {
                break;
            }
        }
    }
    out
}

/// Every element under `root` whose ScrollPattern reports a scrollable
/// vertical axis: one `FindAll` on the availability property, with the raw
/// walk as the second source when `FindAll` answers nothing.
pub fn find_scrollables(automation: &UIAutomation, root: &UIElement) -> (Vec<UIElement>, &'static str) {
    let found = automation
        .create_property_condition(UIProperty::IsScrollPatternAvailable, Variant::from(true), None)
        .and_then(|condition| root.find_all(TreeScope::Subtree, &condition))
        .unwrap_or_default();
    let from_find_all: Vec<UIElement> = found.into_iter().filter(vertically_scrollable).collect();
    if !from_find_all.is_empty() {
        return (from_find_all, "FindAll");
    }
    let walked = walk(automation, root)
        .into_iter()
        .filter(vertically_scrollable)
        .collect();
    (walked, "raw_walk")
}

/// How many raw-view nodes each framework id and control type contributes,
/// and how many advertise ScrollPattern: shows whether a provider exposed its
/// content at all, without recording any name.
pub fn census(automation: &UIAutomation, root: &UIElement) -> Value {
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let nodes = walk(automation, root);
    let mut scroll_available = 0usize;
    for node in &nodes {
        let key = format!(
            "{}/{}",
            node.get_framework_id().unwrap_or_default(),
            node.get_control_type().map(|kind| format!("{kind:?}")).unwrap_or_default()
        );
        *counts.entry(key).or_default() += 1;
        if flag(node, UIProperty::IsScrollPatternAvailable) == Some(true) {
            scroll_available += 1;
        }
    }
    json!({ "nodes": nodes.len(), "scroll_pattern_available": scroll_available, "by_framework_and_type": counts })
}

pub fn first_focusable(automation: &UIAutomation, root: &UIElement) -> Option<UIElement> {
    let condition = automation
        .create_property_condition(UIProperty::IsKeyboardFocusable, Variant::from(true), None)
        .ok()?;
    root.find_first(TreeScope::Subtree, &condition).ok()
}

pub fn first_item_container(automation: &UIAutomation, root: &UIElement) -> Option<UIElement> {
    let condition = automation
        .create_property_condition(
            UIProperty::IsItemContainerPatternAvailable,
            Variant::from(true),
            None,
        )
        .ok()?;
    root.find_first(TreeScope::Subtree, &condition).ok()
}

pub struct Selector {
    pub automation_id: Option<String>,
    pub class: Option<String>,
    pub control_type: Option<String>,
    pub nth: usize,
}

pub fn pick(elements: &[UIElement], selector: &Selector) -> Option<UIElement> {
    elements
        .iter()
        .filter(|element| {
            selector
                .automation_id
                .as_ref()
                .is_none_or(|wanted| element.get_automation_id().ok().as_ref() == Some(wanted))
                && selector
                    .class
                    .as_ref()
                    .is_none_or(|wanted| element.get_classname().ok().as_ref() == Some(wanted))
                && selector.control_type.as_ref().is_none_or(|wanted| {
                    element
                        .get_control_type()
                        .ok()
                        .map(|kind| format!("{kind:?}"))
                        .as_ref()
                        == Some(wanted)
                })
        })
        .nth(selector.nth)
        .cloned()
}

/// One control pattern call by name, for the `act` mode: the same bracket as
/// the scroll calls, on the other patterns the adapter marks as foreground.
pub fn pattern_call(element: &UIElement, call: &str, value: &str) -> Outcome {
    use uiautomation::patterns::{
        UIExpandCollapsePattern, UIInvokePattern, UILegacyIAccessiblePattern, UIRangeValuePattern,
        UISelectionItemPattern, UITogglePattern, UIValuePattern,
    };
    match call {
        "invoke" => timed(|| element.get_pattern::<UIInvokePattern>()?.invoke()),
        "toggle" => timed(|| element.get_pattern::<UITogglePattern>()?.toggle()),
        "setvalue" => timed(|| element.get_pattern::<UIValuePattern>()?.set_value(value)),
        "range" => timed(|| {
            element
                .get_pattern::<UIRangeValuePattern>()?
                .set_value(value.parse().unwrap_or(0.0))
        }),
        "expand" => timed(|| element.get_pattern::<UIExpandCollapsePattern>()?.expand()),
        "collapse" => timed(|| element.get_pattern::<UIExpandCollapsePattern>()?.collapse()),
        "select" => timed(|| element.get_pattern::<UISelectionItemPattern>()?.select()),
        "scrollitem" => timed(|| element.get_pattern::<UIScrollItemPattern>()?.scroll_into_view()),
        "legacy_default" => timed(|| element.get_pattern::<UILegacyIAccessiblePattern>()?.do_default_action()),
        "name" => read_name(element),
        "focus" => set_focus(element),
        _ => Outcome { ok: false, hresult: None, sentinel: None, note: Some("unknown call".into()), ms: 0.0 },
    }
}

/// The readable state an `act` call can change, read back for the effect.
pub fn effect_state(element: &UIElement) -> Value {
    use uiautomation::patterns::{
        UIExpandCollapsePattern, UIRangeValuePattern, UISelectionItemPattern, UITogglePattern, UIValuePattern,
    };
    json!({
        "toggle": element.get_pattern::<UITogglePattern>().ok().and_then(|p| p.get_toggle_state().ok()).map(|s| s as i32),
        "value_len": element.get_pattern::<UIValuePattern>().ok().and_then(|p| p.get_value().ok()).map(|v| v.len()),
        "range": element.get_pattern::<UIRangeValuePattern>().ok().and_then(|p| p.get_value().ok()),
        "selected": element.get_pattern::<UISelectionItemPattern>().ok().and_then(|p| p.is_selected().ok()),
        "expand": element.get_pattern::<UIExpandCollapsePattern>().ok().and_then(|p| p.get_state().ok()).map(|s| s as i32),
        "name_len": element.get_name().ok().map(|n| n.len()),
    })
}

pub fn find_by(automation: &UIAutomation, root: &UIElement, automation_id: Option<&str>, name: Option<&str>) -> Option<UIElement> {
    let (property, wanted) = match (automation_id, name) {
        (Some(id), _) => (UIProperty::AutomationId, id),
        (None, Some(name)) => (UIProperty::Name, name),
        _ => return None,
    };
    let condition = automation.create_property_condition(property, Variant::from(wanted), None).ok()?;
    root.find_first(TreeScope::Subtree, &condition).ok()
}

/// Names of the selected items of a selection container. Only the harness's
/// own scratch target is read this way, so the names are test content.
pub fn selected_names(element: &UIElement) -> Option<Vec<String>> {
    use uiautomation::patterns::UISelectionPattern;
    let selection = element.get_pattern::<UISelectionPattern>().ok()?.get_selection().ok()?;
    Some(selection.iter().filter_map(|item| item.get_name().ok()).collect())
}
