use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionPolicy, WindowInfo, WindowOp,
};

use super::hold::give_back_place;
use super::refuse::{refused, unreadable};
use super::{OpWorld, apply};
use crate::system::focus::restore_minimized::{Lifted, still_hidden};
use crate::system::x11::control::ControlRead;
use crate::system::x11::control::control_send::WindowWrite;
use crate::system::x11::control::window_facts::WindowFacts;

pub(super) fn read_before(world: &impl OpWorld, client: u32) -> Result<WindowFacts, AdapterError> {
    match world.facts(client) {
        ControlRead::Ready(facts) => Ok(facts),
        _ => Err(unreadable()),
    }
}

pub(super) fn restore(
    world: &impl OpWorld,
    win: &WindowInfo,
    client: u32,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let mut current = read_before(world, client)?;
    if current.hidden && world.on_wayland() {
        return Err(refused(
            ErrorCode::ActionNotSupported,
            "wayland_session",
            "A Wayland compositor refuses to show a minimized window for another client",
            "Show the window yourself, for example from the overview",
        ));
    }
    if current.hidden {
        WindowOp::require_unminimize_allowed(policy)?;
        if let Lifted::StillHidden { manager } = world.lift(client)? {
            return Err(still_hidden(&manager, world.overview_open() == Some(true)));
        }
        current = read_before(world, client)
            .map_err(|error| error.with_disposition(DeliverySemantics::delivered_unverified()))?;
    }
    if !current.maximized_vertical && !current.maximized_horizontal {
        return give_back_place(world, win, client, deadline);
    }
    apply(
        world,
        win,
        client,
        &WindowWrite::SetMaximized(false),
        deadline,
    )?;
    give_back_place(world, win, client, deadline)
}
