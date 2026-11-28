use crate::actions::delivery_observation::DeliveryObservation;

/// How one rung of a delivery chain ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryOutcome {
    /// Nothing was sent; the chain tries the next rung.
    NotDelivered,
    /// The element was already in the wanted state; nothing was sent.
    Satisfied,
    /// The action was sent, with what the rung read back afterwards.
    Delivered(DeliveryObservation),
}

impl DeliveryOutcome {
    /// The outcome of a rung that sent the action when `delivered` and saw
    /// its effect when `verified`.
    pub fn from_delivery(delivered: bool, verified: bool) -> Self {
        match (delivered, verified) {
            (false, _) => Self::NotDelivered,
            (true, true) => Self::Delivered(DeliveryObservation::Verified),
            (true, false) => Self::Delivered(DeliveryObservation::Unverified),
        }
    }

    /// The outcome of a rung that sent the action and read back `read`, as
    /// [`DeliveryObservation::of_read`] judges it.
    pub fn observed(read: Option<bool>) -> Self {
        Self::Delivered(DeliveryObservation::of_read(read))
    }
}
