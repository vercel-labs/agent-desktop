use crate::{AdapterError, Effect, actions::delivery_outcome::DeliveryOutcome};

/// One mechanism a delivery chain may try, with the foreground effects it has.
pub struct ChainRung<'a> {
    pub label: &'static str,
    pub effects: &'static [Effect],
    pub run: Box<dyn FnMut() -> Result<DeliveryOutcome, AdapterError> + 'a>,
}
