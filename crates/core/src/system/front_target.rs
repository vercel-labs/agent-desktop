use super::front_process::FrontProcess;
use super::front_window::FrontWindow;

/// How far before the capture a process may have started and still count as
/// started after it: process start times are read with a resolution of a
/// second on some platforms.
const START_SLACK_MS: u64 = 1_000;

/// The process a headless command acts on. A window is the target's when its
/// owner is the target process, or when it is new and its owner started after
/// the capture and runs the target's executable: an application that forks
/// and detaches opens its window from a new process of the same program.
/// Every other window belongs to the user.
#[derive(Debug, Clone, Default)]
pub(crate) struct FrontTarget {
    pub pid: Option<u32>,
    pub image: Option<String>,
}

impl FrontTarget {
    /// A launch names the program it starts, not yet a process.
    pub(crate) fn program(id: &str) -> Self {
        Self {
            pid: None,
            image: Some(id.to_owned()),
        }
    }

    /// A command on a running process: its windows, and the new windows of
    /// processes that start after the capture and run its program.
    pub(crate) fn of_process(adapter: &dyn crate::adapter::PlatformAdapter, pid: u32) -> Self {
        Self {
            pid: Some(pid),
            image: adapter.front_process(pid).map(|process| process.image),
        }
    }

    pub(crate) fn owns(
        &self,
        window: &FrontWindow,
        is_new: bool,
        process: impl FnOnce() -> Option<FrontProcess>,
        captured_ms: u64,
    ) -> bool {
        if self.pid == Some(window.pid) {
            return true;
        }
        let (true, Some(image)) = (is_new, self.image.as_deref()) else {
            return false;
        };
        process().is_some_and(|process| {
            process.started_ms + START_SLACK_MS >= captured_ms
                && same_program(&process.image, image)
        })
    }
}

/// The name a program is known by: the last path component without a
/// `.exe` suffix, lower case, and for a reverse-DNS desktop id its last part.
fn program_name(text: &str) -> String {
    let base = text.rsplit(['/', '\\']).next().unwrap_or(text);
    let lower = base.to_ascii_lowercase();
    let bare = lower.strip_suffix(".exe").unwrap_or(&lower);
    if bare.matches('.').count() >= 2 {
        return bare.rsplit('.').next().unwrap_or(bare).to_owned();
    }
    bare.to_owned()
}

fn same_program(left: &str, right: &str) -> bool {
    let (left, right) = (program_name(left), program_name(right));
    !left.is_empty() && left == right
}

#[cfg(test)]
#[path = "front_target_tests.rs"]
mod tests;
