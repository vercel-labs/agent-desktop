//! The one place that registers and releases process-wide COM MTA usage.
//! Both natives live here so the increment and its matching decrement cannot drift apart.

pub(crate) type MtaUsageRelease = Box<dyn FnOnce(usize) -> i32 + Send + Sync>;

/// One MTA usage registration and whether its cookie is still owed to COM.
pub(crate) enum MtaUsage {
    Held {
        cookie_address: usize,
        release: MtaUsageRelease,
    },
    Released,
}

impl MtaUsage {
    /// Registers MTA usage with COM; the error is the failing HRESULT.
    pub(crate) fn acquire() -> Result<Self, i32> {
        Self::acquire_with(co_increment_mta_usage, Box::new(co_decrement_mta_usage))
    }

    /// Builds a registration from the calls it should register and release with.
    pub(crate) fn acquire_with(
        acquire: impl FnOnce() -> (i32, usize),
        release: MtaUsageRelease,
    ) -> Result<Self, i32> {
        let (hresult, cookie_address) = acquire();
        classify_mta_usage_hresult(hresult)?;
        Ok(Self::Held {
            cookie_address,
            release,
        })
    }

    /// Moves `Held` to `Released` and returns COM's verdict; an already released
    /// registration is `Ok`.
    pub(crate) fn release(&mut self) -> Result<(), i32> {
        match std::mem::replace(self, Self::Released) {
            Self::Held {
                cookie_address,
                release,
            } => classify_mta_usage_hresult(release(cookie_address)),
            Self::Released => Ok(()),
        }
    }
}

pub(crate) fn classify_mta_usage_hresult(hresult: i32) -> Result<(), i32> {
    if hresult >= 0 { Ok(()) } else { Err(hresult) }
}

#[cfg(target_os = "windows")]
pub(crate) fn co_increment_mta_usage() -> (i32, usize) {
    use windows_sys::Win32::System::Com::{CO_MTA_USAGE_COOKIE, CoIncrementMTAUsage};
    let mut cookie: CO_MTA_USAGE_COOKIE = std::ptr::null_mut();
    let hresult = unsafe { CoIncrementMTAUsage(&mut cookie) };
    (hresult, cookie.expose_provenance())
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn co_increment_mta_usage() -> (i32, usize) {
    (crate::system::runtime::hresult::S_OK, 0)
}

#[cfg(target_os = "windows")]
fn co_decrement_mta_usage(cookie_address: usize) -> i32 {
    use windows_sys::Win32::System::Com::CoDecrementMTAUsage;
    unsafe { CoDecrementMTAUsage(std::ptr::with_exposed_provenance_mut(cookie_address)) }
}

#[cfg(not(target_os = "windows"))]
fn co_decrement_mta_usage(_cookie_address: usize) -> i32 {
    crate::system::runtime::hresult::S_OK
}

#[cfg(test)]
#[path = "mta_usage_tests.rs"]
mod tests;
