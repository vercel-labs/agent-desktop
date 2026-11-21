use crate::AdAdapter;
use crate::convert::app::{app_info_to_c, free_app_info_fields};
use crate::error::{AdResult, set_last_error};
use crate::ffi_try::{trap_panic, trap_panic_void};
use crate::types::{AdAppInfo, AdAppList};
use std::ptr;

/// The apps of a complete inventory. The list has no field for a skipped
/// record, so an inventory that skipped one is refused instead of returned as
/// the whole list.
fn complete_apps(
    inventory: agent_desktop_core::AppInventory,
) -> Result<Vec<agent_desktop_core::AppInfo>, agent_desktop_core::AdapterError> {
    match inventory.skipped.first() {
        Some(skipped) => {
            let subject = skipped
                .get("pid")
                .map_or_else(|| "unknown".to_owned(), ToString::to_string);
            Err(inventory.incomplete_error(&subject))
        }
        None => Ok(inventory.apps),
    }
}

/// # Safety
/// `adapter` must be a valid pointer from `ad_adapter_create`.
/// `out` must be a valid writable `*mut *mut AdAppList`.
/// On success, `*out` is a newly-allocated opaque list freed with
/// `ad_app_list_free`. On error, `*out` is null and last-error is set.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_list_apps(
    adapter: *const AdAdapter,
    out: *mut *mut AdAppList,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        *out = ptr::null_mut();
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        let adapter = crate::adapter::acquire_adapter!(adapter);
        let deadline = crate::operation::operation_deadline!();
        match adapter
            .inner
            .query_app_inventory(agent_desktop_core::AppInventoryQuery::All, deadline)
            .and_then(complete_apps)
        {
            Ok(apps) => {
                if let Err(error) = crate::resource::validate_list_len(apps.len(), "App list") {
                    set_last_error(&error);
                    return crate::error::last_error_code();
                }
                let items: Vec<AdAppInfo> = apps.iter().map(app_info_to_c).collect();
                let list = Box::new(AdAppList {
                    items: items.into_boxed_slice(),
                });
                *out = Box::into_raw(list);
                AdResult::Ok
            }
            Err(e) => {
                set_last_error(&e);
                crate::error::last_error_code()
            }
        }
    })
}

/// # Safety
/// `list` must be null or a pointer returned by `ad_list_apps`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_app_list_count(list: *const AdAppList) -> u32 {
    if list.is_null() {
        return 0;
    }
    let list_ref: &AdAppList = unsafe { &*list };
    list_ref.items.len() as u32
}

/// Returns a borrowed pointer into the list; valid until the list is freed.
/// Out-of-range `index` returns null.
///
/// # Safety
/// `list` must be null or a pointer returned by `ad_list_apps`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_app_list_get(list: *const AdAppList, index: u32) -> *const AdAppInfo {
    if list.is_null() {
        return ptr::null();
    }
    let list_ref: &AdAppList = unsafe { &*list };
    match list_ref.items.get(index as usize) {
        Some(item) => item as *const AdAppInfo,
        None => ptr::null(),
    }
}

/// Frees the list and every `AdAppInfo` it owns, including the interior
/// C-strings.
///
/// # Safety
/// `list` must be null or a pointer returned by `ad_list_apps`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_app_list_free(list: *mut AdAppList) {
    trap_panic_void(|| unsafe {
        if list.is_null() {
            return;
        }
        let mut list = Box::from_raw(list);
        for item in list.items.iter_mut() {
            free_app_info_fields(item);
        }
    })
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
