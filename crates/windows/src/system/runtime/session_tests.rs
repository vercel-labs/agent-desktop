use super::*;
use crate::system::runtime::mta_usage::MtaUsageRelease;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

const FAKE_COOKIE_ADDRESS: usize = 0x5EED;
const E_OUTOFMEMORY_HRESULT: i32 = 0x8007_000E_u32 as i32;

fn counting_release(count: &Arc<AtomicU32>) -> MtaUsageRelease {
    let count = Arc::clone(count);
    Box::new(move |_| {
        count.fetch_add(1, Ordering::SeqCst);
        0
    })
}

fn counted_session(count: &Arc<AtomicU32>) -> WindowsAdapterSession {
    open_with(Deadline::after(1_000).unwrap(), || {
        MtaUsage::acquire_with(|| (0, FAKE_COOKIE_ADDRESS), counting_release(count))
    })
    .unwrap()
}

#[test]
fn close_releases_the_acquired_cookie_exactly_once() {
    let released: Arc<Mutex<Vec<usize>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&released);
    let session = open_with(Deadline::after(1_000).unwrap(), || {
        MtaUsage::acquire_with(
            || (0, FAKE_COOKIE_ADDRESS),
            Box::new(move |cookie_address| {
                sink.lock().unwrap().push(cookie_address);
                0
            }),
        )
    })
    .unwrap();

    Box::new(session).close().unwrap();

    assert_eq!(*released.lock().unwrap(), vec![FAKE_COOKIE_ADDRESS]);
}

#[test]
fn a_session_dropped_without_close_releases_exactly_once() {
    let count = Arc::new(AtomicU32::new(0));
    let session = counted_session(&count);

    drop(session);

    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn a_failing_release_reports_the_hresult_and_never_retries() {
    let count = Arc::new(AtomicU32::new(0));
    let calls = Arc::clone(&count);
    let session = open_with(Deadline::after(1_000).unwrap(), || {
        MtaUsage::acquire_with(
            || (0, FAKE_COOKIE_ADDRESS),
            Box::new(move |_| {
                calls.fetch_add(1, Ordering::SeqCst);
                E_OUTOFMEMORY_HRESULT
            }),
        )
    })
    .unwrap();

    let error = Box::new(session).close().unwrap_err();

    assert_eq!(error.code, ErrorCode::Internal);
    assert!(
        error
            .platform_detail
            .is_some_and(|detail| detail.contains("0x8007000E"))
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn a_failing_acquire_is_an_error_that_schedules_no_release() {
    let count = Arc::new(AtomicU32::new(0));

    let Err(error) = open_with(Deadline::after(1_000).unwrap(), || {
        MtaUsage::acquire_with(|| (E_OUTOFMEMORY_HRESULT, 0), counting_release(&count))
    }) else {
        panic!("a failing acquire must not produce a session");
    };

    assert_eq!(error.code, ErrorCode::Internal);
    assert!(
        error
            .platform_detail
            .is_some_and(|detail| detail.contains("0x8007000E"))
    );
    assert_eq!(count.load(Ordering::SeqCst), 0);
}

#[test]
fn an_expired_deadline_times_out_before_touching_the_native_acquire() {
    let Err(error) = open_with(Deadline::after(0).unwrap(), || -> Result<MtaUsage, i32> {
        panic!("an expired deadline must not reach the native acquire")
    }) else {
        panic!("an expired deadline must not produce a session");
    };

    assert_eq!(error.code, ErrorCode::Timeout);
}

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn the_session_is_send_and_sync_as_a_value_and_as_a_box() {
    assert_send_sync::<WindowsAdapterSession>();
    assert_send_sync::<Box<WindowsAdapterSession>>();
}

#[test]
fn a_session_moved_to_another_thread_releases_exactly_once_there() {
    let count = Arc::new(AtomicU32::new(0));
    let session = Box::new(counted_session(&count));

    std::thread::spawn(move || drop(session))
        .join()
        .expect("the thread that drops a moved session must not panic");

    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[cfg(target_os = "windows")]
#[test]
fn a_real_com_mta_usage_acquires_and_closes_cleanly() {
    let session = open(Deadline::after(5_000).unwrap()).unwrap();

    Box::new(session)
        .close()
        .expect("releasing a real MTA usage cookie must succeed");
}
