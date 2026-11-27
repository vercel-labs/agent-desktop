use crate::locator::live_locator::ObservedTree;

/// Parent links and DFS entry/exit timestamps over one observed tree, so any
/// ancestor scope query is a binary search over the shared window posting list
/// with no per-ancestor cache allocations.
pub(crate) struct TreeIndex {
    pub(crate) parents: Vec<Option<usize>>,
    pub(crate) order: Vec<usize>,
    pub(crate) tin: Vec<u32>,
    pub(crate) tout: Vec<u32>,
}

impl TreeIndex {
    pub(crate) fn build(observed: &ObservedTree) -> Self {
        let len = observed.nodes.len();
        let mut parents = vec![None; len];
        for (parent, node) in observed.nodes.iter().enumerate() {
            for &child in &node.children {
                if let Some(slot) = parents.get_mut(child as usize) {
                    *slot = Some(parent);
                }
            }
        }
        let mut order = Vec::with_capacity(len);
        let mut tin = vec![0_u32; len];
        let mut tout = vec![0_u32; len];
        let mut timer = 0_u32;
        let mut stack: Vec<(usize, bool)> = Vec::new();
        for start in observed
            .roots
            .iter()
            .map(|&root| root as usize)
            .chain(0..len)
        {
            if start >= len || tin[start] != 0 {
                continue;
            }
            stack.push((start, false));
            while let Some((node, exiting)) = stack.pop() {
                if exiting {
                    tout[node] = timer;
                    continue;
                }
                if tin[node] != 0 {
                    continue;
                }
                timer += 1;
                tin[node] = timer;
                order.push(node);
                stack.push((node, true));
                for &child in observed.nodes[node].children.iter().rev() {
                    let child = child as usize;
                    if child < len && tin[child] == 0 {
                        stack.push((child, false));
                    }
                }
            }
        }
        Self {
            parents,
            order,
            tin,
            tout,
        }
    }
}
