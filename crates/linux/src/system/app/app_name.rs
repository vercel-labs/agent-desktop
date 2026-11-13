use super::app_alias::{AliasIndex, current_index};
use crate::system::process::executable_identity::ExecutableIdentity;
use crate::system::process::process_facts::ProcessFacts;

fn non_blank(text: Option<&str>) -> Option<&str> {
    text.filter(|text| !text.trim().is_empty())
}

const DELETED_SUFFIX: &str = " (deleted)";

/// An executable's basename without the marker the kernel appends to the
/// link target of a binary that was removed or replaced while it runs, so an
/// application keeps its name across a package upgrade.
fn live_basename(basename: &str) -> &str {
    basename.strip_suffix(DELETED_SUFFIX).unwrap_or(basename)
}

/// The name an application is listed under: the executable's basename,
/// then `comm`, which the kernel cuts at 15 characters. It comes from `/proc`
/// alone, never from the accessibility root, so an application keeps one name
/// whether it answers or is stopped: core upgrades a stale reference to an
/// unresponsive application only when the listed name is unchanged.
pub(crate) fn app_name(facts: &ProcessFacts) -> String {
    non_blank(facts.exe_basename.as_deref())
        .map(live_basename)
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(&facts.comm)
        .to_owned()
}

/// The desktop entry names and ids the process's executable answers to. The
/// listed name never follows them.
pub(crate) fn aliases_of(facts: &ProcessFacts) -> Vec<String> {
    aliases_in(&current_index(), facts)
}

/// The desktop id of the process when exactly one shown entry names it.
pub(crate) fn desktop_id_in(index: &AliasIndex, facts: &ProcessFacts) -> Option<String> {
    let mut ids = index.ids_for(facts.executable.as_ref()?).into_iter();
    match (ids.next(), ids.next()) {
        (Some(id), None) => Some(id),
        _ => None,
    }
}

pub(crate) fn desktop_id(facts: &ProcessFacts) -> Option<String> {
    desktop_id_in(&current_index(), facts)
}

/// As [`aliases_of`] over an index the caller already holds, for a caller that
/// asks about many processes.
pub(crate) fn aliases_in(index: &AliasIndex, facts: &ProcessFacts) -> Vec<String> {
    aliases_of_identity(index, facts.executable.as_ref())
}

/// The desktop entry names and ids that name this executable identity, read
/// the way [`aliases_of`] reads them for a running process. A launch request
/// carries the same evidence, so its resolved aliases compare like a
/// resident's.
pub(crate) fn aliases_of_identity(
    index: &AliasIndex,
    identity: Option<&ExecutableIdentity>,
) -> Vec<String> {
    let Some(actual) = identity else {
        return Vec::new();
    };
    let mut aliases = index.names_for(actual);
    aliases.extend(index.ids_for(actual));
    aliases
}

#[cfg(test)]
#[path = "app_name_tests.rs"]
mod tests;
