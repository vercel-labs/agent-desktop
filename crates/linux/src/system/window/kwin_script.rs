use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::sleep;
use std::time::Duration;

use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, ErrorCode, WindowInfo, WindowOp,
};

use crate::system::bus::bus_deadline::within;
use crate::system::bus::session_bus::SessionBus;

const KWIN_SERVICE: &str = "org.kde.KWin";
const SCRIPTING_PATH: &str = "/Scripting";
const SCRIPTING_IFACE: &str = "org.kde.kwin.Scripting";
const SCRIPT_IFACE: &str = "org.kde.kwin.Script";
const CLOSE_POLL: Duration = Duration::from_millis(50);
const CLOSE_GRACE: Duration = Duration::from_secs(3);

static NEXT_SCRIPT: AtomicU64 = AtomicU64::new(1);

pub(crate) fn is_available(deadline: Deadline) -> bool {
    let Ok(session) = SessionBus::get(deadline) else {
        return false;
    };
    within(deadline, async {
        let reply = session
            .call_method(
                Some(KWIN_SERVICE),
                SCRIPTING_PATH,
                Some(SCRIPTING_IFACE),
                "isScriptLoaded",
                &("agent_desktop_probe",),
            )
            .await;
        Ok(reply.is_ok())
    })
    .unwrap_or(false)
}

fn temp_script_path(plugin: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{plugin}.js"))
}

fn write_temp_script(plugin: &str, source: &str) -> Result<PathBuf, AdapterError> {
    let path = temp_script_path(plugin);
    fs::write(&path, source).map_err(|error| {
        AdapterError::new(
            ErrorCode::ActionFailed,
            "Could not write temporary KWin script",
        )
        .with_platform_detail(error.to_string())
        .with_disposition(DeliverySemantics::not_delivered())
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(path)
}

fn run_script(source: &str, deadline: Deadline) -> Result<(), AdapterError> {
    let seq = NEXT_SCRIPT.fetch_add(1, Ordering::Relaxed);
    let plugin = format!("agent_desktop_kwin_{}_{seq}", std::process::id());
    let path = write_temp_script(&plugin, source)?;
    let path_str = path.to_string_lossy().into_owned();
    let session = SessionBus::get(deadline)?;
    let outcome = within(deadline, async {
        let loaded = session
            .call_method(
                Some(KWIN_SERVICE),
                SCRIPTING_PATH,
                Some(SCRIPTING_IFACE),
                "loadScript",
                &(path_str.as_str(), plugin.as_str()),
            )
            .await
            .map_err(|error| {
                AdapterError::new(ErrorCode::ActionFailed, "KWin loadScript failed")
                    .with_platform_detail(error.to_string())
                    .with_disposition(DeliverySemantics::not_delivered())
            })?;
        let script_id: i32 = loaded.body().deserialize().map_err(|error| {
            AdapterError::new(
                ErrorCode::ActionFailed,
                "KWin returned an invalid script id",
            )
            .with_platform_detail(error.to_string())
            .with_disposition(DeliverySemantics::not_delivered())
        })?;
        if script_id < 0 {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "KWin refused to load the window control script",
            )
            .with_disposition(DeliverySemantics::not_delivered()));
        }
        let script_path = format!("/Scripting/Script{script_id}");
        let run_res = session
            .call_method(
                Some(KWIN_SERVICE),
                script_path.as_str(),
                Some(SCRIPT_IFACE),
                "run",
                &(),
            )
            .await;
        let _ = session
            .call_method(
                Some(KWIN_SERVICE),
                SCRIPTING_PATH,
                Some(SCRIPTING_IFACE),
                "unloadScript",
                &(plugin.as_str(),),
            )
            .await;
        run_res.map(|_| ()).map_err(|error| {
            AdapterError::new(ErrorCode::ActionFailed, "KWin script execution failed")
                .with_platform_detail(error.to_string())
                .with_disposition(DeliverySemantics::uncertain())
        })
    });
    let _ = fs::remove_file(&path);
    outcome
}

pub(super) fn match_target_js(pid: u32, title: &str) -> String {
    let quoted_title = serde_json::to_string(title).unwrap_or_else(|_| "\"\"".to_owned());
    format!(
        "const list = workspace.windowList().filter(w => w.pid === {pid} && !w.specialWindow);\n\
         const target = list.find(w => w.caption === {quoted_title}) || list[0];\n"
    )
}

pub(super) fn focus_script(pid: u32, title: &str) -> String {
    format!(
        "{}if (target) {{\n\
         target.minimized = false;\n\
         workspace.activeWindow = target;\n\
         }}\n",
        match_target_js(pid, title)
    )
}

pub(super) fn op_script(win: &WindowInfo, op: &WindowOp) -> String {
    let action = match op {
        WindowOp::Move { x, y } => format!(
            "const g = target.frameGeometry;\n\
             target.frameGeometry = {{ x: {x}, y: {y}, width: g.width, height: g.height }};"
        ),
        WindowOp::Resize { width, height } => format!(
            "const g = target.frameGeometry;\n\
             target.frameGeometry = {{ x: g.x, y: g.y, width: {width}, height: {height} }};"
        ),
        WindowOp::Minimize => "target.minimized = true;".to_owned(),
        WindowOp::Maximize => {
            "target.minimized = false;\ntarget.setMaximize(true, true);".to_owned()
        }
        WindowOp::Restore => {
            "target.minimized = false;\ntarget.setMaximize(false, false);".to_owned()
        }
    };
    format!(
        "{}if (target) {{\n{action}\n}}\n",
        match_target_js(win.pid.get(), &win.title)
    )
}

pub(super) fn close_pid_script(pid: u32) -> String {
    format!(
        "for (const w of workspace.windowList()) {{\n\
         if (w.pid === {pid} && !w.specialWindow) {{ w.closeWindow(); }}\n\
         }}\n"
    )
}

pub(crate) fn focus_window(pid: u32, title: &str, deadline: Deadline) -> Result<(), AdapterError> {
    run_script(&focus_script(pid, title), deadline)?;
    sleep(Duration::from_millis(80).min(deadline.remaining()));
    Ok(())
}

pub(crate) fn apply_window_op(
    win: &WindowInfo,
    op: &WindowOp,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    run_script(&op_script(win, op), deadline)
}

pub(crate) fn try_window_op(
    win: &WindowInfo,
    op: &WindowOp,
    deadline: Deadline,
) -> Option<Result<agent_desktop_core::OperationReport, AdapterError>> {
    use crate::system::display::display_server::{DisplayServer, session_server};
    if session_server() != DisplayServer::Wayland || !is_available(deadline) {
        return None;
    }
    let dispatched = apply_window_op(win, op, deadline);
    Some(agent_desktop_core::OperationReport::from_window_dispatch(
        win,
        op,
        super::window_report::window_readback(op),
        dispatched,
    ))
}

pub(crate) fn close_pid_windows(
    pid: u32,
    is_gone: impl Fn() -> bool,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    run_script(&close_pid_script(pid), deadline)?;
    let wait_until = deadline.capped(CLOSE_GRACE);
    while !wait_until.is_expired() {
        if is_gone() {
            return Ok(true);
        }
        sleep(CLOSE_POLL.min(wait_until.remaining()));
    }
    Ok(true)
}

#[cfg(test)]
#[path = "kwin_script_tests.rs"]
mod tests;
