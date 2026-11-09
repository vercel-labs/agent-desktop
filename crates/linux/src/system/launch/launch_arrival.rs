use agent_desktop_core::LaunchFacts;

/// How the application a launch reports came to be its owner. The launch
/// names the owner, not the process it spawned: a launcher can hand off to an
/// application that was already running, or fork the application away from
/// the process it spawned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Arrival {
    Attached,
    Spawned,
    Appeared,
    Forwarded,
    Unlisted,
}

impl Arrival {
    pub(super) const fn started(owner_is_spawned: bool, new_instance: bool) -> Self {
        if owner_is_spawned {
            Self::Spawned
        } else if new_instance {
            Self::Appeared
        } else {
            Self::Forwarded
        }
    }

    pub(super) const fn created(self) -> bool {
        matches!(self, Self::Spawned)
    }

    pub(super) const fn facts(self) -> LaunchFacts {
        LaunchFacts {
            attached: matches!(self, Self::Attached),
            activation_dispatched: false,
            postcondition_observed: matches!(self, Self::Spawned | Self::Appeared),
        }
    }
}

#[cfg(test)]
#[path = "launch_arrival_tests.rs"]
mod tests;
