pub mod actions;
pub mod automation;
pub mod chromium;
pub mod descriptor;
pub(crate) mod hit_test;
pub(crate) mod live_read;
pub(crate) mod minimized;
pub mod name_evidence;
pub(crate) mod observe;
pub(crate) mod resolve;
pub mod roles;
pub mod states;
pub(crate) mod surface_inventory;
pub mod surfaces;
pub(crate) mod text_insertion;
pub(crate) mod text_selection;
pub mod walk;

#[cfg(test)]
mod captures;
pub mod element;
pub mod properties;

#[cfg(test)]
mod test_support;

#[cfg(test)]
#[path = "hit_test_scan_tests.rs"]
mod hit_test_scan_tests;
