//! The effect witness read: what an action's effect can change outside the
//! target's own value, as core compares it before and after a click or select.
//!
//! The target answers through its own patterns (`SelectionItem`, `Value`,
//! `ExpandCollapse`), the focus through the system's one focused element, and
//! the window through its root handle and the direct children of its element.
//! A field that cannot be read cheaply stays `None`, and a vanished target is a
//! stale ref.

use std::hash::{Hash, Hasher};

/// Open surfaces below an element from its `ExpandCollapse` reads: `None`
/// without the pattern or when its state cannot be read, one while expanded,
/// none otherwise.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) fn surfaces_from(available: Option<bool>, state: Option<i32>) -> Option<u32> {
    if available != Some(true) {
        return None;
    }
    state.map(|state| u32::from(state == 1))
}

/// A sign of what the direct children of a window are: equal signs mean the
/// children the read named did not change.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) fn content_sign<T: Hash>(children: &[T]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    children.hash(&mut hasher);
    hasher.finish()
}

/// The window handle a native window's runtime id carries (prefix 42). Unlike
/// the element's root window, it stays readable after the window is gone.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) fn hwnd_of(runtime_id: &[i32]) -> Option<isize> {
    match runtime_id {
        [42, hwnd, ..] => Some(*hwnd as isize),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{content_sign, hwnd_of, surfaces_from};
    use crate::tree::automation::{automation_client, uia_error};
    use crate::tree::element::{UIAElement, uia_element};
    use crate::tree::hit_test::corroborate::element_root_hwnd;
    use crate::tree::properties::property_ids::TreeProperty;
    use crate::tree::properties::read_cached;
    use crate::tree::walk::cache::build_walk_cache_request;
    use agent_desktop_core::{
        AdapterError, Deadline, EffectWitness, FocusWitness, NativeHandle, TargetWitness,
        WindowWitness,
    };
    use std::cell::RefCell;
    use uiautomation::UIAutomation;
    use uiautomation::core::UICacheRequest;
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GA_ROOT, GUITHREADINFO, GW_CHILD, GW_HWNDNEXT, GetAncestor, GetGUIThreadInfo,
        GetWindow, GetWindowTextW, GetWindowThreadProcessId, IsChild, IsWindowVisible,
    };

    const MAX_CHILDREN: usize = 512;

    const TARGET_SET: [TreeProperty; 6] = [
        TreeProperty::SelectionItemAvailable,
        TreeProperty::SelectionItemIsSelected,
        TreeProperty::ValueAvailable,
        TreeProperty::Value,
        TreeProperty::ExpandCollapseAvailable,
        TreeProperty::ExpandCollapseState,
    ];

    thread_local! {
        /// The cache request of the target read, built once per thread.
        static TARGET_CACHE: RefCell<Option<UICacheRequest>> = const { RefCell::new(None) };
    }

    pub(crate) fn read_effect_witness(
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Result<EffectWitness, AdapterError> {
        deadline.ensure_remaining()?;
        let element = uia_element(handle)?;
        let client = automation_client()?;
        let target = target_witness(element, &client)?;
        let walker = client
            .get_raw_view_walker()
            .map_err(|error| uia_error(&error, "open the effect witness walker"))?;
        let target_root = element_root_hwnd(element, &walker, deadline).ok().flatten();
        let anchor = element.0.get_runtime_id().ok().as_deref().and_then(hwnd_of);
        Ok(EffectWitness {
            target,
            focus: focus_witness(anchor, target_root),
            window: window_witness(element, target_root, anchor.or(target_root)),
        })
    }

    /// One cached read of the target's three patterns. A target that is gone
    /// fails the update: a stale ref.
    fn target_witness(
        element: &UIAElement,
        client: &UIAutomation,
    ) -> Result<TargetWitness, AdapterError> {
        let cache = TARGET_CACHE
            .with(|cell| {
                if cell.borrow().is_none() {
                    *cell.borrow_mut() = build_walk_cache_request(client, &TARGET_SET).ok();
                }
                cell.borrow().clone()
            })
            .ok_or_else(|| {
                AdapterError::internal("the effect witness cache request did not build")
            })?;
        let updated = element.0.build_updated_cache(&cache).map_err(|error| {
            let mapped = uia_error(&error, "read the effect witness target");
            if mapped.code == agent_desktop_core::ErrorCode::StaleRef {
                AdapterError::stale_ref_because("The target element is gone")
                    .with_details(serde_json::json!({ "complete": true, "retryable": false }))
            } else {
                mapped
            }
        })?;
        let (properties, _) = read_cached(&UIAElement::from(updated), &TARGET_SET);
        Ok(TargetWitness {
            selected: properties.gated_flag(TreeProperty::SelectionItemIsSelected),
            value: properties
                .is_true(TreeProperty::ValueAvailable)
                .then(|| properties.get(TreeProperty::Value).non_blank_text())
                .flatten(),
            surfaces: surfaces_from(
                properties.get(TreeProperty::ExpandCollapseAvailable).flag(),
                properties.get(TreeProperty::ExpandCollapseState).number(),
            ),
        })
    }

    /// The system focus at window-handle grain: the focused control's handle
    /// names it, and its relation to the target comes from the handle tree.
    fn focus_witness(anchor: Option<isize>, target_root: Option<isize>) -> FocusWitness {
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        let focus = (unsafe { GetGUIThreadInfo(0, &mut info) } != 0 && !info.hwndFocus.is_null())
            .then_some(info.hwndFocus);
        FocusWitness {
            element: focus.map(|focus| format!("hwnd:{}", focus as isize)),
            inside_target: focus.zip(anchor).map(|(focus, anchor)| {
                focus as isize == anchor || unsafe { IsChild(anchor as HWND, focus) } != 0
            }),
            in_target_window: focus
                .zip(target_root)
                .map(|(focus, root)| (unsafe { GetAncestor(focus, GA_ROOT) }) as isize == root),
            value: None,
        }
    }

    fn window_witness(
        element: &UIAElement,
        root: Option<isize>,
        anchor: Option<isize>,
    ) -> WindowWitness {
        let listed = element
            .0
            .get_process_id()
            .ok()
            .map(|pid| process_windows(pid, anchor));
        WindowWitness {
            title: root.and_then(title_of),
            content: root.and_then(content_of),
            windows: listed.map(|(count, _)| count),
            present: listed.and_then(|(_, present)| present),
        }
    }

    fn title_of(window: isize) -> Option<String> {
        let mut buffer = [0u16; 256];
        let length = unsafe { GetWindowTextW(window as HWND, buffer.as_mut_ptr(), 256) };
        (length > 0).then(|| String::from_utf16_lossy(&buffer[..length as usize]))
    }

    /// A sign of the window's direct child windows (their handles): a window
    /// gaining or losing a control or a pane changes it.
    fn content_of(root: isize) -> Option<u64> {
        let mut children = Vec::new();
        let mut child = unsafe { GetWindow(root as HWND, GW_CHILD) };
        while !child.is_null() && children.len() < MAX_CHILDREN {
            children.push(child as isize);
            child = unsafe { GetWindow(child, GW_HWNDNEXT) };
        }
        Some(content_sign(&children))
    }

    /// The visible top-level windows of a process, and whether `target` is one
    /// of them: one `EnumWindows` pass with one owner read per window.
    fn process_windows(pid: u32, target: Option<isize>) -> (u32, Option<bool>) {
        struct Listing {
            pid: u32,
            target: isize,
            count: u32,
            found: bool,
        }
        unsafe extern "system" fn list(window: HWND, state: LPARAM) -> i32 {
            let listing = unsafe { &mut *(state as *mut Listing) };
            let mut owner = 0;
            unsafe { GetWindowThreadProcessId(window, &mut owner) };
            if owner == listing.pid && unsafe { IsWindowVisible(window) } != 0 {
                listing.count += 1;
                listing.found |= window as isize == listing.target;
            }
            1
        }
        let top = target.map(|handle| {
            let root = unsafe { GetAncestor(handle as HWND, GA_ROOT) };
            if root.is_null() {
                handle
            } else {
                root as isize
            }
        });
        let mut listing = Listing {
            pid,
            target: top.unwrap_or_default(),
            count: 0,
            found: false,
        };
        unsafe { EnumWindows(Some(list), &mut listing as *mut Listing as LPARAM) };
        (listing.count, target.map(|_| listing.found))
    }
}

#[cfg(target_os = "windows")]
pub(crate) use imp::read_effect_witness;

/// Off Windows nothing can be read.
#[cfg(not(target_os = "windows"))]
pub(crate) fn read_effect_witness(
    _handle: &agent_desktop_core::NativeHandle,
    _deadline: agent_desktop_core::Deadline,
) -> Result<agent_desktop_core::EffectWitness, agent_desktop_core::AdapterError> {
    Err(agent_desktop_core::AdapterError::not_supported(
        "read_effect_witness",
    ))
}

#[cfg(test)]
#[path = "effect_witness_tests.rs"]
mod tests;
