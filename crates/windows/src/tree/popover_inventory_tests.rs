use super::{PopupFacts, is_popover};

fn facts() -> PopupFacts {
    PopupFacts {
        owned_by_same_process: true,
        popup_style: true,
        no_activate: false,
        menu_class: false,
    }
}

#[test]
fn only_an_owned_activating_popup_that_is_not_a_menu_is_a_popover() {
    assert!(is_popover(&facts()));
    assert!(!is_popover(&PopupFacts {
        owned_by_same_process: false,
        ..facts()
    }));
    assert!(!is_popover(&PopupFacts {
        popup_style: false,
        ..facts()
    }));
    assert!(!is_popover(&PopupFacts {
        no_activate: true,
        ..facts()
    }));
    assert!(!is_popover(&PopupFacts {
        menu_class: true,
        ..facts()
    }));
}
