use super::*;

#[test]
fn the_first_waiter_is_granted_a_free_lease() {
    let mut queue = LeaseQueue::new();
    let first = queue.enqueue();

    assert!(queue.try_grant(first));
    assert!(queue.is_held());
}

#[test]
fn a_later_waiter_is_refused_while_the_lease_is_held() {
    let mut queue = LeaseQueue::new();
    let first = queue.enqueue();
    let second = queue.enqueue();
    assert!(queue.try_grant(first));

    assert!(!queue.try_grant(second));
}

#[test]
fn a_waiter_behind_the_head_is_refused_even_when_the_lease_is_free() {
    let mut queue = LeaseQueue::new();
    let first = queue.enqueue();
    let second = queue.enqueue();

    assert!(!queue.try_grant(second));
    assert!(queue.try_grant(first));
}

#[test]
fn a_releaser_that_returns_at_once_queues_behind_the_waiter() {
    let mut queue = LeaseQueue::new();
    let holder = queue.enqueue();
    let waiter = queue.enqueue();
    assert!(queue.try_grant(holder));
    queue.release();
    let returning = queue.enqueue();

    assert!(!queue.try_grant(returning));
    assert!(queue.try_grant(waiter));
    queue.release();
    assert!(queue.try_grant(returning));
}

#[test]
fn an_abandoned_ticket_never_blocks_the_waiters_behind_it() {
    let mut queue = LeaseQueue::new();
    let gone = queue.enqueue();
    let next = queue.enqueue();
    queue.abandon(gone);

    assert!(queue.try_grant(next));
    assert!(!queue.try_grant(gone));
}

#[test]
fn release_frees_the_lease_and_a_free_lease_with_no_queue_is_idle() {
    let mut queue = LeaseQueue::new();
    let only = queue.enqueue();
    assert!(queue.try_grant(only));
    queue.release();

    assert!(!queue.is_held());
    assert_eq!(queue, {
        let mut idle = LeaseQueue::new();
        idle.next_ticket = 1;
        idle
    });
}
