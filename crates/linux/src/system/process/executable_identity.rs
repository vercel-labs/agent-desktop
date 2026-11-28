use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

/// Executable evidence shared by launch attachment and desktop aliases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecutableIdentity {
    pub(crate) executable: PathBuf,
    pub(crate) argv: Option<Vec<OsString>>,
}

impl ExecutableIdentity {
    pub(crate) fn requested(argv: &[String], path: &[PathBuf]) -> Option<Self> {
        let program = argv.first()?;
        let file = if program.contains('/') {
            let file = PathBuf::from(program);
            if !file.is_absolute() {
                return None;
            }
            file
        } else {
            crate::system::launch::launch_target::find_on_path(program, path)?
        };
        let executable = file.canonicalize().ok()?;
        let argv = Some({
            std::iter::once(file.into_os_string())
                .chain(argv[1..].iter().map(OsString::from))
                .collect()
        });
        Some(Self { executable, argv })
    }

    pub(crate) fn read(pid: u32) -> Option<Self> {
        let executable = std::fs::canonicalize(format!("/proc/{pid}/exe")).ok()?;
        let argv = std::fs::read(format!("/proc/{pid}/cmdline"))
            .ok()
            .and_then(|bytes| {
                let words: Vec<OsString> = bytes
                    .strip_suffix(&[0])?
                    .split(|b| *b == 0)
                    .map(|word| OsString::from_vec(word.to_vec()))
                    .collect();
                Some(words)
            });
        Some(Self { executable, argv })
    }

    /// How a desktop entry's launcher names `running`: the same executable
    /// under the same invocation name, or an interpreter running the
    /// launcher's script. `Exact` means every argument of the entry appears
    /// in the process's arguments, in order, with only bare long options
    /// between them. `Launcher` is the looser match for a service, where the
    /// entry and the process pass different bare long options
    /// (`--new-window` against `--gapplication-service`) and every other word
    /// leads the process's own, so an interpreter entry never names another
    /// script.
    pub(crate) fn naming(&self, running: &Self) -> Option<Naming> {
        let (argv, actual) = (self.argv.as_deref()?, running.argv.as_deref()?);
        let actual = if self.executable == running.executable && same_invocation(argv, actual) {
            actual
        } else {
            script_run(&self.executable, &running.executable, actual)?
        };
        let arguments = &argv[1.min(argv.len())..];
        if holds_in_order(arguments, &actual[1.min(actual.len())..]) {
            return Some(Naming::Exact(arguments.len()));
        }
        let (wanted, found) = (compared(argv), compared(actual));
        (found.len() >= wanted.len() && found[..wanted.len()] == wanted[..])
            .then_some(Naming::Launcher)
    }
}

/// How strongly a desktop entry names a running process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Naming {
    Launcher,
    Exact(usize),
}

fn holds_in_order(wanted: &[OsString], found: &[OsString]) -> bool {
    consume_in_order(wanted, &mut found.iter())
}

fn consume_in_order(wanted: &[OsString], rest: &mut std::slice::Iter<'_, OsString>) -> bool {
    wanted.iter().all(|word| {
        rest.by_ref()
            .find(|candidate| *candidate == word || !is_bare_long_option(candidate))
            .is_some_and(|candidate| candidate == word)
    })
}

fn is_bare_long_option(word: &OsString) -> bool {
    let bytes = word.as_encoded_bytes();
    bytes.len() > 2 && bytes.starts_with(b"--") && !bytes.contains(&b'=')
}

/// The process arguments from the script on, when `actual` runs `script`:
/// the process executable is the interpreter the script's `#!` line names,
/// the words between `actual[0]` and the script path match the exact
/// interpreter options declared on that `#!` line, and the script path sits
/// immediately after them. Extra interpreter options or code-evaluation flags
/// are refused so an option operand cannot spoof a script launcher.
fn script_run<'a>(
    script: &Path,
    interpreter: &Path,
    actual: &'a [OsString],
) -> Option<&'a [OsString]> {
    let (expected_interpreter, shebang_args) = shebang_spec(script)?;
    if expected_interpreter != interpreter {
        return None;
    }
    let index = 1 + shebang_args.len();
    if actual.get(1..index)? != shebang_args.as_slice() {
        return None;
    }
    let word = actual.get(index)?;
    (word.as_encoded_bytes().contains(&b'/') && std::fs::canonicalize(word).ok()? == script)
        .then(|| &actual[index..])
}

fn shebang_spec(script: &Path) -> Option<(PathBuf, Vec<OsString>)> {
    use std::io::Read;
    let mut head = [0_u8; 256];
    let read = std::fs::File::open(script).ok()?.read(&mut head).ok()?;
    let line = std::str::from_utf8(
        head[..read]
            .strip_prefix(b"#!")?
            .split(|byte| *byte == b'\n')
            .next()?,
    )
    .ok()?
    .trim();
    let (program_str, rest) = match line.split_once(char::is_whitespace) {
        Some((program, rest)) => (program, rest.trim()),
        None => (line, ""),
    };
    if program_str.is_empty() {
        return None;
    }
    let program = PathBuf::from(program_str);
    if program.file_name()? == "env" {
        return env_shebang_spec(rest);
    }
    let args = if rest.is_empty() {
        Vec::new()
    } else {
        vec![OsString::from(rest)]
    };
    Some((program.canonicalize().ok()?, args))
}

fn env_shebang_spec(rest: &str) -> Option<(PathBuf, Vec<OsString>)> {
    let mut words = rest.split_whitespace();
    let first = words.next()?;
    let (named, args) = if first == "-S" {
        let named = words.next()?;
        if named.starts_with('-') {
            return None;
        }
        (named, words.map(OsString::from).collect())
    } else if first.starts_with('-') || words.next().is_some() {
        return None;
    } else {
        (first, Vec::new())
    };
    let path = crate::system::launch::launch_search::Search::from_env().path_dirs;
    let interpreter = crate::system::launch::launch_target::find_on_path(named, &path)?
        .canonicalize()
        .ok()?;
    Some((interpreter, args))
}

impl ExecutableIdentity {
    /// Whether a launch with this identity is the running process: the same
    /// executable under the same invocation name, with every launch argument
    /// and no other argument except the bare long options the process was
    /// started with. A service started with `--gapplication-service` is the
    /// bare launch it serves. A launch that asks for a mode, such as `--gui`,
    /// is not a process running in another mode, and a launch is never a
    /// process running other code or another file.
    #[cfg(test)]
    pub(crate) fn is_running_as(&self, running: &Self) -> bool {
        self.is_running_as_with(running, false)
    }

    /// `is_running_as`, where the running process may also carry documents it
    /// was asked to open. A launcher whose command takes files or URLs is
    /// the same application with or without them; for any other command a
    /// path is code or another file, as with an interpreter's script.
    pub(crate) fn is_running_as_with(&self, running: &Self, documents: bool) -> bool {
        let Some(argv) = self.argv.as_deref() else {
            return false;
        };
        let Some(actual) = self.program_arguments(running) else {
            return false;
        };
        if !same_invocation(argv, actual) {
            return false;
        }
        let mut rest = actual[1..].iter();
        consume_in_order(&argv[1..], &mut rest)
            && rest.all(|word| {
                is_bare_long_option(word)
                    || is_short_switch(word)
                    || (documents && is_document(word))
            })
    }

    /// Whether the running process is this launch's program: the same
    /// executable image, or the interpreter running the launch's script.
    pub(crate) fn same_program(&self, running: &Self) -> bool {
        self.executable == running.executable || self.program_arguments(running).is_some()
    }

    /// The process's words from the program on. A process of the launch's own
    /// image is read whole. When the launch is a script and the process is the
    /// interpreter the script's `#!` line names, running that script, the
    /// words are the ones from the script on, so the script path stands where
    /// the launch's program stands. Any other process has none.
    fn program_arguments<'a>(&self, running: &'a Self) -> Option<&'a [OsString]> {
        let actual = running.argv.as_deref()?;
        if self.executable == running.executable {
            return Some(actual);
        }
        script_run(&self.executable, &running.executable, actual)
    }
}

/// An absolute path or a URL: what an application is asked to open.
fn is_document(word: &OsString) -> bool {
    let bytes = word.as_encoded_bytes();
    bytes.starts_with(b"/") || bytes.windows(3).any(|window| window == b"://")
}

/// A single-letter switch such as `-b`: a mode of the process, never other
/// code or another file, so a launch without it still names the process.
fn is_short_switch(word: &OsString) -> bool {
    let bytes = word.as_encoded_bytes();
    bytes.len() == 2 && bytes[0] == b'-' && bytes[1] != b'-'
}

fn compared(argv: &[OsString]) -> Vec<&OsString> {
    argv.iter()
        .skip(1)
        .filter(|word| !is_bare_long_option(word))
        .collect()
}

fn same_invocation(expected: &[OsString], actual: &[OsString]) -> bool {
    let (Some(program), Some(invocation)) = (expected.first(), actual.first()) else {
        return false;
    };
    program == invocation
        || (!invocation.as_encoded_bytes().contains(&b'/')
            && Path::new(program).file_name() == Some(invocation.as_os_str()))
}

#[cfg(test)]
#[path = "executable_identity_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "executable_identity_switch_tests.rs"]
mod switch_tests;

#[cfg(test)]
#[path = "executable_identity_script_tests.rs"]
mod script_tests;
