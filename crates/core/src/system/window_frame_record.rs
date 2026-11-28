use serde::{Deserialize, Serialize};

use crate::{ProcessIdentity, Rect};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowFrameRecord {
    pub process: ProcessIdentity,
    pub before: Rect,
    pub maximized: Rect,
}
