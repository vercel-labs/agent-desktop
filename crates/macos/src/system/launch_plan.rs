//! What a macOS launch does about the instances already running, decided from
//! what the launch arguments mean before anything is sent.
//!
//! Launch Services hands an argument to an application in one of two ways. An
//! argument that is an explicit path to an existing file (absolute, `./`,
//! `~/`, or a `file://` URL) is a document: Launch Services opens it with the
//! application, which grants a sandboxed application access to it and
//! delivers it to an instance that is already running. Any other argument,
//! including a bare word that happens to name a file in the working
//! directory, is a process argument, which only a process that is starting
//! can receive.

use agent_desktop_core::system::launch_options::LaunchOptions;

pub(crate) struct LaunchArguments {
    pub(crate) documents: Vec<String>,
    pub(crate) plain: Vec<String>,
}

impl LaunchArguments {
    pub(crate) fn classify(arguments: &[String]) -> Self {
        let home = std::env::var("HOME").ok();
        Self::classify_with(arguments, home.as_deref(), |path| {
            std::fs::canonicalize(path)
                .ok()
                .map(|file| file.to_string_lossy().into_owned())
        })
    }

    pub(crate) fn classify_with(
        arguments: &[String],
        home: Option<&str>,
        resolve: impl Fn(&str) -> Option<String>,
    ) -> Self {
        let mut classified = Self {
            documents: Vec::new(),
            plain: Vec::new(),
        };
        for argument in arguments {
            match explicit_path(argument, home).and_then(|path| resolve(&path)) {
                Some(path) => classified.documents.push(path),
                None => classified.plain.push(argument.clone()),
            }
        }
        classified
    }
}

/// The path an argument names when it says so explicitly: an absolute path, a
/// path that starts with `./` or `~/`, or a `file://` URL. A bare word is
/// never a path, whatever the working directory holds, so it stays an
/// argument.
fn explicit_path(argument: &str, home: Option<&str>) -> Option<String> {
    if let Some(url) = argument.strip_prefix("file://") {
        let path = url.strip_prefix("localhost").unwrap_or(url);
        return path.starts_with('/').then(|| percent_decoded(path));
    }
    if argument.starts_with('/') || argument.starts_with("./") {
        return Some(argument.to_owned());
    }
    argument
        .strip_prefix("~/")
        .zip(home)
        .map(|(rest, home)| format!("{}/{rest}", home.trim_end_matches('/')))
}

fn percent_decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let escaped = (bytes[index] == b'%')
            .then(|| bytes.get(index + 1..index + 3))
            .flatten()
            .and_then(|pair| std::str::from_utf8(pair).ok())
            .and_then(|pair| u8::from_str_radix(pair, 16).ok());
        match escaped {
            Some(byte) => {
                decoded.push(byte);
                index += 3;
            }
            None => {
                decoded.push(bytes[index]);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
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
