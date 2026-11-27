use std::time::{Duration, Instant};

use super::{GHOSTS, abandoned_waiter_epoch, settle_abandoned_waiters};
use crate::refs::refs_test_support::HomeGuard;
use crate::{Deadline, FileLock};

fn lock_path(home: &HomeGuard) -> std::path::PathBuf {
    let directory = home.path().join("lockwait");
    crate::storage::private_file_parent::ensure_private(&directory).unwrap();
    directory.join("test.lock")
}

#[test]
fn a_waiter_that_times_out_leaves_no_lock_behind_when_the_kernel_grants_late() {
    let home = HomeGuard::new();
    let path = lock_path(&home);
    let holder = FileLock::acquire(&path, Deadline::after(1_000).unwrap(), "holder").unwrap();
    let error = FileLock::acquire(&path, Deadline::after(60).unwrap(), "waiter")
        .err()
        .expect("the held lock times out");
    assert_eq!(error.code, crate::ErrorCode::Timeout);
    drop(holder);

    let started = Instant::now();
    let next = FileLock::acquire(&path, Deadline::after(3_000).unwrap(), "next")
        .expect("a late grant must not keep the lock");
    assert!(started.elapsed() < Duration::from_millis(2_000));
    drop(next);
}

#[test]
fn a_blocked_waiter_takes_the_lock_as_soon_as_the_holder_releases() {
    let home = HomeGuard::new();
    let path = lock_path(&home);
    let holder = FileLock::acquire(&path, Deadline::after(1_000).unwrap(), "holder").unwrap();
    let waiter_path = path.clone();
    let waiter = std::thread::spawn(move || {
        let started = Instant::now();
        let lock = FileLock::acquire(&waiter_path, Deadline::after(5_000).unwrap(), "waiter");
        (lock.is_ok(), started.elapsed())
    });
    std::thread::sleep(Duration::from_millis(100));
    drop(holder);

    let (acquired, waited) = waiter.join().unwrap();
    assert!(acquired);
    assert!(waited < Duration::from_millis(500), "waited {waited:?}");
}

#[test]
fn three_agents_looping_on_one_lock_take_turns_instead_of_one_repeating() {
    let home = HomeGuard::new();
    let path = lock_path(&home);
    let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    #[allow(clippy::needless_collect)]
    let workers: Vec<_> = (0..3_usize)
        .map(|worker| {
            let path = path.clone();
            let order = order.clone();
            std::thread::spawn(move || {
                for _ in 0..12 {
                    let lock = FileLock::acquire(&path, Deadline::after(30_000).unwrap(), "loop")
                        .expect("every waiter is eventually served");
                    order.lock().unwrap().push(worker);
                    std::thread::sleep(Duration::from_millis(20));
                    drop(lock);
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    let order = order.lock().unwrap();
    let longest_streak = order
        .chunk_by(|left, right| left == right)
        .map(<[usize]>::len)
        .max()
        .unwrap();

    assert!(
        longest_streak <= 4,
        "one agent hogged the lock instead of queueing FIFO: {order:?}"
    );
}

#[test]
fn a_timed_out_kernel_waiter_releases_its_late_grant_without_holding_the_file() {
    let home = HomeGuard::new();
    let path = lock_path(&home);
    let holder = crate::storage::private_file::open_private_lock(&path, true).unwrap();
    holder.lock().unwrap();
    let waiter = crate::storage::private_file::open_private_lock(&path, false).unwrap();
    let error = super::wait_for_lock(waiter, Deadline::after(40).unwrap(), &path);
    assert!(matches!(error, Err(super::LockWaitFailure::TimedOut)));
    drop(holder);

    let next = FileLock::acquire(&path, Deadline::after(3_000).unwrap(), "next")
        .expect("a late kernel grant must drop and free the lock");
    drop(next);
}

#[test]
fn a_timed_out_waiter_then_release_then_a_new_acquire_succeeds_under_a_frozen_clock() {
    let home = HomeGuard::new();
    let path = lock_path(&home);
    for _ in 0..20 {
        let clock = crate::wait::deadline::VirtualClock::freeze();
        let held = FileLock::acquire(&path, Deadline::after(2_000).unwrap(), "held").unwrap();
        let timed_out = FileLock::acquire(&path, Deadline::after(2_000).unwrap(), "late")
            .err()
            .expect("a held lock times out");
        assert_eq!(timed_out.code, crate::ErrorCode::Timeout);
        assert_eq!(clock.elapsed(), Duration::from_millis(2_000));
        drop(held);

        let next = FileLock::acquire(&path, Deadline::after(2_000).unwrap(), "next")
            .expect("the abandoned waiter must not take the next acquirer's turn");
        drop(next);
    }
}

#[test]
fn an_acquirer_waits_for_an_abandoned_helper_to_finish_before_reading_the_lock_as_held() {
    let path = std::path::PathBuf::from("/abandoned-helper-registry-probe");
    GHOSTS.appeared(&path);
    let before = abandoned_waiter_epoch(&path);
    let finisher_path = path.clone();
    let finisher = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(30));
        GHOSTS.vanished(&finisher_path);
    });

    let settled = settle_abandoned_waiters(&path, Deadline::after(5_000).unwrap(), before);

    finisher.join().unwrap();
    assert!(settled);
    assert!(!settle_abandoned_waiters(
        &path,
        Deadline::after(5_000).unwrap(),
        abandoned_waiter_epoch(&path),
    ));
}

#[test]
fn an_abandoned_helper_that_vanishes_during_try_lock_still_prompts_a_retry() {
    let path = std::path::PathBuf::from("/abandoned-helper-vanished-mid-try-probe");
    GHOSTS.appeared(&path);
    let before = abandoned_waiter_epoch(&path);
    GHOSTS.vanished(&path);

    assert!(settle_abandoned_waiters(
        &path,
        Deadline::after(5_000).unwrap(),
        before,
    ));
    assert!(!settle_abandoned_waiters(
        &path,
        Deadline::after(5_000).unwrap(),
        abandoned_waiter_epoch(&path),
    ));
}

#[test]
fn an_abandoned_helper_that_never_finishes_costs_the_acquirer_only_the_grace() {
    let path = std::path::PathBuf::from("/abandoned-helper-never-finishes-probe");
    GHOSTS.appeared(&path);

    let started = Instant::now();
    let settled = settle_abandoned_waiters(
        &path,
        Deadline::after(5_000).unwrap(),
        abandoned_waiter_epoch(&path),
    );

    GHOSTS.vanished(&path);
    assert!(!settled);
    assert!(started.elapsed() < Duration::from_secs(2));
}
