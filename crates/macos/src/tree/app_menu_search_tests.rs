use super::*;
use agent_desktop_core::Deadline;

#[test]
fn surface_node_limit_is_explicitly_incomplete() {
    let error = surface_limit_error();

    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert_eq!(error.details.expect("limit details")["complete"], false);
}

#[test]
fn menus_beyond_eight_wrappers_remain_discoverable() {
    let deadline = Deadline::after(5000).unwrap();
    for target_depth in [9, 11, 30] {
        let found = find_surface_descendant_with(
            0,
            "app".into(),
            ChildExpansion::AnchorsOnly,
            deadline,
            |depth, _| Ok((*depth == target_depth, vec![depth + 1])),
        )
        .unwrap();
        assert_eq!(found.map(|(_, node)| node), Some(target_depth));
    }
}

#[test]
fn cyclic_menu_search_remains_bounded_and_explicitly_incomplete() {
    let mut reads = 0;
    let error = find_surface_descendant_with(
        0,
        "app".into(),
        ChildExpansion::AnchorsOnly,
        Deadline::after(5000).unwrap(),
        |node, _| {
            reads += 1;
            Ok((false, vec![*node]))
        },
    )
    .unwrap_err();
    assert_eq!(reads, MAX_SURFACE_NODES);
    assert_eq!(error.details.unwrap()["complete"], false);
}

#[test]
fn expired_menu_search_never_reads_a_node() {
    let result = find_surface_descendant_with::<()>(
        (),
        "app".into(),
        ChildExpansion::AnchorsOnly,
        Deadline::after(0).unwrap(),
        |_, _| panic!("expired search must not call AX"),
    );
    assert_eq!(result.unwrap_err().code, ErrorCode::Timeout);
}

#[test]
fn incomplete_content_branch_does_not_hide_a_menu_in_a_sibling() {
    let result = find_surface_descendant_with(
        0,
        "app".into(),
        ChildExpansion::AnchorsOnly,
        Deadline::after(5000).unwrap(),
        |node, _| match node {
            0 => Ok((false, vec![1, 2])),
            1 => Err(surface_limit_error()),
            2 => Ok((true, vec![])),
            _ => unreachable!(),
        },
    );
    assert_eq!(result.unwrap(), Some(("app/children/1".into(), 2)));
}

#[test]
fn an_incomplete_branch_cannot_prove_menu_absence() {
    let result = find_surface_descendant_with(
        0,
        "app".into(),
        ChildExpansion::AnchorsOnly,
        Deadline::after(5000).unwrap(),
        |node, _| match node {
            0 => Ok((false, vec![1, 2])),
            1 => Err(surface_limit_error()),
            _ => Ok((false, vec![])),
        },
    );
    assert_eq!(result.unwrap_err().details.unwrap()["complete"], false);
}

#[test]
fn shallow_menu_precedes_unbounded_content_and_permission_errors_stay_terminal() {
    let deadline = Deadline::after(5000).unwrap();
    let found = find_surface_descendant_with(
        0,
        "app".into(),
        ChildExpansion::AnchorsOnly,
        deadline,
        |node, _| match node {
            0 => Ok((false, vec![1, 2])),
            1 => Ok((false, vec![1])),
            _ => Ok((true, vec![])),
        },
    );
    assert_eq!(found.unwrap(), Some(("app/children/1".into(), 2)));
    let denied = find_surface_descendant_with(
        0,
        "app".into(),
        ChildExpansion::AnchorsOnly,
        deadline,
        |node, _| match node {
            0 => Ok((false, vec![1, 2])),
            1 => Err(AdapterError::new(ErrorCode::PermDenied, "denied")),
            _ => panic!("permission failure must stop all further AX reads"),
        },
    );
    assert_eq!(denied.unwrap_err().code, ErrorCode::PermDenied);
}

#[test]
fn invalidated_menu_roots_are_reacquired_once_and_permission_errors_are_terminal() {
    use accessibility_sys::{kAXErrorAPIDisabled, kAXErrorInvalidUIElement};
    for (ax_error, failures) in [
        (kAXErrorInvalidUIElement, 1),
        (kAXErrorInvalidUIElement, 2),
        (kAXErrorAPIDisabled, 1),
    ] {
        let mut reads = 0;
        let result = reacquire_once(Deadline::after(1000).unwrap(), &mut false, || {
            reads += 1;
            if reads <= failures {
                Err(AdapterError::new(ErrorCode::PermDenied, "read failed")
                    .with_details(serde_json::json!({ "ax_error": ax_error })))
            } else {
                Ok("fresh root")
            }
        });
        if ax_error == kAXErrorAPIDisabled {
            assert_eq!(reads, 1);
            assert_eq!(result.unwrap_err().code, ErrorCode::PermDenied);
        } else {
            assert_eq!(reads, 2);
            if failures == 1 {
                assert_eq!(result.unwrap(), "fresh root");
            } else {
                let error = result.unwrap_err();
                assert_eq!(error.code, ErrorCode::AppUnresponsive);
                let details = error.details.unwrap();
                assert_eq!(details["complete"], false);
                assert_eq!(details["menu_root_reacquired"], true);
                assert_eq!(details["retryable"], false);
                assert_eq!(details["ax_error"], kAXErrorInvalidUIElement);
            }
        }
    }
}

#[test]
fn the_items_of_a_content_container_are_checked_but_not_opened() {
    let deadline = Deadline::after(5000).unwrap();
    let wide = super::super::surface_inventory::ANCHOR_CHILD_LIMIT + 1;
    let mut opened = Vec::new();
    let found = find_surface_descendant_with(
        0_usize,
        "app".into(),
        ChildExpansion::AnchorsOnly,
        deadline,
        |node, expand| {
            if expand {
                opened.push(*node);
            }
            Ok(match *node {
                0 => (false, vec![1, 2]),
                1 => (false, (100..100 + wide).collect()),
                2 => (false, vec![3]),
                3 => (true, vec![]),
                item if item >= 100 => (false, vec![item * 10]),
                _ => (false, vec![]),
            })
        },
    )
    .unwrap();

    assert_eq!(found.map(|(_, node)| node), Some(3));
    assert!(opened.iter().all(|node| *node < 100));
}

#[test]
fn a_menu_among_the_items_of_a_content_container_is_still_found() {
    let deadline = Deadline::after(5000).unwrap();
    let wide = super::super::surface_inventory::ANCHOR_CHILD_LIMIT + 1;
    let found = find_surface_descendant_with(
        0_usize,
        "app".into(),
        ChildExpansion::AnchorsOnly,
        deadline,
        |node, _| {
            Ok(match *node {
                0 => (false, (100..100 + wide).collect()),
                node => (node == 100 + wide - 1, vec![]),
            })
        },
    )
    .unwrap();

    assert_eq!(found.map(|(_, node)| node), Some(100 + wide - 1));
}

#[test]
fn a_popover_anchored_inside_wide_content_is_reached_only_when_every_node_is_opened() {
    let deadline = Deadline::after(5000).unwrap();
    let wide = super::super::surface_inventory::ANCHOR_CHILD_LIMIT + 1;
    let popover = (100 + wide - 1) * 10;
    let locate = |expansion: ChildExpansion| {
        find_surface_descendant_with(
            0_usize,
            "app".into(),
            expansion,
            deadline,
            |node, expand| {
                Ok(match *node {
                    0 => (false, vec![1]),
                    1 => (false, (100..100 + wide).collect()),
                    item if item == popover => (true, vec![]),
                    item if item >= 100 => {
                        (false, if expand { vec![item * 10] } else { Vec::new() })
                    }
                    _ => (false, vec![]),
                })
            },
        )
        .unwrap()
    };

    assert_eq!(
        locate(ChildExpansion::EveryNode),
        Some((
            format!("app/children/0/children/{}/children/0", wide - 1),
            popover
        ))
    );
    assert_eq!(
        locate(ChildExpansion::AnchorsOnly).map(|(_, node)| node),
        None
    );
}
