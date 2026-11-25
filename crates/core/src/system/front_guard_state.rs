/// Where a front guard stands. `Clean`, `GivenBack` and `GiveBackFailed` are
/// the three ways a guard ends; `Watching` and `Intruded` are the two it
/// passes through while the command runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FrontGuardState {
    Watching,
    Intruded,
    Clean,
    GivenBack,
    GiveBackFailed,
}
