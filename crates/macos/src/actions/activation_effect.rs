#[cfg(target_os = "macos")]
mod imp {
    use agent_desktop_core::Deadline;

    use crate::tree::AXElement;

    const SETTLE_POLL_MS: u64 = 40;
    const SETTLE_BUDGET_MS: u64 = 400;
    const FOCUS_DESCENT_WALK: usize = 3;

    /// The readback for `perform`, which has no written attribute to re-read.
    /// Focus alone misses a whole family of controls: a sidebar row answers an
    /// unsupported code to the action that navigates it, and moves its
    /// selection rather than the focus. Selection is therefore read as well,
    /// on every element, because an attribute an element does not publish reads
    /// as absent and the rule simply never fires for it. The focused editor's
    /// value is read because a press can replace what that editor shows. The
    /// pressed element's own value is read because a radio button answers
    /// -25205 to the press that turns it on and publishes no `AXSelected`.
    #[derive(Default)]
    pub(crate) struct FocusState {
        focused_element: Option<AXElement>,
        focused_value: Option<String>,
        selected: Option<bool>,
        own_value: Option<String>,
        menu_open: Option<bool>,
    }

    /// Accessibility hands back a fresh reference for the same element on every
    /// read, and reuses a released one for a different element later, so a raw
    /// pointer answers neither question this comparison asks. `CFEqual` is the
    /// identity the framework defines.
    impl PartialEq for FocusState {
        fn eq(&self, other: &Self) -> bool {
            match (&self.focused_element, &other.focused_element) {
                (None, None) => true,
                (Some(mine), Some(theirs)) => crate::tree::same_element(mine, theirs),
                _ => false,
            }
        }
    }

    impl FocusState {
        pub(crate) fn focused_element(self) -> Option<AXElement> {
            self.focused_element
        }
    }

    pub(crate) fn focus_state(element: &AXElement, deadline: Deadline) -> Option<FocusState> {
        let app = crate::system::app::app_ops::pid_from_element(element, deadline)
            .map(crate::tree::element_for_pid)?;
        let focused_element = crate::tree::properties::attributes::copy_element_attr_result(
            &app,
            "AXFocusedUIElement",
            deadline,
        )
        .ok()?;
        Some(FocusState {
            focused_value: focused_element
                .as_ref()
                .and_then(|focused| crate::tree::copy_value_typed(focused, deadline)),
            focused_element,
            selected: crate::tree::properties::attributes::copy_bool_attr_result(
                element,
                crate::actions::select::container_select::SELECTED,
                deadline,
            )
            .ok()
            .flatten(),
            own_value: crate::tree::copy_value_typed(element, deadline),
            menu_open: crate::actions::owned_menu::owned_menu_open(element, deadline),
        })
    }

    pub(crate) fn changed_now(
        before: &Option<FocusState>,
        element: &AXElement,
        deadline: Deadline,
    ) -> bool {
        let after = focus_state(element, deadline);
        let focused = after
            .as_ref()
            .and_then(|state| state.focused_element.as_ref());
        let target_focused =
            focused.is_some_and(|focused| focus_reached(focused, element, deadline));
        observed_change(before, &after, target_focused)
            || focus_changed_in_target_window(before, &after, || {
                focused.and_then(|focused| shares_window(focused, element, deadline)) == Some(true)
            })
    }

    /// A control that owns an editor hands focus to the editor, not to itself:
    /// a PDF form field in Preview publishes a text area and focus lands there.
    /// Demanding an exact match called that a failure and reported an action
    /// that had plainly worked as one whose outcome was unknown.
    pub(crate) fn focus_reached(
        focused: &AXElement,
        element: &AXElement,
        deadline: Deadline,
    ) -> bool {
        let mut current = focused.clone();
        for _ in 0..FOCUS_DESCENT_WALK {
            if crate::tree::same_element(&current, element) {
                return true;
            }
            let Some(parent) = crate::tree::properties::attributes::copy_element_attr_result(
                &current, "AXParent", deadline,
            )
            .ok()
            .flatten() else {
                return false;
            };
            current = parent;
        }
        false
    }

    pub(crate) fn settled_change(
        before: &Option<FocusState>,
        element: &AXElement,
        deadline: Deadline,
    ) -> bool {
        if before.is_none() {
            return false;
        }
        let deadline = deadline.capped(std::time::Duration::from_millis(SETTLE_BUDGET_MS));
        while !deadline.is_expired() {
            if changed_now(before, element, deadline) {
                return true;
            }
            std::thread::sleep(
                deadline
                    .remaining()
                    .min(std::time::Duration::from_millis(SETTLE_POLL_MS)),
            );
        }
        false
    }

    fn observed_change(
        before: &Option<FocusState>,
        after: &Option<FocusState>,
        target_focused: bool,
    ) -> bool {
        let focus_moved_to_target = target_focused
            && matches!((before, after), (Some(before), Some(after)) if before != after);
        focus_moved_to_target
            || selection_turned_on(before, after)
            || own_value_changed(before, after)
            || matches!(
                (
                    before.as_ref().and_then(|state| state.menu_open),
                    after.as_ref().and_then(|state| state.menu_open),
                ),
                (Some(false), Some(true))
            )
    }

    /// Focus moved, or the focused element shows new content, in the target's window.
    fn focus_changed_in_target_window(
        before: &Option<FocusState>,
        after: &Option<FocusState>,
        focused_in_target_window: impl FnOnce() -> bool,
    ) -> bool {
        matches!((before, after), (Some(before), Some(after))
            if after.focused_element.is_some() && (before != after
                || matches!((&before.focused_value, &after.focused_value), (Some(old), Some(new)) if old != new)))
            && focused_in_target_window()
    }

    fn shares_window(focused: &AXElement, element: &AXElement, deadline: Deadline) -> Option<bool> {
        let window = |el| {
            crate::tree::properties::attributes::copy_element_attr_result(el, "AXWindow", deadline)
        };
        Some(crate::tree::same_element(
            &window(focused).ok()??,
            &window(element).ok()??,
        ))
    }

    /// An element whose own value changed after the press was acted on: a
    /// radio button or checkbox reads 0 before and 1 after.
    fn own_value_changed(before: &Option<FocusState>, after: &Option<FocusState>) -> bool {
        matches!(
            (
                before.as_ref().and_then(|state| state.own_value.as_ref()),
                after.as_ref().and_then(|state| state.own_value.as_ref()),
            ),
            (Some(old), Some(new)) if old != new
        )
    }

    /// An element that was not selected and now is has been acted on, whatever
    /// return code the application chose. The reverse is not evidence: a
    /// selection that was already true says nothing about this action.
    fn selection_turned_on(before: &Option<FocusState>, after: &Option<FocusState>) -> bool {
        matches!(
            (
                before.as_ref().and_then(|state| state.selected),
                after.as_ref().and_then(|state| state.selected),
            ),
            (Some(false), Some(true))
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn focused(pid: i32) -> Option<FocusState> {
            Some(FocusState {
                focused_element: Some(crate::tree::element_for_pid(pid)),
                ..FocusState::default()
            })
        }

        fn selected(value: Option<bool>) -> Option<FocusState> {
            Some(FocusState {
                selected: value,
                ..FocusState::default()
            })
        }

        #[test]
        fn a_focus_change_counts_only_inside_the_pressed_elements_window() {
            let editor = |pid, value: &str| {
                Some(FocusState {
                    focused_value: Some(value.into()),
                    ..focused(pid).unwrap_or_default()
                })
            };
            let changed =
                |old, new, in_window| focus_changed_in_target_window(&old, &new, || in_window);
            assert!(changed(editor(1, "old note"), editor(1, ""), true));
            assert!(changed(focused(1), editor(2, ""), true));
            assert!(!changed(editor(1, "old note"), editor(1, ""), false));
            assert!(!changed(focused(1), editor(2, ""), false));
            assert!(!changed(editor(1, "same"), editor(1, "same"), true));
            assert!(!changed(editor(1, "old note"), focused(1), true));
        }

        #[test]
        fn focus_changes_must_correlate_with_the_target() {
            let before = focused(1);
            let after = focused(2);
            assert!(!observed_change(&before, &after, false));
            assert!(observed_change(&before, &after, true));
            assert!(!observed_change(&after, &after, true));
        }

        #[test]
        fn a_selection_that_turned_on_is_an_effect_without_any_focus_move() {
            assert!(observed_change(
                &selected(Some(false)),
                &selected(Some(true)),
                false
            ));
        }

        #[test]
        fn a_radio_value_that_turned_on_is_an_effect_and_an_unchanged_one_is_not() {
            let valued = |value: Option<&str>| {
                Some(FocusState {
                    own_value: value.map(str::to_owned),
                    ..FocusState::default()
                })
            };
            assert!(observed_change(
                &valued(Some("0")),
                &valued(Some("1")),
                false
            ));
            for (before, after) in [
                (Some("1"), Some("1")),
                (None, Some("1")),
                (Some("0"), None),
                (None, None),
            ] {
                assert!(
                    !observed_change(&valued(before), &valued(after), false),
                    "before={before:?} after={after:?}"
                );
            }
        }

        #[test]
        fn an_unchanged_or_unreadable_selection_proves_nothing() {
            for (before, after) in [
                (Some(true), Some(true)),
                (Some(false), Some(false)),
                (Some(true), Some(false)),
                (None, Some(true)),
                (Some(false), None),
                (None, None),
            ] {
                assert!(
                    !observed_change(&selected(before), &selected(after), false),
                    "before={before:?} after={after:?} must not count as an effect"
                );
            }
        }

        #[test]
        fn missing_observation_cannot_verify_an_action() {
            let before = Some(FocusState::default());
            let after = Some(FocusState::default());
            assert!(!observed_change(&before, &None, true));
            assert!(!observed_change(&None, &after, true));
            assert!(!observed_change(&None, &None, true));
            assert!(!observed_change(&before, &before, true));
            assert!(!observed_change(&before, &after, false));
            assert!(!observed_change(&before, &after, true));
        }

        #[test]
        fn only_a_newly_visible_owned_menu_verifies_delivery() {
            for before in [None, Some(false), Some(true)] {
                for after in [None, Some(false), Some(true)] {
                    let state = |menu_open| {
                        Some(FocusState {
                            menu_open,
                            ..FocusState::default()
                        })
                    };
                    assert_eq!(
                        observed_change(&state(before), &state(after), false),
                        before == Some(false) && after == Some(true),
                    );
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) use imp::focus_reached;
pub(crate) use imp::{changed_now, focus_state, settled_change};
