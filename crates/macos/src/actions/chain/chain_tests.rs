use super::{ChainStep, exhaustion_disposition, record_step_outcome, step_mechanism};
use crate::actions::chain::chain_delivery::DeliveryOutcome;
use agent_desktop_core::MouseButton;
use agent_desktop_core::actions::step_mechanism::StepMechanism;

#[test]
fn step_mechanism_tags_physical_for_cgclick_and_keyboard_clear() {
    assert_eq!(
        step_mechanism(&ChainStep::CGClick {
            button: MouseButton::Left,
            count: 1,
        }),
        StepMechanism::PhysicalSynthetic
    );
    assert_eq!(
        step_mechanism(&ChainStep::FocusThenClearByKeyboard),
        StepMechanism::PhysicalSynthetic
    );
    assert_eq!(
        step_mechanism(&ChainStep::Action("AXPress")),
        StepMechanism::SemanticApi
    );
}

#[test]
fn headed_menubar_click_uses_only_semantic_press_and_keeps_other_clicks_physical() {
    let menu = crate::actions::chain::chain_defs::click_chain(true);
    assert!(matches!(menu.steps, [ChainStep::Action("AXPress")]));
    assert!(!menu.continue_after_unverified_delivery);
    let mut steps = Vec::new();
    assert!(record_step_outcome(
        &mut steps,
        &menu.steps[0],
        DeliveryOutcome::DeliveredUnverified,
        false
    ));
    assert_eq!(steps[0].mechanism(), Some(StepMechanism::SemanticApi));
    assert_eq!(steps[0].verified(), Some(false));
    let mut unsupported = Vec::new();
    assert!(!record_step_outcome(
        &mut unsupported,
        &menu.steps[0],
        DeliveryOutcome::NotDelivered,
        false,
    ));
    assert_eq!(
        exhaustion_disposition(&unsupported),
        agent_desktop_core::DeliverySemantics::not_delivered()
    );
    assert!(matches!(
        crate::actions::chain::chain_defs::click_chain(false)
            .steps
            .first(),
        Some(ChainStep::CGClick { .. })
    ));
}
