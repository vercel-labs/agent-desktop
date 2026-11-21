use std::collections::HashSet;
use std::time::Duration;

use agent_desktop_core::system::launch_options::LaunchOptions;
use agent_desktop_core::system::launch_result::LaunchResult;
use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionLease, LaunchGuidance,
    ProcessId, RendererKind, WindowInfo,
};

use self::launch_arrival::Arrival;
use self::launch_attach::{Decision, decide};
use self::launch_conclude::{FocusAnswer, conclude};
use self::launch_observe::{POLL_INTERVAL, Spawned, find_owner, pids_of, running_of};
use self::launch_search::Search;
use self::launch_spawn::{before_spawn, spawn, validate};
use self::launch_target::{Plan, resolve};
use super::SharedBus;
use crate::system::app::app_alias::current_index;
use crate::system::focus::focus_window::focus_window;
use crate::system::process::liveness::{Liveness, liveness};
use crate::system::process::process_facts;
use crate::system::process::running_app::Running;
use crate::system::process::web_family::WebFamily;
use crate::system::registry::registry_frames::RootRead;
use crate::system::window::native_group::is_unproven;
use crate::system::window::window_ops::{require_complete_frames, windows_of_process};

#[path = "launch_baseline.rs"]
mod baseline;

fn bounded(deadline: Deadline, timeout_ms: u64) -> Deadline {
    if timeout_ms == 0 {
        deadline
    } else {
        deadline.capped(Duration::from_millis(timeout_ms))
    }
}

fn chromium_renderer(bus: &SharedBus, pid: u32, deadline: Deadline) -> Option<RendererKind> {
    super::registry::read_registry_for(bus, Some(pid), deadline)
        .ok()?
        .apps
        .iter()
        .any(|app| {
            crate::tree::properties::toolkit_trust::web_family(&app.toolkit)
                == Some(WebFamily::Chromium)
        })
        .then_some(RendererKind::Chromium)
}

/// Refreshes the same process-owned window after successful focus when a complete read is available.
fn same_window(
    observation: Result<(Vec<WindowInfo>, RootRead), AdapterError>,
    expected: &WindowInfo,
) -> Option<WindowInfo> {
    let (windows, read) = observation.ok()?;
    if read != RootRead::Read {
        return None;
    }
    windows.into_iter().find(|window| {
        window.id == expected.id
            && window.pid == expected.pid
            && window.process_instance == expected.process_instance
    })
}

/// The refusal for an application that was on the registry and left before it
/// showed a window: reporting its pid as a launch would hand the caller a dead
/// process.
fn exited_early(owner: &Running, liveness: Liveness) -> Result<(), AdapterError> {
    if liveness != Liveness::Gone {
        return Ok(());
    }
    Err(AdapterError::new(
        ErrorCode::ActionFailed,
        format!(
            "'{}' (pid {}) exited before it showed a window",
            owner.name, owner.pid
        ),
    )
    .with_suggestion("Run the program from a terminal to see why it stopped, then launch it again.")
    .with_disposition(DeliverySemantics::delivered_unverified()))
}

/// The refusal a launch ends on when the deadline passed with no window
/// proven. An application the launch started has been delivered to, whatever
/// the read could prove.
fn unproven_at_deadline(error: AdapterError, started: bool) -> AdapterError {
    let error = error.with_suggestion(
        "Run 'list-windows' to see whether the window of the application is open.",
    );
    if started {
        error.with_disposition(DeliverySemantics::delivered_unverified())
    } else {
        before_spawn(error)
    }
}

/// The window a launch reports among the windows of its application: one
/// that was not open before the launch, or, for an application the launch
/// started, its first. A window that was already open is not the launch's.
fn choose_window(own: Vec<WindowInfo>, known: &HashSet<String>) -> Option<WindowInfo> {
    own.into_iter().find(|window| !known.contains(&window.id))
}

/// How long a launch that was not asked to bring the window forward waits for
/// the first window of an application it started.
const WINDOW_GRACE: Duration = Duration::from_millis(2_000);

/// How long such a launch waits for the new window of an application that
/// was already running and forwarded the request.
const NEW_WINDOW_WAIT: Duration = Duration::from_millis(5_000);

struct Launch<'a> {
    bus: &'a SharedBus,
    id: &'a str,
    options: &'a LaunchOptions,
    lease: &'a InteractionLease,
    deadline: Deadline,
}

impl Launch<'_> {
    fn window_of(
        &self,
        owner: &Running,
        waits: bool,
        known: &HashSet<String>,
    ) -> Result<Option<WindowInfo>, AdapterError> {
        self.wait_for_window(owner, waits, known, || {
            windows_of_process(self.bus, owner.pid, self.deadline)
        })
    }

    /// Reads the owner's windows until one is reported. `waits` is true for an
    /// application this launch spawned: the read goes on for a window and ends
    /// if the owner exits. An attach passes false and reads only until it has
    /// an answer. A read that could not prove a window is read again until the
    /// launch deadline.
    fn wait_for_window(
        &self,
        owner: &Running,
        waits: bool,
        known: &HashSet<String>,
        mut read: impl FnMut() -> Result<(Vec<WindowInfo>, RootRead), AdapterError>,
    ) -> Result<Option<WindowInfo>, AdapterError> {
        let limit = self.wait_limit(known);
        loop {
            let observed = read().and_then(|(windows, status)| {
                let own: Vec<WindowInfo> = windows
                    .into_iter()
                    .filter(|window| {
                        window.pid.get() == owner.pid
                            && window.process_instance.as_deref() == Some(owner.instance.as_str())
                    })
                    .collect();
                let window = choose_window(own, known);
                if window.is_none() {
                    require_complete_frames(status)?;
                }
                Ok(window)
            });
            let unproven = match observed {
                Ok(Some(window)) => return Ok(Some(window)),
                Ok(None) => None,
                Err(error)
                    if is_unproven(&error)
                        || matches!(
                            error.code,
                            ErrorCode::Timeout
                                | ErrorCode::AppUnresponsive
                                | ErrorCode::AppNotFound
                                | ErrorCode::WindowNotFound
                        ) =>
                {
                    Some(error)
                }
                Err(error) => {
                    return Err(error.with_disposition(DeliverySemantics::delivered_unverified()));
                }
            };
            if waits {
                exited_early(owner, liveness(owner.pid, &owner.instance))?;
            }
            let until = if unproven.is_some() {
                self.deadline
            } else {
                limit
            };
            if until.is_expired() || (unproven.is_none() && !waits) {
                return unproven.map_or(Ok(None), |error| Err(unproven_at_deadline(error, waits)));
            }
            std::thread::sleep(POLL_INTERVAL.min(until.remaining()));
        }
    }

    fn wait_limit(&self, known: &HashSet<String>) -> Deadline {
        if self.options.activate {
            return self.deadline;
        }
        let wait = if known.is_empty() {
            WINDOW_GRACE
        } else {
            NEW_WINDOW_WAIT
        };
        self.deadline.capped(wait)
    }

    fn conclude_with(
        &self,
        owner: &Running,
        window: Option<WindowInfo>,
        arrival: Arrival,
    ) -> LaunchResult {
        let renderer = chromium_renderer(self.bus, owner.pid, self.deadline);
        let mut result = conclude(owner, window, self.options, arrival, |window| {
            let activation = focus_window(self.bus, window, self.lease)?;
            Ok(FocusAnswer {
                window: same_window(
                    windows_of_process(self.bus, owner.pid, self.deadline),
                    window,
                ),
                dispatched: activation.dispatched(),
            })
        });
        result.guidance.renderer = renderer;
        result
    }

    /// A running application is reported with its window. One that shows no
    /// window, such as a file manager that stays resident after its last
    /// window closed, is started again so that its launcher forwards to it
    /// and a window opens.
    fn attach(
        &self,
        owner: &Running,
        plan: &Plan,
        running: &[Running],
    ) -> Result<LaunchResult, AdapterError> {
        match self.window_of(owner, false, &HashSet::new())? {
            Some(window) => Ok(self.conclude_with(owner, Some(window), Arrival::Attached)),
            None => self.start(plan, running),
        }
    }

    fn report_unlisted(&self, spawned: &Spawned) -> LaunchResult {
        let facts = process_facts::read(spawned.pid);
        LaunchResult {
            app: facts
                .as_ref()
                .map_or_else(|| self.id.to_owned(), super::listed_app_name),
            pid: ProcessId::new(spawned.pid),
            process_instance: facts.map(|facts| facts.instance),
            window: None,
            guidance: LaunchGuidance {
                suggestion: Some(
                    "The process started but is not on the accessibility bus yet. Its pid is the spawned process's and may be a wrapper of the application. Run 'list-apps' to see whether it appears.".to_owned(),
                ),
                ..LaunchGuidance::default()
            },
            disposition: Arrival::Unlisted.facts().disposition(),
        }
    }

    fn start(&self, plan: &Plan, running: &[Running]) -> Result<LaunchResult, AdapterError> {
        let before = baseline::prepare(self.bus, &plan.identity, running, self.deadline);
        self.deadline.ensure_remaining().map_err(before_spawn)?;
        let mut spawned = spawn(self.id, plan, self.options)?;
        match find_owner(self.bus, &mut spawned, plan, &before, self.deadline)? {
            Some(owner) => {
                let known = before
                    .residents
                    .iter()
                    .find(|resident| resident.owner.identity() == owner.identity());
                let window = match known {
                    Some(resident) => self.window_of(&owner, true, &resident.windows)?,
                    None if before.processes.contains(&owner.identity()) => None,
                    None => self.window_of(&owner, true, &HashSet::new())?,
                };
                let arrival = Arrival::started(
                    owner.pid == spawned.pid,
                    !before.processes.contains(&owner.identity()),
                );
                Ok(self.conclude_with(&owner, window, arrival))
            }
            None => Ok(self.report_unlisted(&spawned)),
        }
    }
}

/// Starts an application by command name, absolute path, desktop id or
/// desktop entry name, or attaches to the running one, and reports the pid of
/// the application that owns the windows with its first window. The attach
/// decision is made before anything is spawned.
pub(crate) fn launch_app(
    bus: &SharedBus,
    id: &str,
    options: &LaunchOptions,
    lease: &InteractionLease,
) -> Result<LaunchResult, AdapterError> {
    validate(options)?;
    let deadline = bounded(lease.deadline(), options.timeout_ms);
    let search = Search::from_env();
    let plan = resolve(id, &options.args, &search, &current_index())?;
    let registry = crate::system::registry::read_processes(bus, deadline).map_err(before_spawn)?;
    let running = running_of(&registry).map_err(before_spawn)?;
    let launch = Launch {
        bus,
        id,
        options,
        lease,
        deadline,
    };
    match decide(
        &plan.identity,
        &running,
        options.attach_if_running,
        !options.args.is_empty(),
    ) {
        Decision::Refuse(pids) => Err(before_spawn(
            AdapterError::new(
                ErrorCode::ActionFailed,
                format!("'{id}' is already running (pids {pids:?}) and a new launch was required"),
            )
            .with_suggestion("Allow attaching to use the running instance, or close it first."),
        )),
        Decision::Ambiguous(pids) => Err(AdapterError::ambiguous_process_target(
            format!("'{id}' matches several running applications"),
            &pids_of(&pids),
        )),
        Decision::Attach(owner) => launch.attach(&owner, &plan, &running),
        Decision::Forward | Decision::Spawn => launch.start(&plan, &running),
    }
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "launch_window_tests.rs"]
mod window_tests;

pub(super) mod desktop_entry;
pub(crate) mod desktop_entry_traits;
pub(super) mod desktop_exec;
pub(super) mod desktop_lookup;
pub(super) mod desktop_stamp;
pub(super) mod indexed_entry;
mod launch_arrival;
pub(super) mod launch_attach;
mod launch_conclude;
mod launch_handoff;
pub(super) mod launch_observe;
pub(crate) mod launch_sample;
pub(crate) mod launch_search;
mod launch_spawn;
pub(crate) mod launch_step;
pub(super) mod launch_target;
