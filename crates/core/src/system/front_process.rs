/// What the front guard needs to know about a process to say whether a new
/// window is its target's: the name of its executable and when it started, in
/// milliseconds since the Unix epoch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontProcess {
    pub image: String,
    pub started_ms: u64,
}
