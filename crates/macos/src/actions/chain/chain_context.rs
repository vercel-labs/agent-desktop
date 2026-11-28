use agent_desktop_core::{Deadline, InteractionPolicy, Point};

#[derive(Clone, Copy)]
pub(crate) struct ChainContext<'a> {
    pub(crate) dynamic_value: Option<&'a str>,
    pub(crate) verified_point: Option<&'a Point>,
    pub(crate) deadline: Deadline,
    pub(crate) policy: InteractionPolicy,
}
