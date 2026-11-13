use super::menu_lineage::Lineage;
use agent_desktop_core::AdapterError;
use std::rc::Rc;

const MENU_ROLE: &str = "AXMenu";
const MENU_BAR_ROLE: &str = "AXMenuBar";
const MENU_ITEM_ROLE: &str = "AXMenuItem";
const NON_CONTAINER_ROLES: [&str; 4] = [MENU_ROLE, MENU_BAR_ROLE, "AXMenuBarItem", MENU_ITEM_ROLE];
const LANDMARK_SUBROLE_PREFIX: &str = "AXLandmark";

pub(super) trait MenuTreeReader<T> {
    fn string(&mut self, element: &T, attribute: &str) -> Result<Option<String>, AdapterError>;
    fn boolean(&mut self, element: &T, attribute: &str) -> Result<Option<bool>, AdapterError>;
    fn children(&mut self, element: &T) -> Result<Vec<T>, AdapterError>;
}

pub(super) type PopupContainer<T> = Option<(String, T)>;

pub(super) type MenuSearchStep<T> = (bool, Vec<Rc<Lineage<T>>>);

pub(super) fn inspect_node<T: Clone>(
    node: &Rc<Lineage<T>>,
    reader: &mut impl MenuTreeReader<T>,
    container: Option<&mut PopupContainer<T>>,
) -> Result<MenuSearchStep<T>, AdapterError> {
    let role = reader.string(&node.element, "AXRole")?;
    match role.as_deref() {
        Some(MENU_BAR_ROLE) => return Ok((false, Vec::new())),
        Some(MENU_ROLE) if reader.boolean(&node.element, "AXVisible")? != Some(false) => {
            return Ok((true, Vec::new()));
        }
        Some(MENU_ITEM_ROLE) => record_container(node, reader, container)?,
        _ => {}
    }
    let children = reader.children(&node.element)?;
    Ok((
        false,
        children
            .into_iter()
            .enumerate()
            .map(|(index, child)| Lineage::child(node, child, index))
            .collect(),
    ))
}

fn record_container<T: Clone>(
    item: &Rc<Lineage<T>>,
    reader: &mut impl MenuTreeReader<T>,
    container: Option<&mut PopupContainer<T>>,
) -> Result<(), AdapterError> {
    let Some(slot) = container.filter(|slot| slot.is_none()) else {
        return Ok(());
    };
    if reader.boolean(&item.element, "AXVisible")? == Some(false) {
        return Ok(());
    }
    if let Some(parent) = item.parent.as_ref()
        && is_popup_container(parent, reader)?
    {
        *slot = Some((parent.path.clone(), parent.element.clone()));
    }
    Ok(())
}

fn is_popup_container<T>(
    parent: &Rc<Lineage<T>>,
    reader: &mut impl MenuTreeReader<T>,
) -> Result<bool, AdapterError> {
    if let Some(verdict) = parent.container_verdict.get() {
        return Ok(verdict);
    }
    let verdict = judge_container(parent, reader)?;
    parent.container_verdict.set(Some(verdict));
    Ok(verdict)
}

fn judge_container<T>(
    parent: &Rc<Lineage<T>>,
    reader: &mut impl MenuTreeReader<T>,
) -> Result<bool, AdapterError> {
    let role = reader.string(&parent.element, "AXRole")?;
    if role
        .as_deref()
        .is_some_and(|role| NON_CONTAINER_ROLES.contains(&role))
    {
        return Ok(false);
    }
    Ok(!excluded_by_ancestry(parent, reader)?)
}

fn excluded_by_ancestry<T>(
    start: &Rc<Lineage<T>>,
    reader: &mut impl MenuTreeReader<T>,
) -> Result<bool, AdapterError> {
    let mut current = Some(start);
    while let Some(node) = current {
        if reader.boolean(&node.element, "AXVisible")? == Some(false) {
            return Ok(true);
        }
        let subrole = reader.string(&node.element, "AXSubrole")?;
        if subrole
            .as_deref()
            .is_some_and(|subrole| subrole.starts_with(LANDMARK_SUBROLE_PREFIX))
        {
            return Ok(true);
        }
        current = node.parent.as_ref();
    }
    Ok(false)
}

#[cfg(test)]
#[path = "popup_menu_tests.rs"]
mod tests;
