/// An effect a platform declares for an action, with the reason it expects
/// it, which a refusal reports as `details.kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeclaredEffect {
    pub effect: crate::Effect,
    pub kind: &'static str,
}
