use std::collections::VecDeque;

/// The in-process interaction lease as a first-in first-out queue.
///
/// The lease is either `Free` or `Held`. A waiter takes a ticket on arrival
/// and is granted the lease only when it is at the head of the queue and the
/// lease is `Free`, so a releaser that comes straight back queues behind every
/// thread that already waited. A waiter that gives up leaves the queue and
/// never takes the lease.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LeaseQueue {
    slot: Slot,
    waiting: VecDeque<u64>,
    next_ticket: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    Free,
    Held,
}

impl LeaseQueue {
    pub(crate) const fn new() -> Self {
        Self {
            slot: Slot::Free,
            waiting: VecDeque::new(),
            next_ticket: 0,
        }
    }

    pub(crate) fn enqueue(&mut self) -> u64 {
        let ticket = self.next_ticket;
        self.next_ticket += 1;
        self.waiting.push_back(ticket);
        ticket
    }

    pub(crate) fn try_grant(&mut self, ticket: u64) -> bool {
        if self.slot == Slot::Free && self.waiting.front() == Some(&ticket) {
            self.waiting.pop_front();
            self.slot = Slot::Held;
            return true;
        }
        false
    }

    pub(crate) fn abandon(&mut self, ticket: u64) {
        self.waiting.retain(|waiting| *waiting != ticket);
    }

    pub(crate) fn release(&mut self) {
        self.slot = Slot::Free;
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.slot == Slot::Free && self.waiting.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn is_held(&self) -> bool {
        self.slot == Slot::Held
    }
}

#[cfg(test)]
#[path = "process_lease_queue_tests.rs"]
mod tests;
