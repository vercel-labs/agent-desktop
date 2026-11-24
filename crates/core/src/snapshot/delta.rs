use super::baseline::SnapshotBaseline;
use super::baseline_node::BaselineNode;
use crate::{AccessibilityNode, refs::ref_token::qualify_ref_id};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct SnapshotDelta {
    pub(crate) added: Vec<Value>,
    pub(crate) updated: Vec<Value>,
    pub(crate) removed: Vec<Value>,
}

impl SnapshotDelta {
    pub(crate) fn is_empty(&self) -> bool {
        self.added.is_empty() && self.updated.is_empty() && self.removed.is_empty()
    }
}

pub(crate) struct BaselineRef<'a> {
    pub(crate) snapshot_id: &'a str,
    pub(crate) baseline: &'a SnapshotBaseline,
}

struct DiffContext<'a> {
    old_id: &'a str,
    new_id: &'a str,
    old_children: HashMap<&'a [usize], Vec<&'a BaselineNode>>,
    new_children: HashMap<&'a [usize], Vec<&'a BaselineNode>>,
    new_values: HashMap<Vec<usize>, Option<&'a str>>,
}

pub(crate) fn diff_baselines(
    old: BaselineRef<'_>,
    new: BaselineRef<'_>,
    new_raw_tree: &AccessibilityNode,
) -> SnapshotDelta {
    let mut delta = SnapshotDelta {
        added: Vec::new(),
        updated: Vec::new(),
        removed: Vec::new(),
    };
    let old_map = index_by_path(&old.baseline.nodes);
    let new_map = index_by_path(&new.baseline.nodes);
    let mut new_values = HashMap::new();
    let mut path = Vec::new();
    collect_raw_values(new_raw_tree, &mut path, &mut new_values);

    let ctx = DiffContext {
        old_id: old.snapshot_id,
        new_id: new.snapshot_id,
        old_children: group_children(&old.baseline.nodes),
        new_children: group_children(&new.baseline.nodes),
        new_values,
    };

    match (old_map.get([].as_slice()), new_map.get([].as_slice())) {
        (Some(&old_root), Some(&new_root)) => {
            if same_root_identity(old_root, new_root) {
                if let Some(updated) = build_updated_entry(old_root, new_root, &ctx) {
                    delta.updated.push(updated);
                }
                diff_children(&[], &ctx, &mut delta);
            } else {
                remove_subtree(old_root, &ctx, &mut delta);
                add_subtree(new_root, &ctx, &mut delta);
            }
        }
        (Some(&old_root), None) => remove_subtree(old_root, &ctx, &mut delta),
        (None, Some(&new_root)) => add_subtree(new_root, &ctx, &mut delta),
        (None, None) => {}
    }

    delta
}

fn index_by_path(nodes: &[BaselineNode]) -> HashMap<&[usize], &BaselineNode> {
    let mut map = HashMap::with_capacity(nodes.len());
    for node in nodes {
        map.insert(node.path.as_slice(), node);
    }
    map
}

fn group_children(nodes: &[BaselineNode]) -> HashMap<&[usize], Vec<&BaselineNode>> {
    let mut map: HashMap<&[usize], Vec<&BaselineNode>> = HashMap::new();
    for node in nodes {
        if let Some((_, parent)) = node.path.split_last() {
            map.entry(parent).or_default().push(node);
        }
    }
    map
}

fn collect_raw_values<'a>(
    node: &'a AccessibilityNode,
    path: &mut Vec<usize>,
    out: &mut HashMap<Vec<usize>, Option<&'a str>>,
) {
    out.insert(path.clone(), node.identity.value.as_deref());
    for (idx, child) in node.children.iter().enumerate() {
        path.push(idx);
        collect_raw_values(child, path, out);
        path.pop();
    }
}

fn diff_children(parent: &[usize], ctx: &DiffContext<'_>, delta: &mut SnapshotDelta) {
    let old_kids = ctx
        .old_children
        .get(parent)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let new_kids = ctx
        .new_children
        .get(parent)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let max_len = old_kids.len().max(new_kids.len());
    for idx in 0..max_len {
        match (old_kids.get(idx).copied(), new_kids.get(idx).copied()) {
            (Some(old_child), Some(new_child)) => {
                if same_identity_at_index(old_child, new_child, old_kids, new_kids, idx) {
                    if let Some(updated) = build_updated_entry(old_child, new_child, ctx) {
                        delta.updated.push(updated);
                    }
                    diff_children(&new_child.path, ctx, delta);
                } else {
                    remove_subtree(old_child, ctx, delta);
                    add_subtree(new_child, ctx, delta);
                }
            }
            (Some(old_child), None) => remove_subtree(old_child, ctx, delta),
            (None, Some(new_child)) => add_subtree(new_child, ctx, delta),
            (None, None) => {}
        }
    }
}

fn same_root_identity(old: &BaselineNode, new: &BaselineNode) -> bool {
    old.role == new.role && old.identity.native_id == new.identity.native_id
}

fn same_identity_at_index(
    old: &BaselineNode,
    new: &BaselineNode,
    old_kids: &[&BaselineNode],
    new_kids: &[&BaselineNode],
    index: usize,
) -> bool {
    if old.role != new.role || old.identity.native_id != new.identity.native_id {
        return false;
    }
    if old.identity.native_id.is_none() && old.identity.name != new.identity.name {
        let new_elsewhere_in_old = old_kids.iter().enumerate().any(|(j, candidate)| {
            j != index && candidate.role == new.role && candidate.identity.name == new.identity.name
        });
        let old_elsewhere_in_new = new_kids.iter().enumerate().any(|(j, candidate)| {
            j != index && candidate.role == old.role && candidate.identity.name == old.identity.name
        });
        if new_elsewhere_in_old || old_elsewhere_in_new {
            return false;
        }
    }
    true
}

fn add_subtree(node: &BaselineNode, ctx: &DiffContext<'_>, delta: &mut SnapshotDelta) {
    delta.added.push(build_added_entry(node, ctx));
    if let Some(children) = ctx.new_children.get(node.path.as_slice()) {
        for child in children {
            add_subtree(child, ctx, delta);
        }
    }
}

fn remove_subtree(node: &BaselineNode, ctx: &DiffContext<'_>, delta: &mut SnapshotDelta) {
    delta.removed.push(build_removed_entry(node, ctx.old_id));
    if let Some(children) = ctx.old_children.get(node.path.as_slice()) {
        for child in children {
            remove_subtree(child, ctx, delta);
        }
    }
}

fn build_added_entry(node: &BaselineNode, ctx: &DiffContext<'_>) -> Value {
    let mut map = Map::new();
    map.insert("path".into(), json!(node.path));
    map.insert("role".into(), json!(node.role));
    if let Some(local) = &node.local_ref {
        map.insert("ref_id".into(), json!(qualify_ref_id(ctx.new_id, local)));
    }
    insert_some(&mut map, "name", &node.identity.name);
    if let Some(raw_value) = ctx.new_values.get(node.path.as_slice()).copied().flatten() {
        map.insert("value".into(), formatted_value(node, Some(raw_value)));
    }
    insert_some(&mut map, "description", &node.identity.description);
    insert_some(&mut map, "native_id", &node.identity.native_id);
    insert_some(&mut map, "hint", &node.presentation.hint);
    if !node.presentation.states.is_empty() {
        map.insert("states".into(), json!(node.presentation.states));
    }
    if !node.presentation.available_actions.is_empty() {
        map.insert(
            "available_actions".into(),
            json!(node.presentation.available_actions),
        );
    }
    insert_some(&mut map, "bounds", &node.presentation.bounds);
    insert_some(&mut map, "subrole", &node.presentation.descriptors.subrole);
    insert_some(
        &mut map,
        "role_description",
        &node.presentation.descriptors.role_description,
    );
    insert_some(
        &mut map,
        "placeholder",
        &node.presentation.descriptors.placeholder,
    );
    if !node.presentation.descriptors.dom_classes.is_empty() {
        map.insert(
            "dom_classes".into(),
            json!(node.presentation.descriptors.dom_classes),
        );
    }
    insert_some(&mut map, "children_count", &node.children_count);
    if node.subtree_truncated {
        map.insert("subtree_truncated".into(), json!(true));
    }
    Value::Object(map)
}

fn build_removed_entry(node: &BaselineNode, old_id: &str) -> Value {
    let mut map = Map::new();
    map.insert("path".into(), json!(node.path));
    map.insert("role".into(), json!(node.role));
    if let Some(local) = &node.local_ref {
        map.insert("ref_id".into(), json!(qualify_ref_id(old_id, local)));
    }
    insert_some(&mut map, "name", &node.identity.name);
    insert_some(&mut map, "native_id", &node.identity.native_id);
    Value::Object(map)
}

fn build_updated_entry(
    old: &BaselineNode,
    new: &BaselineNode,
    ctx: &DiffContext<'_>,
) -> Option<Value> {
    let mut changes = Map::new();
    if old.identity.name != new.identity.name {
        changes.insert("name".into(), opt_to_json(&new.identity.name));
    }
    if old.identity.value_digest != new.identity.value_digest {
        let raw_opt = ctx.new_values.get(new.path.as_slice()).copied().flatten();
        changes.insert("value".into(), formatted_value(new, raw_opt));
    }
    if old.identity.description != new.identity.description {
        changes.insert("description".into(), opt_to_json(&new.identity.description));
    }
    if old.presentation.hint != new.presentation.hint {
        changes.insert("hint".into(), opt_to_json(&new.presentation.hint));
    }
    if old.presentation.states != new.presentation.states {
        changes.insert("states".into(), json!(new.presentation.states));
    }
    if old.presentation.available_actions != new.presentation.available_actions {
        changes.insert(
            "available_actions".into(),
            json!(new.presentation.available_actions),
        );
    }
    if old.presentation.bounds != new.presentation.bounds {
        changes.insert("bounds".into(), opt_to_json(&new.presentation.bounds));
    }
    if old.presentation.descriptors.subrole != new.presentation.descriptors.subrole {
        changes.insert(
            "subrole".into(),
            opt_to_json(&new.presentation.descriptors.subrole),
        );
    }
    if old.presentation.descriptors.role_description
        != new.presentation.descriptors.role_description
    {
        changes.insert(
            "role_description".into(),
            opt_to_json(&new.presentation.descriptors.role_description),
        );
    }
    if old.presentation.descriptors.placeholder != new.presentation.descriptors.placeholder {
        changes.insert(
            "placeholder".into(),
            opt_to_json(&new.presentation.descriptors.placeholder),
        );
    }
    if old.presentation.descriptors.dom_classes != new.presentation.descriptors.dom_classes {
        changes.insert(
            "dom_classes".into(),
            json!(new.presentation.descriptors.dom_classes),
        );
    }
    if old.children_count != new.children_count {
        changes.insert("children_count".into(), opt_to_json(&new.children_count));
    }
    if old.subtree_truncated != new.subtree_truncated {
        changes.insert("subtree_truncated".into(), json!(new.subtree_truncated));
    }
    let ref_shifted = old.local_ref != new.local_ref;
    if changes.is_empty() && !ref_shifted {
        return None;
    }

    let mut map = Map::new();
    map.insert("path".into(), json!(new.path));
    map.insert("role".into(), json!(new.role));
    if let Some(local) = &new.local_ref {
        map.insert("ref_id".into(), json!(qualify_ref_id(ctx.new_id, local)));
    } else if old.local_ref.is_some() {
        map.insert("ref_id".into(), Value::Null);
    }
    map.extend(changes);
    Some(Value::Object(map))
}

fn formatted_value(node: &BaselineNode, raw_value: Option<&str>) -> Value {
    match raw_value {
        None => Value::Null,
        Some(_) if node.identity.value_is_secure => json!({ "redacted": true }),
        Some(text) => json!(text),
    }
}

fn insert_some<T: Serialize>(map: &mut Map<String, Value>, key: &str, value: &Option<T>) {
    if let Some(inner) = value {
        map.insert(key.into(), json!(inner));
    }
}

fn opt_to_json<T: Serialize>(value: &Option<T>) -> Value {
    match value {
        Some(inner) => json!(inner),
        None => Value::Null,
    }
}

#[cfg(test)]
#[path = "delta_tests.rs"]
mod tests;
