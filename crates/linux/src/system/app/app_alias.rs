use crate::system::app::alias_cache::AliasCache;
use crate::system::launch::indexed_entry::IndexedEntry;
use std::path::PathBuf;
use std::sync::Arc;

use crate::system::launch::desktop_entry::parse;
use crate::system::launch::desktop_exec::{ExecContext, expand};
use crate::system::launch::desktop_lookup::{current_dirs, find};
use crate::system::process::executable_identity::{ExecutableIdentity, Naming};

/// The desktop entries of the machine, as the names each executable goes by.
pub(crate) struct AliasIndex {
    entries: Vec<IndexedEntry>,
}

impl AliasIndex {
    pub(in crate::system) fn build(files: &[(String, String)], dirs: &[PathBuf]) -> Self {
        let desktops = current_desktops();
        let desktops: Vec<&str> = desktops
            .split(':')
            .filter(|name| !name.is_empty())
            .collect();
        let entries = files
            .iter()
            .filter_map(|(id, text)| parse(id, text))
            .filter(|entry| entry.traits.shown_in(&desktops))
            .map(|entry| {
                let argv = entry
                    .exec
                    .as_deref()
                    .map(|exec| {
                        let file =
                            find(&entry.id, dirs).map(|file| file.to_string_lossy().into_owned());
                        if exec.contains("%k") && file.is_none() {
                            return Vec::new();
                        }
                        expand(
                            exec,
                            &ExecContext {
                                arguments: &[],
                                icon: entry.icon.as_deref(),
                                name: &entry.name,
                                file: file.as_deref(),
                            },
                        )
                    })
                    .unwrap_or_default();
                IndexedEntry {
                    id: entry.id,
                    name: entry.name,
                    argv,
                }
            })
            .collect();
        Self { entries }
    }

    /// The `Name` of every shown entry that names the verified process. Two
    /// applications may share a name; core reports that as ambiguous.
    pub(crate) fn names_for(&self, actual: &ExecutableIdentity) -> Vec<String> {
        self.naming(actual)
            .into_iter()
            .map(|entry| entry.name.clone())
            .collect()
    }

    /// The id of every shown entry that names the verified process.
    pub(crate) fn ids_for(&self, actual: &ExecutableIdentity) -> Vec<String> {
        self.naming(actual)
            .into_iter()
            .map(|entry| entry.id.clone())
            .collect()
    }

    fn naming<'a>(&'a self, actual: &ExecutableIdentity) -> Vec<&'a IndexedEntry> {
        let path = crate::system::launch::launch_search::Search::from_env().path_dirs;
        let named: Vec<(Naming, &IndexedEntry)> = self
            .entries
            .iter()
            .filter_map(|entry| {
                let wanted = ExecutableIdentity::requested(&entry.argv, &path)?;
                Some((wanted.naming(actual)?, entry))
            })
            .collect();
        let best = named.iter().map(|(naming, _)| *naming).max_by_key(rank);
        let kept: Vec<&IndexedEntry> = named
            .iter()
            .filter(|(naming, _)| Some(*naming) == best)
            .map(|(_, entry)| *entry)
            .collect();
        let one_command = kept.windows(2).all(|pair| pair[0].argv == pair[1].argv);
        if one_command { kept } else { Vec::new() }
    }

    /// The id of every entry whose `Name` equals `name`, ignoring case.
    pub(crate) fn ids_named(&self, name: &str) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| entry.name.eq_ignore_ascii_case(name))
            .map(|entry| entry.id.clone())
            .collect()
    }
}

fn rank(naming: &Naming) -> usize {
    match naming {
        Naming::Launcher => 0,
        Naming::Exact(arguments) => 1 + arguments,
    }
}

/// The desktops the user is in, which decide the entries `OnlyShowIn` and
/// `NotShowIn` show; the cache rebuilds the index when they change.
pub(crate) fn current_desktops() -> String {
    std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default()
}

static CACHE: AliasCache = AliasCache::new();

/// The index of the entries the process environment names.
pub(crate) fn current_index() -> Arc<AliasIndex> {
    CACHE.index(&current_dirs())
}

#[cfg(test)]
#[path = "app_alias_tests.rs"]
mod tests;
