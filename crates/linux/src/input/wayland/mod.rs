pub(crate) mod kept_session;
pub(crate) mod key_session;
pub(crate) mod keysym_send;
pub(crate) mod layout;
pub(crate) mod pointer_send;
pub(crate) mod portal_pointer;
pub(crate) mod portal_remote_desktop;
pub(crate) mod portal_request;
pub(crate) mod portal_token;
pub(crate) mod remote_desktop;
pub(crate) mod session_backend;

#[cfg(test)]
mod fake_session;
#[cfg(test)]
mod live_shell;
#[cfg(test)]
mod live_tests;
