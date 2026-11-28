use super::*;
use crate::system::runtime::hresult::S_OK;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

const E_OUTOFMEMORY_HRESULT: i32 = 0x8007_000E_u32 as i32;

#[test]
fn acquire_with_holds_the_cookie_com_returned() {
    let usage = MtaUsage::acquire_with(|| (S_OK, 0x5EED), Box::new(|_| S_OK))
        .expect("the canned acquire succeeds");
    assert!(matches!(
        usage,
        MtaUsage::Held {
            cookie_address: 0x5EED,
            ..
        }
    ));
}

#[test]
fn a_failing_acquire_is_the_hresult_and_schedules_no_release() {
    let calls = Arc::new(AtomicU32::new(0));
    let counted = Arc::clone(&calls);
    let outcome = MtaUsage::acquire_with(
        || (E_OUTOFMEMORY_HRESULT, 0),
        Box::new(move |_| {
            counted.fetch_add(1, Ordering::SeqCst);
            S_OK
        }),
    );
    match outcome {
        Err(hresult) => assert_eq!(hresult, E_OUTOFMEMORY_HRESULT),
        Ok(_) => panic!("a failed registration must not yield a held cookie"),
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn release_hands_the_cookie_back_once_and_ends_released() {
    let cookies = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&cookies);
    let mut usage = MtaUsage::acquire_with(
        || (S_OK, 0x5EED),
        Box::new(move |cookie_address| {
            recorded.lock().unwrap().push(cookie_address);
            S_OK
        }),
    )
    .expect("the canned acquire succeeds");
    assert_eq!(usage.release(), Ok(()));
    assert_eq!(*cookies.lock().unwrap(), vec![0x5EED]);
    assert!(matches!(usage, MtaUsage::Released));
}

#[test]
fn releasing_twice_calls_com_once() {
    let calls = Arc::new(AtomicU32::new(0));
    let counted = Arc::clone(&calls);
    let mut usage = MtaUsage::acquire_with(
        || (S_OK, 0x5EED),
        Box::new(move |_| {
            counted.fetch_add(1, Ordering::SeqCst);
            S_OK
        }),
    )
    .expect("the canned acquire succeeds");
    assert_eq!(usage.release(), Ok(()));
    assert_eq!(usage.release(), Ok(()));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn a_failing_release_reports_the_hresult_and_is_never_retried() {
    let calls = Arc::new(AtomicU32::new(0));
    let counted = Arc::clone(&calls);
    let mut usage = MtaUsage::acquire_with(
        || (S_OK, 0x5EED),
        Box::new(move |_| {
            counted.fetch_add(1, Ordering::SeqCst);
            E_OUTOFMEMORY_HRESULT
        }),
    )
    .expect("the canned acquire succeeds");
    assert_eq!(usage.release(), Err(E_OUTOFMEMORY_HRESULT));
    assert!(matches!(&usage, MtaUsage::Released));
    assert_eq!(usage.release(), Ok(()));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn mta_usage_success_and_failure_split_on_hresult_sign() {
    assert_eq!(classify_mta_usage_hresult(S_OK), Ok(()));
    assert_eq!(
        classify_mta_usage_hresult(E_OUTOFMEMORY_HRESULT),
        Err(E_OUTOFMEMORY_HRESULT)
    );
    assert_eq!(classify_mta_usage_hresult(1), Ok(()));
}

#[test]
fn a_registration_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<MtaUsage>();
}

#[cfg(target_os = "windows")]
#[test]
fn a_real_registration_acquires_and_releases_cleanly() {
    let mut usage = match MtaUsage::acquire() {
        Ok(usage) => usage,
        Err(hresult) => panic!("CoIncrementMTAUsage failed with HRESULT 0x{hresult:08X}"),
    };
    assert_eq!(usage.release(), Ok(()));
}
