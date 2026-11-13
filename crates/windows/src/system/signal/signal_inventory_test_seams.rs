#![cfg(test)]

pub(super) mod enum_windows_calls {
    use std::cell::Cell;

    thread_local! {
        static COUNT: Cell<usize> = const { Cell::new(0) };
    }

    pub(crate) fn record() {
        crate::system::test_support::call_counter::record(&COUNT);
    }

    pub(crate) fn take() -> usize {
        crate::system::test_support::call_counter::take(&COUNT)
    }
}

#[cfg(target_os = "windows")]
pub(super) mod force_race {
    use std::cell::Cell;

    thread_local! {
        static REMAINING: Cell<usize> = const { Cell::new(0) };
    }

    pub(crate) fn with<R>(times: usize, run: impl FnOnce() -> R) -> R {
        crate::system::test_support::helpers::with_usize_flag(&REMAINING, times, run)
    }

    pub(crate) fn consume_if_armed() -> bool {
        REMAINING.with(|cell| {
            let remaining = cell.get();
            if remaining == 0 {
                false
            } else {
                cell.set(remaining - 1);
                true
            }
        })
    }
}

#[cfg(target_os = "windows")]
pub(super) mod force_truncation {
    use std::cell::Cell;

    thread_local! {
        static ACTIVE: Cell<bool> = const { Cell::new(false) };
    }

    pub(crate) fn with<R>(run: impl FnOnce() -> R) -> R {
        crate::system::test_support::helpers::with_flag(&ACTIVE, true, run)
    }

    pub(crate) fn is_active() -> bool {
        ACTIVE.with(Cell::get)
    }
}

pub(in crate::system::signal) mod force_failure {
    use std::cell::RefCell;

    use agent_desktop_core::AdapterError;

    thread_local! {
        static ARMED: RefCell<Option<AdapterError>> = const { RefCell::new(None) };
    }

    pub(in crate::system::signal) fn with<R>(error: AdapterError, run: impl FnOnce() -> R) -> R {
        ARMED.with(|cell| *cell.borrow_mut() = Some(error));
        let outcome = run();
        ARMED.with(|cell| cell.borrow_mut().take());
        outcome
    }

    pub(in crate::system::signal) fn take() -> Option<AdapterError> {
        ARMED.with(|cell| cell.borrow_mut().take())
    }
}
