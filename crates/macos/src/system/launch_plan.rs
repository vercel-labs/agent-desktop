//! What a macOS launch does about the instances already running, decided from
//! what the launch arguments mean before anything is sent.
//!
//! Launch Services hands an argument to an application in one of two ways. An
//! argument that names an existing file is a document: Launch Services opens
//! it with the application, which grants a sandboxed application access to it
//! and delivers it to an instance that is already running. Any other argument
//! is process arguments, which only a process that is starting can receive.

use agent_desktop_core::system::launch_options::LaunchOptions;

pub(crate) struct LaunchArguments {
    pub(crate) documents: Vec<String>,
    pub(crate) plain: Vec<String>,
}

impl LaunchArguments {
    pub(crate) fn classify(arguments: &[String]) -> Self {
        Self::classify_with(arguments, resolve_document)
    }

    pub(crate) fn classify_with(
        arguments: &[String],
        resolve: impl Fn(&str) -> Option<String>,
    ) -> Self {
        let mut classified = Self {
            documents: Vec::new(),
            plain: Vec::new(),
        };
        for argument in arguments {
            match resolve(argument).filter(|_| !argument.starts_with('-')) {
                Some(path) => classified.documents.push(path),
                None => classified.plain.push(argument.clone()),
            }
        }
        classified
    }
}

fn resolve_document(argument: &str) -> Option<String> {
    std::fs::canonicalize(argument)
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
}

/// The one move a launch makes. Every plan except the refusals and
/// `Ambiguous` sends something; `Attach` sends nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LaunchPlan {
    Fresh,
    Attach,
    Activate,
    Forward,
    RefuseRunning,
    RefuseArguments,
    Ambiguous,
}

pub(crate) fn plan(
    running: usize,
    options: &LaunchOptions,
    arguments: &LaunchArguments,
) -> LaunchPlan {
    match (running, options.attach_if_running) {
        (0, _) => LaunchPlan::Fresh,
        (_, false) => LaunchPlan::RefuseRunning,
        (1, true) if !arguments.plain.is_empty() => LaunchPlan::RefuseArguments,
        (1, true) if !arguments.documents.is_empty() => LaunchPlan::Forward,
        (1, true) if options.activate => LaunchPlan::Activate,
        (1, true) => LaunchPlan::Attach,
        (_, true) => LaunchPlan::Ambiguous,
    }
}

#[cfg(test)]
#[path = "launch_plan_tests.rs"]
mod tests;
