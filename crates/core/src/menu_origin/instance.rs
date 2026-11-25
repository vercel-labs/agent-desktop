//! The identity of one appearance of an application-owned menu.
//!
//! Only a platform element token identifies a menu appearance. Two openings of
//! a menu look identical — same position, same items, same states — so anything
//! derived from what the menu shows can never tell them apart, and a menu whose
//! label changed is still the same menu. A platform that cannot name the
//! element therefore cannot prove which opening a caller is looking at, and
//! reports nothing instead.

use crate::AccessibilityNode;

const INSTANCE_PREFIX: &str = "mnu1:";
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// One appearance of a menu, named by the element the platform returned for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MenuInstance {
    id: String,
}

impl MenuInstance {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }
}

/// The instance of the menu `tree` describes, when the platform named its
/// element.
///
/// A tree without an element token answers `None`: a menu this driver cannot
/// name cannot be proven to be the one a delivery opened.
pub(crate) fn of_tree(tree: &AccessibilityNode) -> Option<MenuInstance> {
    let token = tree
        .identity
        .retained_object
        .as_deref()
        .filter(|token| !token.is_empty())?;
    let mut bytes = Vec::with_capacity(token.len() + 6);
    bytes.extend_from_slice(b"token\n");
    bytes.extend_from_slice(token.as_bytes());
    Some(MenuInstance {
        id: format!("{INSTANCE_PREFIX}{:016x}", digest(&bytes)),
    })
}

/// Whether `value` is an instance id this module issues.
pub(crate) fn is_instance_id(value: &str) -> bool {
    value
        .strip_prefix(INSTANCE_PREFIX)
        .is_some_and(|hex| hex.len() == 16 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

/// A stable 64-bit digest, so ids never depend on the process that made them.
pub(crate) fn digest(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
