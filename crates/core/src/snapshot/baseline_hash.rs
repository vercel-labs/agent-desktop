//! Deterministic SHA-256 hashing and canonical accessibility-tree encoding.

use crate::{AccessibilityNode, ElementIdentifier, Rect};

const INITIAL_STATE: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

const ROUND_CONSTANTS: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

pub(crate) struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffer_len: usize,
    total_len: u64,
}

impl Sha256 {
    pub(crate) fn new() -> Self {
        Self {
            state: INITIAL_STATE,
            buffer: [0; 64],
            buffer_len: 0,
            total_len: 0,
        }
    }

    pub(crate) fn update(&mut self, mut input: &[u8]) {
        self.total_len = self.total_len.wrapping_add(input.len() as u64);
        if self.buffer_len > 0 {
            let needed = 64 - self.buffer_len;
            if input.len() < needed {
                self.buffer[self.buffer_len..self.buffer_len + input.len()].copy_from_slice(input);
                self.buffer_len += input.len();
                return;
            }
            self.buffer[self.buffer_len..64].copy_from_slice(&input[..needed]);
            let block = self.buffer;
            compress(&mut self.state, &block);
            self.buffer_len = 0;
            input = &input[needed..];
        }
        while input.len() >= 64 {
            let mut block = [0u8; 64];
            block.copy_from_slice(&input[..64]);
            compress(&mut self.state, &block);
            input = &input[64..];
        }
        if !input.is_empty() {
            self.buffer[..input.len()].copy_from_slice(input);
            self.buffer_len = input.len();
        }
    }

    pub(crate) fn finish_hex(mut self) -> String {
        let bit_len = self.total_len.wrapping_mul(8);
        self.update(&[0x80]);
        if self.buffer_len > 56 {
            self.buffer[self.buffer_len..64].fill(0);
            let block = self.buffer;
            compress(&mut self.state, &block);
            self.buffer_len = 0;
        }
        self.buffer[self.buffer_len..56].fill(0);
        self.buffer[56..64].copy_from_slice(&bit_len.to_be_bytes());
        let block = self.buffer;
        compress(&mut self.state, &block);
        let mut out = String::with_capacity(64);
        for word in self.state {
            use std::fmt::Write;
            let _ = write!(&mut out, "{word:08x}");
        }
        out
    }
}

fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for (idx, chunk) in block.chunks_exact(4).enumerate() {
        w[idx] = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ ((!e) & g);
        let temp1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(ROUND_CONSTANTS[i])
            .wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let temp2 = s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(temp1);
        d = c;
        c = b;
        b = a;
        a = temp1.wrapping_add(temp2);
    }
    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finish_hex()
}

pub(crate) fn hash_canonical_tree(tree: &AccessibilityNode) -> String {
    let mut hasher = Sha256::new();
    hash_node(&mut hasher, tree);
    hasher.finish_hex()
}

fn hash_node(hasher: &mut Sha256, node: &AccessibilityNode) {
    hasher.update(b"node:v1");
    hash_opt_str(hasher, node.ref_id.as_deref().map(strip_snapshot_prefix));
    hash_str(hasher, &node.role);
    hash_opt_str(hasher, node.identity.name.as_deref());
    hash_opt_str(hasher, node.identity.value.as_deref());
    hash_opt_str(hasher, node.identity.description.as_deref());
    hash_native_id(hasher, node.identity.native_id.as_ref());
    hash_opt_str(hasher, node.presentation.hint.as_deref());
    hash_str_slice(hasher, &node.presentation.states);
    hash_str_slice(hasher, &node.presentation.available_actions);
    hash_bounds(hasher, node.presentation.bounds);
    hash_opt_str(hasher, node.presentation.descriptors.subrole.as_deref());
    hash_opt_str(
        hasher,
        node.presentation.descriptors.role_description.as_deref(),
    );
    hash_opt_str(hasher, node.presentation.descriptors.placeholder.as_deref());
    hash_str_slice(hasher, &node.presentation.descriptors.dom_classes);
    match node.children_count {
        Some(count) => {
            hasher.update(&[1]);
            hasher.update(&count.to_le_bytes());
        }
        None => hasher.update(&[0]),
    }
    hasher.update(&[u8::from(node.subtree_truncated)]);
    hasher.update(&(node.children.len() as u64).to_le_bytes());
    for child in &node.children {
        hash_node(hasher, child);
    }
}

pub(crate) fn strip_snapshot_prefix(ref_id: &str) -> &str {
    match ref_id
        .strip_prefix('@')
        .and_then(|rest| rest.rsplit_once(':'))
    {
        Some((_, local)) => local,
        None => ref_id.strip_prefix('@').unwrap_or(ref_id),
    }
}

fn hash_str(hasher: &mut Sha256, text: &str) {
    hasher.update(&(text.len() as u64).to_le_bytes());
    hasher.update(text.as_bytes());
}

fn hash_opt_str(hasher: &mut Sha256, text: Option<&str>) {
    match text {
        Some(value) => {
            hasher.update(&[1]);
            hash_str(hasher, value);
        }
        None => hasher.update(&[0]),
    }
}

fn hash_str_slice(hasher: &mut Sha256, items: &[String]) {
    hasher.update(&(items.len() as u64).to_le_bytes());
    for item in items {
        hash_str(hasher, item);
    }
}

fn hash_native_id(hasher: &mut Sha256, identifier: Option<&ElementIdentifier>) {
    match identifier {
        Some(id) => {
            hasher.update(&[1]);
            let kind = serde_json::to_string(&id.kind).unwrap_or_default();
            hash_str(hasher, &kind);
            hash_str(hasher, &id.value);
        }
        None => hasher.update(&[0]),
    }
}

fn hash_bounds(hasher: &mut Sha256, bounds: Option<Rect>) {
    match bounds {
        Some(rect) => {
            hasher.update(&[1]);
            for coord in [rect.x, rect.y, rect.width, rect.height] {
                let normalized = if coord == 0.0 { 0.0_f64 } else { coord };
                hasher.update(&normalized.to_bits().to_le_bytes());
            }
        }
        None => hasher.update(&[0]),
    }
}

#[cfg(test)]
#[path = "baseline_hash_tests.rs"]
mod tests;
