//! What the front guard reads and does on macOS: the frontmost application, the
//! window order from CoreGraphics (front to back), and a re-activation that
//! raises the user's windows over an intruder.

use agent_desktop_core::{AdapterError, Deadline, FrontProcess, FrontView, FrontWindow};
use core_foundation::string::CFString;

use super::cg_window::{WindowDictionary, bool_field, required_int_field};
use crate::system::app::workspace_apps::focused_owner_until;
use crate::system::process::process_identity::ProcessIdentity;

const GIVE_BACK_BUDGET_MS: u64 = 1_500;

/// One window of the screen's order: its id, its owner and whether it is on
/// screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OrderedWindow {
    pub(crate) number: i64,
    pub(crate) pid: i32,
    pub(crate) on_screen: bool,
}

pub(crate) fn read_front(deadline: Deadline) -> Result<FrontView, AdapterError> {
    let frontmost = focused_owner_until(deadline)?.map(|owner| owner.pid);
    let order = ordered_windows(onscreen_dictionaries()?)?;
    Ok(view_of(&order, frontmost))
}

/// The front view of a window order that runs front to back. The focused
/// window is the frontmost application's topmost window on screen.
pub(crate) fn view_of(order: &[OrderedWindow], frontmost: Option<i32>) -> FrontView {
    let window_of = |window: &OrderedWindow| FrontWindow {
        id: window_id(window.number),
        pid: u32::try_from(window.pid).unwrap_or_default(),
    };
    let mut on_screen = order.iter().filter(|window| window.on_screen);
    FrontView {
        focused: frontmost
            .and_then(|pid| {
                order
                    .iter()
                    .find(|window| window.on_screen && window.pid == pid)
            })
            .map(window_of),
        top: on_screen.next().map(window_of),
        windows: order
            .iter()
            .map(|window| window_id(window.number))
            .collect(),
        owners: order
            .iter()
            .map(|window| u32::try_from(window.pid).unwrap_or_default())
            .collect(),
    }
}

/// The windows on screen, front to back. The all-windows list holds every
/// window of every space and costs about 12 ms; this one costs well under a
/// millisecond, which matters because the guard reads it every few
/// milliseconds. A window that leaves the screen, such as one the user
/// minimizes, is not in it, and the guard then stops watching it.
fn onscreen_dictionaries() -> Result<Vec<WindowDictionary>, AdapterError> {
    use crate::tree::properties::cf_type::borrowed_cf_dictionary;
    use core_graphics::display::CGDisplay;
    use core_graphics::window::{
        kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly,
    };

    let list = CGDisplay::window_list_info(
        kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
        None,
    )
    .ok_or_else(|| AdapterError::internal("CoreGraphics returned no on-screen window list"))?;
    list.get_all_values()
        .into_iter()
        .map(|raw| {
            borrowed_cf_dictionary(raw as core_foundation::base::CFTypeRef).ok_or_else(|| {
                AdapterError::internal("CoreGraphics returned a non-dictionary window")
            })
        })
        .collect()
}

fn window_id(number: i64) -> String {
    format!("w-{number}")
}

fn ordered_windows(
    dictionaries: Vec<WindowDictionary>,
) -> Result<Vec<OrderedWindow>, AdapterError> {
    let mut order = Vec::new();
    for dictionary in dictionaries {
        if required_int_field(&dictionary, "kCGWindowLayer")? != 0 {
            continue;
        }
        let pid = i32::try_from(required_int_field(&dictionary, "kCGWindowOwnerPID")?)
            .unwrap_or_default();
        if pid <= 0
            || dictionary
                .find(CFString::new("kCGWindowOwnerName"))
                .is_none()
        {
            continue;
        }
        order.push(OrderedWindow {
            number: required_int_field(&dictionary, "kCGWindowNumber")?,
            pid,
            on_screen: bool_field(&dictionary, "kCGWindowIsOnscreen")?,
        });
    }
    Ok(order)
}

/// The running processes that descend from `root` and started at or after
/// `since_ms`, from the parent and start time libproc gives for each process.
pub(crate) fn spawned_descendants(root: u32, since_ms: u64) -> Option<Vec<u32>> {
    let mut parents = Vec::new();
    let mut starts = std::collections::HashMap::new();
    for pid in all_pids()? {
        let Some((parent, started)) = parent_and_start(pid) else {
            continue;
        };
        parents.push((pid, parent));
        starts.insert(pid, started);
    }
    Some(agent_desktop_core::spawned_descendants_of(
        root,
        since_ms,
        &parents,
        |pid| starts.get(&pid).copied(),
    ))
}

#[cfg(target_os = "macos")]
fn all_pids() -> Option<Vec<u32>> {
    let wanted = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
    let count = usize::try_from(wanted).ok().filter(|count| *count > 0)? + 64;
    let mut pids = vec![0_i32; count];
    let bytes = i32::try_from(pids.len() * std::mem::size_of::<i32>()).ok()?;
    let filled = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), bytes) };
    let filled = usize::try_from(filled).ok()?;
    Some(
        pids.into_iter()
            .take(filled)
            .filter_map(|pid| u32::try_from(pid).ok())
            .filter(|pid| *pid > 0)
            .collect(),
    )
}

#[cfg(target_os = "macos")]
fn parent_and_start(pid: u32) -> Option<(u32, u64)> {
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let size = i32::try_from(std::mem::size_of::<libc::proc_bsdinfo>()).ok()?;
    let read = unsafe {
        libc::proc_pidinfo(
            i32::try_from(pid).ok()?,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if read != size {
        return None;
    }
    let info = unsafe { info.assume_init() };
    Some((
        info.pbi_ppid,
        info.pbi_start_tvsec * 1_000 + info.pbi_start_tvusec / 1_000,
    ))
}

#[cfg(not(target_os = "macos"))]
fn all_pids() -> Option<Vec<u32>> {
    None
}

#[cfg(not(target_os = "macos"))]
fn parent_and_start(_pid: u32) -> Option<(u32, u64)> {
    None
}

/// The executable name and start time of a process, from libproc.
pub(crate) fn front_process(pid: u32) -> Option<FrontProcess> {
    let pid = i32::try_from(pid).ok()?;
    Some(FrontProcess {
        image: executable_name(pid)?,
        started_ms: ProcessIdentity::capture(pid)
            .ok()
            .flatten()?
            .started_unix_ms(),
    })
}

#[cfg(target_os = "macos")]
fn executable_name(pid: i32) -> Option<String> {
    let mut path = vec![0_u8; 4096];
    let length = unsafe { libc::proc_pidpath(pid, path.as_mut_ptr().cast(), path.len() as u32) };
    let length = usize::try_from(length).ok().filter(|length| *length > 0)?;
    let path = std::str::from_utf8(&path[..length]).ok()?;
    path.rsplit('/').next().map(str::to_owned)
}

#[cfg(not(target_os = "macos"))]
fn executable_name(_pid: i32) -> Option<String> {
    None
}

/// Returns the user's window to the front. An application that is not
/// frontmost is made frontmost first, through its accessibility `AXFrontmost`
/// attribute: `NSRunningApplication` activation waits a full second on a
/// thread the main run loop does not serve, which is where the guard runs.
/// The window itself is then raised through accessibility, which reorders it
/// over an intruder's window.
pub(crate) fn give_back(window: &FrontWindow) -> Result<(), AdapterError> {
    let pid = i32::try_from(window.pid)
        .map_err(|_| AdapterError::internal("front window pid is out of range"))?;
    let number = super::window_resolve::parse_window_number(&window.id)
        .ok_or_else(|| AdapterError::internal("front window id is not a window number"))?;
    let deadline = Deadline::detached_after(GIVE_BACK_BUDGET_MS)?;
    let identity = ProcessIdentity::capture(pid)?
        .ok_or_else(|| AdapterError::stale_ref_because("The front application has exited"))?;
    if !crate::system::focus::is_frontmost(identity, deadline)? {
        make_frontmost(pid, deadline)?;
    }
    let element = super::window_resolve::ax_window_element_for_number(pid, number, None, deadline)?
        .ok_or_else(|| AdapterError::stale_ref_because("The user's window has closed"))?;
    super::window_ops::raise_window(&element, deadline)
}

fn make_frontmost(pid: i32, deadline: Deadline) -> Result<(), AdapterError> {
    use core_foundation::{base::TCFType, boolean::CFBoolean, string::CFString};

    let app = crate::tree::properties::element::element_for_pid(pid);
    let attribute = CFString::new("AXFrontmost");
    let error = crate::tree::properties::ax_ipc::set_attribute_value(
        &app,
        attribute.as_concrete_TypeRef(),
        CFBoolean::true_value().as_CFTypeRef(),
        deadline,
    )?;
    crate::system::focus::finish_mutation(&app, error, "make frontmost", deadline)
}

#[cfg(test)]
#[path = "window_front_tests.rs"]
mod tests;
