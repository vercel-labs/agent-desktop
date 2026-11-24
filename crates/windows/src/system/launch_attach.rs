use std::collections::HashSet;
use std::path::{Path, PathBuf};

use agent_desktop_core::{
    AdapterError, Deadline, ErrorCode, ProcessId, system::launch_options::LaunchOptions,
    system::launch_result::LaunchResult,
};

use super::super::app_ops::{ProcessRow, process_snapshot};
use super::super::launch_path::{image_file_name, resolve_executable};
use super::super::process_identity;

pub(super) fn prepare(
    id: &str,
    options: &LaunchOptions,
    deadline: Deadline,
) -> Result<(PathBuf, Option<LaunchResult>), AdapterError> {
    let matches = matching_processes(image_file_name(id))?;
    if !options.attach_if_running {
        if let Some(row) = matches.first() {
            return Err(super::already_running_error(row.pid, &matches));
        }
        return Ok((resolve_executable(id)?, None));
    }

    let executable = resolve_executable(id)?;
    let mut exact_matches = Vec::new();
    for row in matches {
        let Some(process) = ProcessPin::open(row.pid) else {
            continue;
        };
        let Some(image_path) = process.image_path() else {
            continue;
        };
        if same_executable_path(&executable, &image_path) {
            let token = process_identity::token_for_pid(row.pid)?.ok_or_else(|| {
                AdapterError::new(
                    ErrorCode::AppUnresponsive,
                    "Running process has no exact process instance token",
                )
            })?;
            exact_matches.push((row, token, process));
        }
    }
    // Chromium/Electron apps can run their main process and helper processes
    // from the same executable. Only the root process is the launchable app;
    // child helpers must not turn one running app into an ambiguous target.
    // Keep separate roots ambiguous so we never guess between app instances.
    let roots = same_image_roots(
        &exact_matches
            .iter()
            .map(|(row, _, _)| row.clone())
            .collect::<Vec<_>>(),
    );
    if !roots.is_empty() {
        exact_matches.retain(|(row, _, _)| roots.contains(&u32::from(row.pid)));
    }
    if exact_matches.len() > 1 {
        let rows = exact_matches
            .iter()
            .map(|(row, _, _)| row.clone())
            .collect::<Vec<_>>();
        return Err(super::ambiguous_apps(&rows));
    }
    if let Some((row, token, _process)) = exact_matches.pop() {
        let window = super::observe_window(row.pid, &token, options, deadline)?;
        return Ok((
            executable,
            Some(super::launch_result(id, row.pid, token, window)),
        ));
    }
    Ok((executable, None))
}

fn same_image_roots(matches: &[ProcessRow]) -> HashSet<u32> {
    let candidate_pids: HashSet<u32> = matches.iter().map(|row| u32::from(row.pid)).collect();
    let child_pids: HashSet<u32> = matches
        .iter()
        .filter(|row| {
            row.parent_pid
                .is_some_and(|parent_pid| candidate_pids.contains(&u32::from(parent_pid)))
        })
        .map(|row| u32::from(row.pid))
        .collect();
    let roots: HashSet<u32> = candidate_pids.difference(&child_pids).copied().collect();

    // A malformed or stale parent graph must preserve the existing fail-closed
    // ambiguity behavior instead of hiding every candidate.
    if roots.is_empty() {
        candidate_pids
    } else {
        roots
    }
}

pub(super) fn matching_processes(image: &str) -> Result<Vec<ProcessRow>, AdapterError> {
    Ok(process_snapshot()?
        .into_iter()
        .filter(|row| row.name.eq_ignore_ascii_case(image))
        .collect())
}

fn same_executable_path(expected: &Path, actual: &Path) -> bool {
    std::fs::canonicalize(expected)
        .ok()
        .zip(std::fs::canonicalize(actual).ok())
        .is_some_and(|(expected, actual)| expected == actual)
}

#[cfg(target_os = "windows")]
struct ProcessPin(windows_sys::Win32::Foundation::HANDLE);

#[cfg(target_os = "windows")]
impl ProcessPin {
    fn open(pid: ProcessId) -> Option<Self> {
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };

        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, u32::from(pid)) };
        (!handle.is_null()).then_some(Self(handle))
    }

    fn image_path(&self) -> Option<PathBuf> {
        use std::os::windows::ffi::OsStringExt;
        use windows_sys::Win32::System::Threading::QueryFullProcessImageNameW;

        let mut buffer = vec![0_u16; 32_768];
        let mut size = buffer.len() as u32;
        let ok = unsafe { QueryFullProcessImageNameW(self.0, 0, buffer.as_mut_ptr(), &mut size) };
        if ok == 0 || size == 0 {
            return None;
        }
        Some(PathBuf::from(std::ffi::OsString::from_wide(
            &buffer[..size as usize],
        )))
    }
}

#[cfg(target_os = "windows")]
impl Drop for ProcessPin {
    fn drop(&mut self) {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0) };
    }
}

#[cfg(not(target_os = "windows"))]
struct ProcessPin;

#[cfg(not(target_os = "windows"))]
impl ProcessPin {
    fn open(_pid: ProcessId) -> Option<Self> {
        None
    }

    fn image_path(&self) -> Option<PathBuf> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(pid: u32, parent_pid: Option<u32>) -> ProcessRow {
        ProcessRow {
            pid: ProcessId::from(pid),
            parent_pid: parent_pid.map(ProcessId::from),
            name: "app.exe".to_string(),
        }
    }

    #[test]
    fn same_image_helpers_collapse_to_their_root_process() {
        let matches = [
            process(10, None),
            process(11, Some(10)),
            process(12, Some(11)),
        ];

        assert_eq!(same_image_roots(&matches), HashSet::from([10]));
    }

    #[test]
    fn separate_same_image_roots_remain_ambiguous_candidates() {
        let matches = [process(10, None), process(20, None), process(21, Some(20))];

        assert_eq!(same_image_roots(&matches), HashSet::from([10, 20]));
    }

    #[test]
    fn a_cyclic_parent_snapshot_preserves_all_candidates() {
        let matches = [process(10, Some(11)), process(11, Some(10))];

        assert_eq!(same_image_roots(&matches), HashSet::from([10, 11]));
    }
}
