//! The X11 cursor renderer: a detached child process that draws each agent's
//! cursor in override-redirect windows and plays the same requests the GNOME
//! Shell extension plays on Wayland.
pub(crate) mod art;
pub(crate) mod child;
pub(crate) mod client;
pub(crate) mod cursor;
pub(crate) mod cursor_face;
pub(crate) mod cursor_label;
pub(crate) mod cursor_windows;
pub(crate) mod desk;
pub(crate) mod effect;
pub(crate) mod endpoint;
pub(crate) mod image;
pub(crate) mod layer;
pub(crate) mod motion;
pub(crate) mod paint;
pub(crate) mod painter;
pub(crate) mod picture;
pub(crate) mod piece;
pub(crate) mod presence;
pub(crate) mod raster;
pub(crate) mod request;
pub(crate) mod scene;
pub(crate) mod screens;
pub(crate) mod serve;
pub(crate) mod session_watch;
pub(crate) mod slot;
pub(crate) mod slot_window;
pub(crate) mod spawn;
pub(crate) mod stage;
pub(crate) mod text;
pub(crate) mod travel;
pub(crate) mod travel_order;
pub(crate) mod wire;
