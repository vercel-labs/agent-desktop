use super::popover_count;

#[test]
fn a_child_whose_role_could_not_be_read_leaves_the_popover_count_unknown() {
    let role = |name: &str| Ok::<_, ()>(Some(name.to_owned()));
    assert_eq!(
        popover_count([role("AXPopover"), role("AXButton"), Ok(None)].into_iter()),
        Some(1)
    );
    assert_eq!(popover_count([role("AXButton"), Err(())].into_iter()), None);
}
