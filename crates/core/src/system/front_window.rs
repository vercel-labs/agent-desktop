/// One window as the front guard compares it. `id` is opaque to core: only the
/// adapter that produced it reads it, and core only tests it for equality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontWindow {
    pub id: String,
    pub pid: u32,
}
