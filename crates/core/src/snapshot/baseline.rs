use super::baseline_hash::{hash_canonical_tree, sha256_hex, strip_snapshot_prefix};
use super::baseline_identity::BaselineIdentity;
use super::baseline_node::BaselineNode;
use super::baseline_owner::BaselineOwner;
use super::baseline_presentation::BaselinePresentation;
use super::baseline_scope::BaselineScope;
use crate::{
    AccessibilityNode,
    adapter::TreeOptions,
    snapshot::SnapshotResult,
    tree::state::{SECURE, has_state},
};
use serde::{Deserialize, Serialize};

pub(crate) const BASELINE_FORMAT_VERSION: u32 = 1;
pub(crate) const MAX_BASELINE_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SnapshotBaseline {
    pub(crate) format_version: u32,
    pub(crate) complete: bool,
    pub(crate) owner: BaselineOwner,
    pub(crate) scope: BaselineScope,
    pub(crate) tree_hash: String,
    pub(crate) nodes: Vec<BaselineNode>,
}

impl SnapshotBaseline {
    pub(crate) fn from_snapshot_result(
        result: &SnapshotResult,
        opts: &TreeOptions,
        root_ref: Option<&str>,
    ) -> Self {
        let mut nodes = Vec::new();
        let mut path = Vec::new();
        collect_nodes(&result.tree, &mut path, &mut nodes);
        Self {
            format_version: BASELINE_FORMAT_VERSION,
            complete: result.complete,
            owner: BaselineOwner::from_snapshot_owner(&result.owner),
            scope: BaselineScope::from_options(opts, root_ref),
            tree_hash: hash_canonical_tree(&result.tree),
            nodes,
        }
    }

    pub(crate) fn serialize_with_size_check(&self) -> Option<String> {
        let json = serde_json::to_string(self).ok()?;
        if json.len() as u64 > MAX_BASELINE_BYTES {
            tracing::warn!(bytes = json.len(), "snapshot baseline exceeds size cap");
            return None;
        }
        Some(json)
    }
}

fn collect_nodes(node: &AccessibilityNode, path: &mut Vec<usize>, out: &mut Vec<BaselineNode>) {
    let secure = has_state(&node.presentation.states, SECURE);
    let value_digest = node
        .identity
        .value
        .as_deref()
        .map(|value| sha256_hex(value.as_bytes()));
    let local_ref = node
        .ref_id
        .as_deref()
        .map(|raw| format!("@{}", strip_snapshot_prefix(raw)));
    out.push(BaselineNode {
        path: path.clone(),
        role: node.role.clone(),
        local_ref,
        identity: BaselineIdentity {
            name: node.identity.name.clone(),
            value_digest,
            value_is_secure: secure,
            description: node.identity.description.clone(),
            native_id: node.identity.native_id.clone(),
        },
        presentation: BaselinePresentation {
            hint: node.presentation.hint.clone(),
            states: node.presentation.states.clone(),
            available_actions: node.presentation.available_actions.clone(),
            bounds: node.presentation.bounds,
            descriptors: node.presentation.descriptors.clone(),
        },
        children_count: node.children_count,
        subtree_truncated: node.subtree_truncated,
    });
    for (idx, child) in node.children.iter().enumerate() {
        path.push(idx);
        collect_nodes(child, path, out);
        path.pop();
    }
}

pub(crate) fn redact_sensitive_tree_values(result: &mut SnapshotResult) {
    redact_node(&mut result.tree);
    result.refmap.redact_secure_values();
}

fn redact_node(node: &mut AccessibilityNode) {
    if has_state(&node.presentation.states, SECURE) {
        node.identity.value = None;
    }
    for child in &mut node.children {
        redact_node(child);
    }
}
