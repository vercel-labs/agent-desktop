use agent_desktop_core::{ChainRung, Deadline, Effect, InteractionPolicy, StepMechanism};

use super::{
    cell_rungs, clear_rungs, click_rungs, collapse_rungs, expand_rungs, focus_rungs,
    multi_click_rungs, right_click_rungs, scroll_to_rungs, semantic_click_rungs, set_value_rungs,
};
use crate::actions::chain::ChainContext;
use crate::tree::AXElement;

const POINTER: &[Effect] = &[Effect::FocusSteal, Effect::CursorMove];

fn context() -> ChainContext<'static> {
    ChainContext {
        dynamic_value: None,
        verified_point: None,
        deadline: Deadline::after(5_000).unwrap(),
        policy: InteractionPolicy::headed(),
    }
}

fn labels(rungs: &[ChainRung<'_>]) -> Vec<&'static str> {
    rungs.iter().map(|rung| rung.label).collect()
}

fn assert_semantic(rungs: &[ChainRung<'_>]) {
    for rung in rungs {
        assert!(rung.effects.is_empty(), "{} declares an effect", rung.label);
        assert_eq!(rung.mechanism, StepMechanism::SemanticApi, "{}", rung.label);
    }
}

#[test]
fn a_click_tries_the_physical_click_first_and_declares_the_foreground_it_takes() {
    let element = AXElement(std::ptr::null_mut());
    let ctx = context();

    let rungs = click_rungs(&element, &ctx, false);

    assert_eq!(
        labels(&rungs),
        [
            "CGClick",
            "AXPress",
            "AXOpen",
            "activate_descendant",
            "select_within_container",
            "AXConfirm"
        ]
    );
    assert_eq!(rungs[0].effects, POINTER);
    assert_eq!(rungs[0].mechanism, StepMechanism::PhysicalSynthetic);
    assert_semantic(&rungs[1..]);
}

#[test]
fn a_native_menu_click_is_the_single_semantic_press() {
    let element = AXElement(std::ptr::null_mut());
    let ctx = context();

    let rungs = click_rungs(&element, &ctx, true);

    assert_eq!(labels(&rungs), ["AXPress"]);
    assert_semantic(&rungs);
}

#[test]
fn a_right_click_pairs_the_physical_click_with_one_semantic_menu_rung() {
    let element = AXElement(std::ptr::null_mut());
    let ctx = context();

    let rungs = right_click_rungs(&element, &ctx);

    assert_eq!(labels(&rungs), ["CGClick", "show_menu"]);
    assert_eq!(rungs[0].effects, POINTER);
    assert_eq!(rungs[0].mechanism, StepMechanism::PhysicalSynthetic);
    assert_semantic(&rungs[1..]);
}

#[test]
fn a_multi_click_is_one_physical_rung_because_no_semantic_route_repeats_a_click() {
    let element = AXElement(std::ptr::null_mut());
    let ctx = context();

    for count in [2, 3] {
        let rungs = multi_click_rungs(&element, &ctx, count);

        assert_eq!(labels(&rungs), ["CGClick"]);
        assert_eq!(rungs[0].effects, POINTER);
        assert_eq!(rungs[0].mechanism, StepMechanism::PhysicalSynthetic);
    }
}

#[test]
fn clearing_by_keyboard_takes_focus_and_never_the_pointer() {
    let element = AXElement(std::ptr::null_mut());
    let ctx = context();

    let rungs = clear_rungs(&element, &ctx);

    assert_eq!(labels(&rungs), ["FocusThenClearByKeyboard", "AXValue"]);
    assert_eq!(rungs[0].effects, [Effect::FocusSteal]);
    assert_eq!(rungs[0].mechanism, StepMechanism::PhysicalSynthetic);
    assert_semantic(&rungs[1..]);
}

#[test]
fn every_other_chain_holds_only_semantic_rungs() {
    let element = AXElement(std::ptr::null_mut());
    let ctx = context();

    for rungs in [
        expand_rungs(&element, &ctx),
        collapse_rungs(&element, &ctx),
        set_value_rungs(&element, &ctx),
        semantic_click_rungs(&element, &ctx),
        focus_rungs(&element, &ctx),
        scroll_to_rungs(&element, &ctx),
    ] {
        assert!(!rungs.is_empty());
        assert_semantic(&rungs);
    }
}

#[test]
fn only_a_cell_or_its_editor_gets_a_rung_that_types_and_it_takes_no_foreground() {
    use crate::actions::cell_editor::CellTarget;
    let element = AXElement(std::ptr::null_mut());
    let ctx = context();

    assert!(cell_rungs(CellTarget::Neither, &element, &ctx).is_empty());
    for (target, label) in [
        (CellTarget::Cell, "CellEditorWrite"),
        (CellTarget::Editor, "PIDTargetedReplaceText"),
    ] {
        let rungs = cell_rungs(target, &element, &ctx);

        assert_eq!(labels(&rungs), [label]);
        assert!(rungs[0].effects.is_empty());
        assert_eq!(rungs[0].mechanism, StepMechanism::PhysicalSynthetic);
    }
}

#[test]
fn a_cell_whose_press_takes_the_foreground_declares_it_on_the_write_rung() {
    use crate::actions::cell_editor::CellTarget;
    let element = AXElement(std::ptr::null_mut());
    let ctx = context();

    let rungs = cell_rungs(CellTarget::ForegroundCell, &element, &ctx);

    assert_eq!(labels(&rungs), ["CellEditorWrite"]);
    assert_eq!(rungs[0].effects, [Effect::FocusSteal]);
}
