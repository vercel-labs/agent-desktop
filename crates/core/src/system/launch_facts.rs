use crate::DeliverySemantics;

/// What a launch did and what it read back, reduced to the one delivery it can
/// honestly claim. Every adapter fills these three facts in and takes the
/// answer from `disposition`, so the rule exists once.
///
/// `attached` means the launch was satisfied by an already running instance
/// and spawned nothing. `activation_dispatched` means an activation request
/// was sent to the instance. `postcondition_observed` means the adapter read
/// back the effect it dispatched: a process instance absent before the
/// launch, or the target as the frontmost application after an activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchFacts {
    pub attached: bool,
    pub activation_dispatched: bool,
    pub postcondition_observed: bool,
}

impl LaunchFacts {
    pub const fn disposition(self) -> DeliverySemantics {
        let dispatched = !self.attached || self.activation_dispatched;
        if !dispatched {
            return DeliverySemantics::not_delivered();
        }
        if self.postcondition_observed {
            return DeliverySemantics::delivered_verified();
        }
        DeliverySemantics::delivered_unverified()
    }
}

#[cfg(test)]
#[path = "launch_facts_tests.rs"]
mod tests;
