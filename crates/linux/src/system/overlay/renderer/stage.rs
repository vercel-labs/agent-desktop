//! The renderer's connection to the X server and what every cursor window
//! shares: the ARGB visual, its colormap, the ownership atom and whether a
//! compositing manager blends the windows' alpha.
use agent_desktop_core::AdapterError;
use x11rb::connection::Connection;
use x11rb::protocol::shape::{ConnectionExt as _, SK, SO};
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use crate::system::overlay::cursor_overlay::{
    OWNER, RENDERER, connect, error, visual::argb_visual,
};

const WINDOW_NAME: &[u8] = b"agent-desktop cursor overlay";

pub(crate) struct Stage {
    pub(crate) connection: RustConnection,
    pub(crate) root: Window,
    visual: Visualid,
    colormap: Colormap,
    owner: Atom,
    renderer: Atom,
    compositor: Atom,
}

impl Stage {
    /// Connects to `display`. A server without a 32-bit ARGB visual cannot
    /// show the cursors and is refused.
    pub(crate) fn open(display: &str) -> Result<Self, AdapterError> {
        let (connection, screen) = connect(display)?;
        let root = connection.setup().roots[screen].root;
        let visual = argb_visual(&connection, screen)?;
        let colormap = connection.generate_id().map_err(error)?;
        connection
            .create_colormap(ColormapAlloc::NONE, colormap, root, visual)
            .map_err(error)?
            .check()
            .map_err(error)?;
        let owner = connection
            .intern_atom(false, OWNER)
            .map_err(error)?
            .reply()
            .map_err(error)?
            .atom;
        let renderer = connection
            .intern_atom(false, RENDERER)
            .map_err(error)?
            .reply()
            .map_err(error)?
            .atom;
        let compositor = connection
            .intern_atom(false, format!("_NET_WM_CM_S{screen}").as_bytes())
            .map_err(error)?
            .reply()
            .map_err(error)?
            .atom;
        Ok(Self {
            connection,
            root,
            visual,
            colormap,
            owner,
            renderer,
            compositor,
        })
    }

    /// Whether a compositing manager owns the screen and so blends each
    /// window's alpha. Without one, a window shows its pixels as they are.
    pub(crate) fn is_composited(&self) -> Result<bool, AdapterError> {
        Ok(self
            .connection
            .get_selection_owner(self.compositor)
            .map_err(error)?
            .reply()
            .map_err(error)?
            .owner
            != x11rb::NONE)
    }

    /// A new, unmapped cursor window owned by `owner`: override-redirect, so
    /// no window manager frames or moves it, and with an empty input shape,
    /// so pointer events pass to the windows below. The ownership property
    /// is what the stacking reader and the screenshot path recognise.
    pub(crate) fn create_window(&self, owner: &[u8]) -> Result<Window, AdapterError> {
        let connection = &self.connection;
        let window = connection.generate_id().map_err(error)?;
        connection
            .create_window(
                32,
                window,
                self.root,
                0,
                0,
                1,
                1,
                0,
                WindowClass::INPUT_OUTPUT,
                self.visual,
                &CreateWindowAux::new()
                    .colormap(self.colormap)
                    .border_pixel(0)
                    .background_pixel(0)
                    .override_redirect(1),
            )
            .map_err(error)?;
        connection
            .change_property8(
                PropMode::REPLACE,
                window,
                AtomEnum::WM_NAME,
                AtomEnum::STRING,
                WINDOW_NAME,
            )
            .map_err(error)?;
        connection
            .change_property8(
                PropMode::REPLACE,
                window,
                self.owner,
                AtomEnum::STRING,
                owner,
            )
            .map_err(error)?;
        connection
            .change_property32(
                PropMode::REPLACE,
                window,
                self.renderer,
                AtomEnum::CARDINAL,
                &[std::process::id()],
            )
            .map_err(error)?;
        connection
            .shape_rectangles(
                SO::SET,
                SK::INPUT,
                ClipOrdering::UNSORTED,
                window,
                0,
                0,
                &[],
            )
            .map_err(error)?;
        Ok(window)
    }

    fn has_property(&self, window: Window, property: Atom) -> Result<bool, AdapterError> {
        let reply = self
            .connection
            .get_property(false, window, property, AtomEnum::ANY, 0, 0)
            .map_err(error)?
            .reply();
        Ok(reply.is_ok_and(|reply| reply.type_ != x11rb::NONE))
    }

    /// Removes the cursor windows an earlier version kept on the server
    /// between commands: they carry the ownership property and no renderer
    /// mark. Another renderer's windows, such as one serving another state
    /// root on the same display, carry the mark and are left alone.
    pub(crate) fn sweep_left_behind(&self) -> Result<(), AdapterError> {
        let children = self
            .connection
            .query_tree(self.root)
            .map_err(error)?
            .reply()
            .map_err(error)?
            .children;
        for window in children {
            if self.has_property(window, self.owner)?
                && !self.has_property(window, self.renderer)?
            {
                self.connection.kill_client(window).map_err(error)?;
            }
        }
        self.connection.flush().map_err(error)
    }
}

#[cfg(test)]
#[path = "stage_tests.rs"]
mod tests;
