use crate::LaunchFacts;

/// How a launch ended, in the one vocabulary every adapter reports it in. The
/// launch names the owner it found, not only the process it spawned: a launcher
/// can hand off to an application that was already running, fork the
/// application away from the process it spawned, or exit after starting an
/// application that another process now owns.
///
/// Each end state maps to one disposition. Only a state in which the adapter
/// read back an instance that did not exist before the launch is verified.
/// A state in which a request was sent and its effect was not seen is
/// unverified. Only a state in which nothing was sent is not delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchArrival {
    /// A running instance satisfied the launch and nothing was sent to it.
    Attached,
    /// The process this launch created owns the application.
    Spawned,
    /// The launcher exited and a process that did not exist before owns it.
    Appeared,
    /// The request was handed to an instance that was already running and
    /// its arrival was not seen.
    Forwarded,
    /// A process was created and nothing of the launch was observed: no
    /// owner on the platform's registry, or no window before the wait ended.
    Unobserved,
}

impl LaunchArrival {
    pub const fn started(owner_is_spawned: bool, new_instance: bool) -> Self {
        if owner_is_spawned {
            Self::Spawned
        } else if new_instance {
            Self::Appeared
        } else {
            Self::Forwarded
        }
    }

    pub const fn created(self) -> bool {
        matches!(self, Self::Spawned)
    }

    pub const fn facts(self) -> LaunchFacts {
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
