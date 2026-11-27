//! The `App Paths` registry key, which the Run dialog and `ShellExecute` read
//! to find applications installed outside the system directories, such as
//! `chrome.exe` or `msedge.exe`.

use std::path::PathBuf;

const APP_PATHS_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\App Paths";

/// The executable registered for `image` under the current user's, then the
/// machine's, `App Paths` key, when it names an existing file.
#[cfg(target_os = "windows")]
pub(crate) fn registered_app_path(image: &str) -> Option<PathBuf> {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

    let subkey = format!(r"{APP_PATHS_KEY}\{image}");
    [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE]
        .into_iter()
        .filter_map(|root| read_default_value(root, &subkey))
        .filter_map(|value| parse_registered_path(&value))
        .find(|path| path.is_file())
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn registered_app_path(_image: &str) -> Option<PathBuf> {
    None
}

/// An `App Paths` default value names an absolute executable, sometimes
/// wrapped in quotes.
pub(crate) fn parse_registered_path(value: &str) -> Option<PathBuf> {
    let unquoted = value.trim().trim_matches('"').trim();
    let path = PathBuf::from(unquoted);
    path.is_absolute().then_some(path)
}

#[cfg(target_os = "windows")]
fn read_default_value(
    root: windows_sys::Win32::System::Registry::HKEY,
    subkey: &str,
) -> Option<String> {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{RRF_RT_REG_SZ, RegGetValueW};

    let key: Vec<u16> = subkey.encode_utf16().chain(Some(0)).collect();
    let mut size: u32 = 0;
    let status = unsafe {
        RegGetValueW(
            root,
            key.as_ptr(),
            std::ptr::null(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut size,
        )
    };
    if status != ERROR_SUCCESS || size == 0 {
        return None;
    }
    let mut buffer = vec![0u16; (size as usize).div_ceil(2)];
    let status = unsafe {
        RegGetValueW(
            root,
            key.as_ptr(),
            std::ptr::null(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let length = buffer
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..length]))
}

#[cfg(test)]
#[path = "launch_app_paths_tests.rs"]
mod tests;
