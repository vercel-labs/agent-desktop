//! Whether an AppKit inventory that skipped records is still complete.

use super::{inventory_error, records};
use agent_desktop_core::AdapterError;
use serde_json::Value;
use std::cell::RefCell;

/// What a caller concludes is absent when the inventory finds nothing for it.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Wanted<'a> {
    /// Any app: a full inventory, a process, the frontmost app or a shell surface.
    Any,
    /// The app a lookup names, optionally pinned to a bundle ID.
    App {
        name: &'a str,
        bundle_id: Option<&'a str>,
    },
}

impl<'a> From<&'a str> for Wanted<'a> {
    /// The app a lookup names, by display name, bundle ID or executable name.
    fn from(name: &'a str) -> Self {
        Self::App {
            name,
            bundle_id: None,
        }
    }
}

impl Wanted<'_> {
    /// Whether `record` could be the app the caller wants. `Any` wants every
    /// record. A lookup without a pinned bundle ID compares its text, whatever
    /// the text looks like, with an app's display name, bundle ID and
    /// executable name; a pinned lookup compares the text with the display
    /// name and the pin with the bundle ID. A record that lacks one of those
    /// three fields may lack the one that matches, so it could be the wanted
    /// app; both kinds of lookup need all three, which keeps one rule. A record
    /// with all three could be the wanted app when one of them carries the
    /// text, the stem of an app path, or the pin.
    fn could_be(self, record: &ProcesslessRecord) -> bool {
        let Self::App { name, bundle_id } = self else {
            return true;
        };
        if !record.names_every_alias() {
            return true;
        }
        let path_stem = (name.contains('/') || name.to_ascii_lowercase().ends_with(".app"))
            .then(|| std::path::Path::new(name).file_stem()?.to_str())
            .flatten();
        [Some(name), path_stem, bundle_id]
            .into_iter()
            .flatten()
            .any(|identifier| record.is_named(identifier))
    }
}

/// What identifies a record the bridge skipped because its
/// `NSRunningApplication` has no process (`pid <= 0`). The index is not part
/// of it, because it shifts between reads.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ProcesslessRecord {
    pid: i64,
    terminated: bool,
    name: Option<String>,
    bundle_id: Option<String>,
    executable: Option<String>,
    launch_time: Option<u64>,
}

impl ProcesslessRecord {
    /// The record, or `None` for any other skipped record and for one whose
    /// fields are missing or malformed.
    fn from_skipped(record: &Value) -> Option<Self> {
        if record["field"] != "pid" {
            return None;
        }
        let pid = record["pid"].as_i64().filter(|pid| *pid <= 0)?;
        let text = |key: &str| match &record[key] {
            Value::Null => Some(None),
            Value::String(value) => Some(Some(value.clone())),
            _ => None,
        };
        let launch_time = match &record["launch_time"] {
            Value::Null => None,
            time => Some(
                time.as_f64()
                    .filter(|time| time.is_finite() && *time > 0.0)?
                    .to_bits(),
            ),
        };
        Some(Self {
            pid,
            terminated: record["terminated"].as_bool()?,
            name: text("name")?,
            bundle_id: text("bundle_id")?,
            executable: text("executable")?,
            launch_time,
        })
    }

    /// A terminated record belongs to no process now or later: relaunching
    /// the app creates a new record. A live one may belong to a running app
    /// whose pid AppKit never bound, or to an app that is still launching, so
    /// it may pass only when what it says about its app rules out the app the
    /// caller is about to call absent.
    fn may_pass(&self, wanted: Wanted<'_>) -> bool {
        self.terminated || !wanted.could_be(self)
    }

    /// Whether the record carries a display name, a bundle ID and an
    /// executable name, none of them blank.
    fn names_every_alias(&self) -> bool {
        [&self.name, &self.bundle_id, &self.executable]
            .into_iter()
            .all(|alias| {
                alias
                    .as_deref()
                    .is_some_and(|alias| !alias.trim().is_empty())
            })
    }

    fn is_named(&self, identifier: &str) -> bool {
        self.name.as_deref().is_some_and(|name| {
            name.eq_ignore_ascii_case(identifier)
                || agent_desktop_core::app_name_matches(name, identifier)
        }) || [&self.bundle_id, &self.executable]
            .into_iter()
            .flatten()
            .any(|alias| alias.eq_ignore_ascii_case(identifier))
    }
}

/// One inventory operation: a loop that reads AppKit until it gets a stable
/// answer. Only its own reads can show that a processless record is stable.
#[derive(Default)]
struct Operation {
    /// The processless records of the previous completeness check, or `None`
    /// when there was none or it held a record that could not pass.
    previous: Option<Vec<ProcesslessRecord>>,
}

thread_local! {
    /// The inventory operation running on this thread, if any.
    static OPERATION: RefCell<Option<Operation>> = const { RefCell::new(None) };
}

/// Runs `read` as one inventory operation. Records seen before it started, by
/// an earlier operation or by an enclosing one, never count as stable inside
/// it, and its own reads end with it, even when `read` panics.
pub(crate) fn inventory_operation<T>(read: impl FnOnce() -> T) -> T {
    struct Restore(Option<Operation>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let outer = self.0.take();
            let _ = OPERATION.try_with(|operation| *operation.borrow_mut() = outer);
        }
    }
    let _restore =
        Restore(OPERATION.with(|operation| operation.replace(Some(Operation::default()))));
    read()
}

/// An inventory that skipped records is incomplete. The only exception is a
/// set of processless records that may pass for `wanted` (see
/// [`ProcesslessRecord::may_pass`]) and that is identical, record by record,
/// to the set of the previous completeness check in the same inventory
/// operation. No command can act on such a record, because the adapter
/// reaches an app through its pid, and treating it as churn would make every
/// inventory-dependent command time out until it disappears. Outside an
/// operation no read can be compared, so any skipped record fails.
pub(crate) fn require_complete(skipped: &[Value], wanted: Wanted<'_>) -> Result<(), AdapterError> {
    OPERATION.with(|operation| match operation.borrow_mut().as_mut() {
        Some(operation) => require_complete_with(skipped, wanted, &mut operation.previous),
        None => require_complete_with(skipped, wanted, &mut None),
    })
}

fn require_complete_with(
    skipped: &[Value],
    wanted: Wanted<'_>,
    previous: &mut Option<Vec<ProcesslessRecord>>,
) -> Result<(), AdapterError> {
    let current = skipped
        .iter()
        .map(|record| {
            ProcesslessRecord::from_skipped(record).filter(|record| record.may_pass(wanted))
        })
        .collect::<Option<Vec<_>>>()
        .map(|mut records| {
            records.sort_unstable();
            records
        });
    let stable = current.is_some() && *previous == current;
    *previous = current;
    if skipped.is_empty() || stable {
        return Ok(());
    }
    Err(records::with_skipped(
        inventory_error("AppKit application inventory skipped records"),
        skipped,
    ))
}

#[cfg(test)]
#[path = "workspace_apps_complete_tests.rs"]
mod tests;
