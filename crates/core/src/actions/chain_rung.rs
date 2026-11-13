use crate::{AdapterError, Effect, StepMechanism, actions::delivery_outcome::DeliveryOutcome};

/// One mechanism a delivery chain may try: its label, its foreground effects,
/// how it delivers, and its run.
pub struct ChainRung<'a> {
    pub label: &'static str,
    pub effects: &'static [Effect],
    pub mechanism: StepMechanism,
    pub run: Box<dyn FnMut() -> Result<DeliveryOutcome, AdapterError> + 'a>,
}
