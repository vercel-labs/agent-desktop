use agent_desktop_core::{
    AdapterError, ChainRung, Deadline, DeliveryObservation, DeliveryOutcome, Effect, KeyCombo,
    Modifier, MouseButton, StepMechanism,
};

use crate::actions::chain::{
    ChainContext, activate_descendant, chain_disclosure_steps, chain_menu_steps, chain_value_write,
    press_selection,
};
use crate::actions::{DeliveryTracker, ax_helpers, scroll_into_view, select::container_select};
use crate::tree::AXElement;

const PHYSICAL_CLICK: &str = "CGClick";
const POINTER_EFFECTS: &[Effect] = &[Effect::FocusSteal, Effect::CursorMove];
const KEYBOARD_EFFECTS: &[Effect] = &[Effect::FocusSteal];
const UNVERIFIED: DeliveryOutcome = DeliveryOutcome::Delivered(DeliveryObservation::Unverified);

type ElementRun = fn(&AXElement, Deadline) -> Result<DeliveryOutcome, AdapterError>;

pub(crate) fn click_rungs<'a>(
    element: &'a AXElement,
    ctx: &ChainContext<'a>,
    native_menu: bool,
) -> Vec<ChainRung<'a>> {
    let deadline = ctx.deadline;
    if native_menu {
        return vec![action("AXPress", element, deadline)];
    }
    let context = *ctx;
    vec![
        physical(PHYSICAL_CLICK, POINTER_EFFECTS, move || {
            physical_click(element, &context, MouseButton::Left, 1)?;
            Ok(UNVERIFIED)
        }),
        custom(
            "AXPress",
            press_selection::press_or_select_container,
            element,
            deadline,
        ),
        action("AXOpen", element, deadline),
        custom(
            "activate_descendant",
            activate_descendant::activate_descendant,
            element,
            deadline,
        ),
        custom(
            "select_within_container",
            container_select::select_within_container,
            element,
            deadline,
        ),
        action("AXConfirm", element, deadline),
    ]
}

pub(crate) fn right_click_rungs<'a>(
    element: &'a AXElement,
    ctx: &ChainContext<'a>,
) -> Vec<ChainRung<'a>> {
    let context = *ctx;
    let deadline = ctx.deadline;
    vec![
        physical(PHYSICAL_CLICK, POINTER_EFFECTS, move || {
            let pid = crate::system::app::app_ops::pid_from_element(element, deadline);
            physical_click(element, &context, MouseButton::Right, 1)?;
            let menu_seen = match pid {
                Some(pid) => chain_menu_steps::wait_for_new_menu(pid, deadline)
                    .map_err(|error| DeliveryTracker::from_delivered_units(1).annotate(error))?,
                None => false,
            };
            Ok(DeliveryOutcome::from_delivery(true, menu_seen))
        }),
        custom(
            "show_menu",
            chain_menu_steps::show_menu_by_any_route,
            element,
            deadline,
        ),
    ]
}

pub(crate) fn expand_rungs<'a>(
    element: &'a AXElement,
    ctx: &ChainContext<'a>,
) -> Vec<ChainRung<'a>> {
    vec![custom(
        "expand_verified",
        chain_disclosure_steps::press_to_expand,
        element,
        ctx.deadline,
    )]
}

pub(crate) fn collapse_rungs<'a>(
    element: &'a AXElement,
    ctx: &ChainContext<'a>,
) -> Vec<ChainRung<'a>> {
    vec![custom(
        "collapse_verified",
        chain_disclosure_steps::press_to_collapse,
        element,
        ctx.deadline,
    )]
}

pub(crate) fn set_value_rungs<'a>(
    element: &'a AXElement,
    ctx: &ChainContext<'a>,
) -> Vec<ChainRung<'a>> {
    let value = ctx.dynamic_value;
    let deadline = ctx.deadline;
    vec![
        value_rung(element, ctx),
        semantic("IncrementToDynamic", move || match value {
            Some(value) => chain_value_write::increment_to_value(element, value, deadline),
            None => Ok(DeliveryOutcome::NotDelivered),
        }),
    ]
}

pub(crate) fn clear_rungs<'a>(
    element: &'a AXElement,
    ctx: &ChainContext<'a>,
) -> Vec<ChainRung<'a>> {
    let policy = ctx.policy;
    let deadline = ctx.deadline;
    vec![
        physical("FocusThenClearByKeyboard", KEYBOARD_EFFECTS, move || {
            crate::actions::physical_keyboard::press_sequence(
                element,
                &select_all_then_delete(),
                policy,
                deadline,
            )?;
            Ok(UNVERIFIED)
        }),
        value_rung(element, ctx),
    ]
}

pub(crate) fn semantic_click_rungs<'a>(
    element: &'a AXElement,
    ctx: &ChainContext<'a>,
) -> Vec<ChainRung<'a>> {
    let deadline = ctx.deadline;
    vec![
        action("AXPress", element, deadline),
        action("AXOpen", element, deadline),
        custom(
            "activate_descendant",
            activate_descendant::activate_descendant,
            element,
            deadline,
        ),
        custom(
            "select_within_container",
            container_select::select_within_container,
            element,
            deadline,
        ),
        action("AXConfirm", element, deadline),
    ]
}

pub(crate) fn focus_rungs<'a>(
    element: &'a AXElement,
    ctx: &ChainContext<'a>,
) -> Vec<ChainRung<'a>> {
    let deadline = ctx.deadline;
    vec![semantic("AXFocused", move || {
        prepare(element, deadline)?;
        if !ax_helpers::is_attr_settable(element, "AXFocused", deadline)? {
            return Ok(DeliveryOutcome::NotDelivered);
        }
        chain_value_write::set_bool_verified(element, "AXFocused", true, deadline)
    })]
}

pub(crate) fn scroll_to_rungs<'a>(
    element: &'a AXElement,
    ctx: &ChainContext<'a>,
) -> Vec<ChainRung<'a>> {
    vec![custom(
        "scroll_to_visible_verified",
        scroll_into_view::scroll_into_view_outcome,
        element,
        ctx.deadline,
    )]
}

pub(crate) fn multi_click_rungs<'a>(
    element: &'a AXElement,
    ctx: &ChainContext<'a>,
    count: u32,
) -> Vec<ChainRung<'a>> {
    let context = *ctx;
    vec![physical(PHYSICAL_CLICK, POINTER_EFFECTS, move || {
        physical_click(element, &context, MouseButton::Left, count)?;
        Ok(UNVERIFIED)
    })]
}

fn semantic<'a>(
    label: &'static str,
    run: impl FnMut() -> Result<DeliveryOutcome, AdapterError> + 'a,
) -> ChainRung<'a> {
    ChainRung {
        label,
        effects: &[],
        mechanism: StepMechanism::SemanticApi,
        run: Box::new(run),
    }
}

fn physical<'a>(
    label: &'static str,
    effects: &'static [Effect],
    run: impl FnMut() -> Result<DeliveryOutcome, AdapterError> + 'a,
) -> ChainRung<'a> {
    ChainRung {
        label,
        effects,
        mechanism: StepMechanism::PhysicalSynthetic,
        run: Box::new(run),
    }
}

fn custom<'a>(
    label: &'static str,
    run: ElementRun,
    element: &'a AXElement,
    deadline: Deadline,
) -> ChainRung<'a> {
    semantic(label, move || run(element, deadline))
}

fn action<'a>(label: &'static str, element: &'a AXElement, deadline: Deadline) -> ChainRung<'a> {
    semantic(label, move || {
        prepare(element, deadline)?;
        ax_helpers::perform_observed_action(element, label, deadline)
    })
}

fn value_rung<'a>(element: &'a AXElement, ctx: &ChainContext<'a>) -> ChainRung<'a> {
    let value = ctx.dynamic_value;
    let deadline = ctx.deadline;
    semantic("AXValue", move || {
        let Some(value) = value else {
            return Ok(DeliveryOutcome::NotDelivered);
        };
        prepare(element, deadline)?;
        if !ax_helpers::is_attr_settable(element, "AXValue", deadline)? {
            return Ok(DeliveryOutcome::NotDelivered);
        }
        chain_value_write::set_dynamic_verified(element, "AXValue", value, deadline)
    })
}

fn physical_click(
    element: &AXElement,
    ctx: &ChainContext<'_>,
    button: MouseButton,
    count: u32,
) -> Result<(), AdapterError> {
    crate::actions::physical_click::click_via_bounds(
        element,
        crate::actions::physical_click::PhysicalClick {
            button,
            count,
            verified_point: ctx.verified_point.cloned(),
        },
        ctx.policy,
        ctx.deadline,
    )
}

fn select_all_then_delete() -> [KeyCombo; 2] {
    [
        KeyCombo {
            key: "a".into(),
            modifiers: vec![Modifier::Meta],
        },
        KeyCombo {
            key: "delete".into(),
            modifiers: vec![],
        },
    ]
}

fn prepare(element: &AXElement, deadline: Deadline) -> Result<(), AdapterError> {
    crate::tree::properties::attributes::set_messaging_timeout(element, deadline)
}

#[cfg(test)]
#[path = "chain_defs_tests.rs"]
mod tests;
