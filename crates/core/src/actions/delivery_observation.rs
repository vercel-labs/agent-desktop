/// What a rung read back after it delivered the action. Only the rung knows
/// whether its read-back settled, so it decides between `NoEffect` and
/// `Unverified`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryObservation {
    /// The expected effect is observed.
    Verified,
    /// A settled read-back saw nothing change, so a later rung may try.
    NoEffect,
    /// The read-back did not show the effect but cannot prove that nothing
    /// changed; another rung could act twice.
    Unverified,
    /// Nothing could be read back.
    Unread,
}

impl DeliveryObservation {
    /// The observation of a read-back that is `Some(true)` when the effect is
    /// seen, `Some(false)` when it is not and `None` when unreadable.
    pub fn of_read(read: Option<bool>) -> Self {
        match read {
            Some(true) => Self::Verified,
            Some(false) => Self::Unverified,
            None => Self::Unread,
        }
    }

    /// The `verified` field a step that delivered with this observation records.
    pub fn verified(self) -> Option<bool> {
        match self {
            Self::Verified => Some(true),
            Self::NoEffect | Self::Unverified => Some(false),
            Self::Unread => None,
        }
    }
}

impl crate::DeliveryOutcome {
    /// The step a semantic rung that ended with this outcome records.
    pub fn step(self, label: &'static str) -> crate::ActionStep {
        let step = match self {
            Self::NotDelivered => crate::ActionStep::skipped(label),
            Self::Satisfied => crate::ActionStep::skipped(label).with_verified(true),
            Self::Delivered(observed) => match observed.verified() {
                Some(verified) => crate::ActionStep::succeeded(label).with_verified(verified),
                None => crate::ActionStep::succeeded(label),
            },
        };
        step.with_mechanism(crate::StepMechanism::SemanticApi)
    }
}
