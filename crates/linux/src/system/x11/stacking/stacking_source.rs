use agent_desktop_core::Deadline;

use super::stacking_read::StackingRead;
use crate::system::x11::X11State;

/// Where stacking evidence comes from. The real source talks to the display
/// server; tests supply a fixed answer. `target_client` selects which
/// override-redirect windows count as above. The `_within` reads never wait
/// longer than the caller's deadline.
pub(crate) trait StackingSource: Send + Sync {
    fn read_within(&self, target_client: u32, deadline: Deadline) -> StackingRead;

    /// The managed clients, for finding the client of a listed window. A
    /// source that cannot list them says so, and no window maps to a client.
    fn clients_within(&self, _deadline: Deadline) -> X11State {
        X11State::Unavailable
    }
}
