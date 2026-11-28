//! The native element token a Linux ref carries: the accessibility bus
//! instance, the owning process, the bus name, a fingerprint of the element's
//! identity fields and every object path from the window frame down to the
//! element. Resolution reads that chain again instead of searching the window,
//! and a live read of the element reproduces the token byte for byte only
//! while its chain and its identity both still hold.

use agent_desktop_core::{LocatorEvidence, LocatorField};

use super::AtspiElement;
use crate::tree::walk::walker_expand::UNKNOWN_PID;

const TOKEN_PREFIX: &str = "atspi1:";
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// A decoded token: the bus instance and process it was taken under, and the
/// object paths from the window frame (first) to the element (last).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElementToken {
    pub(crate) guid: String,
    pub(crate) pid: u32,
    pub(crate) bus: String,
    pub(crate) paths: Vec<String>,
}

impl ElementToken {
    /// The window frame path the chain starts at.
    pub(crate) fn frame(&self) -> &str {
        self.paths.first().map(String::as_str).unwrap_or_default()
    }

    /// Whether the token's last link is `element`'s address.
    pub(crate) fn names(&self, element: &AtspiElement) -> bool {
        self.bus == element.bus_name()
            && self.paths.last().map(String::as_str) == Some(element.path())
    }
}

/// The token of `element` on the bus instance `guid`, with `evidence` its
/// projected read. `None` when the chain is not rooted at the window frame,
/// leaves the frame's bus, the frame's process is unknown, or an identity
/// field was not read: such a read has nothing a later read could reproduce.
pub(crate) fn encode(
    guid: &str,
    element: &AtspiElement,
    evidence: &LocatorEvidence,
) -> Option<String> {
    let frame = element.frame()?;
    let chain = element.ancestry().chain();
    let rooted = match chain.first() {
        Some(first) => first == frame,
        None => element == frame,
    };
    let pid = frame.pid();
    if !rooted || guid.is_empty() || pid == UNKNOWN_PID || pid == 0 {
        return None;
    }
    let fingerprint = fingerprint(evidence)?;
    let links: Vec<&AtspiElement> = chain.iter().chain(std::iter::once(element)).collect();
    if links.iter().any(|link| link.bus_name() != frame.bus_name()) {
        return None;
    }
    let paths: Vec<&str> = links.iter().map(|link| link.path()).collect();
    let prefix = common_prefix(&paths);
    let suffixes: Vec<&str> = paths.iter().map(|path| &path[prefix.len()..]).collect();
    Some(format!(
        "{TOKEN_PREFIX}{guid}|{pid}|{}|{fingerprint:016x}|{prefix}|{}",
        frame.bus_name(),
        suffixes.join(",")
    ))
}

/// Reads a value [`encode`] produced. Anything else, including another
/// platform's token or a menu identity, is `None`.
pub(crate) fn decode(token: &str) -> Option<ElementToken> {
    let fields: Vec<&str> = token.strip_prefix(TOKEN_PREFIX)?.split('|').collect();
    let [guid, pid, bus, fingerprint, prefix, suffixes] = fields.as_slice() else {
        return None;
    };
    if guid.is_empty() || bus.is_empty() || fingerprint.len() != 16 || suffixes.is_empty() {
        return None;
    }
    Some(ElementToken {
        guid: (*guid).to_owned(),
        pid: pid.parse().ok()?,
        bus: (*bus).to_owned(),
        paths: suffixes
            .split(',')
            .map(|suffix| format!("{prefix}{suffix}"))
            .collect(),
    })
}

/// A hash of the fields resolution holds an element to: role, stable name,
/// description and native identifier. Values and bounds are never identity,
/// and neither is a name borrowed from a label when a native id is recorded.
fn fingerprint(evidence: &LocatorEvidence) -> Option<u64> {
    let role = evidence.role.known()?;
    let name = read(&evidence.name)?;
    let value = read(&evidence.value)?;
    let description = read(&evidence.description)?;
    if !evidence.identifiers.is_complete() {
        return None;
    }
    let native_id = evidence.identifiers.preferred_value();
    let name = agent_desktop_core::refs::ref_identity::stable_name(role, name, value)
        .filter(|_| !(evidence.identifiers.name_is_borrowed() && native_id.is_some()));
    let description = description.filter(|text| !text.trim().is_empty());
    let fields = [Some(role.as_str()), name, description, native_id];
    Some(fields.iter().fold(FNV_OFFSET, |hash, field| {
        let text = field.unwrap_or_default();
        let length = field.map_or(u64::MAX, |text| text.len() as u64);
        length
            .to_le_bytes()
            .into_iter()
            .chain(text.bytes())
            .fold(hash, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
            })
    }))
}

fn read(field: &LocatorField<String>) -> Option<Option<&str>> {
    match field {
        LocatorField::Known(text) => Some(Some(text.as_str())),
        LocatorField::Absent => Some(None),
        LocatorField::Unknown => None,
    }
}

fn common_prefix<'a>(paths: &[&'a str]) -> &'a str {
    let Some(first) = paths.first() else {
        return "";
    };
    let shared = paths.iter().skip(1).fold(first.len(), |shared, path| {
        first
            .bytes()
            .zip(path.bytes())
            .take(shared)
            .take_while(|(left, right)| left == right)
            .count()
    });
    first[..shared].rfind('/').map_or("", |cut| &first[..=cut])
}

#[cfg(test)]
#[path = "element_token_tests.rs"]
mod tests;
