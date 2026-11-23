//! Where the renderer for one display and state root listens, and the lock
//! that keeps it to one process. Both live in a directory only this user can
//! enter: the user's runtime directory when the session has one, the state
//! root otherwise.
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use rustix::fs::{FlockOperation, flock};

const PRIVATE_DIR_MODE: u32 = 0o700;
/// The longest path a Unix socket address holds, less its terminating NUL.
const MAX_SOCKET_PATH: usize = 107;
const PRIVATE_FILE_MODE: u32 = 0o600;
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Endpoint {
    pub(crate) socket: PathBuf,
    pub(crate) lock: PathBuf,
}

fn fnv64(parts: &[&[u8]]) -> u64 {
    parts.iter().fold(FNV_OFFSET, |hash, part| {
        part.iter()
            .chain(std::iter::once(&0))
            .fold(hash, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
            })
    })
}

impl Endpoint {
    /// The endpoint of the renderer for `display` and `state_root`. The name
    /// is a hash of both, so two state roots on one display, or one state
    /// root on two displays, get separate renderers.
    pub(crate) fn for_display(
        display: &str,
        state_root: &Path,
        runtime_dir: Option<&Path>,
    ) -> Self {
        let directory = match runtime_dir.filter(|dir| dir.is_absolute()) {
            Some(runtime) => runtime.join("agent-desktop"),
            None => state_root.join("cursor"),
        };
        let name = format!(
            "cursor-{:016x}",
            fnv64(&[
                display.as_bytes(),
                state_root.as_os_str().as_encoded_bytes()
            ])
        );
        Self {
            socket: directory.join(format!("{name}.sock")),
            lock: directory.join(format!("{name}.lock")),
        }
    }

    /// Creates the endpoint's directory with mode 0700, or accepts an
    /// existing one only when this user owns it and no one else can enter.
    pub(crate) fn ensure_directory(&self) -> io::Result<()> {
        let directory = self.socket.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "the cursor socket has no directory",
            )
        })?;
        DirBuilder::new()
            .recursive(true)
            .mode(PRIVATE_DIR_MODE)
            .create(directory)?;
        let metadata = fs::symlink_metadata(directory)?;
        let private = metadata.is_dir()
            && metadata.uid() == rustix::process::getuid().as_raw()
            && metadata.permissions().mode() & 0o077 == 0;
        if private {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "the cursor renderer directory is not private to this user",
            ))
        }
    }

    /// Runs `reach` with a path to the socket that fits a socket address. A
    /// path too long for one, such as under a deeply nested state root, is
    /// reached through this process's descriptor of the socket's directory.
    pub(crate) fn with_reachable_socket<T>(
        &self,
        reach: impl FnOnce(&Path) -> io::Result<T>,
    ) -> io::Result<T> {
        if self.socket.as_os_str().len() <= MAX_SOCKET_PATH {
            return reach(&self.socket);
        }
        let (Some(directory), Some(name)) = (self.socket.parent(), self.socket.file_name()) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the cursor socket has no directory",
            ));
        };
        let held = File::open(directory)?;
        let short = PathBuf::from(format!("/proc/self/fd/{}", held.as_raw_fd())).join(name);
        reach(&short)
    }

    /// Takes the renderer lock without waiting. `None` means another renderer
    /// holds it; the lock is released when the returned file closes.
    pub(crate) fn try_lock(&self) -> io::Result<Option<File>> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(PRIVATE_FILE_MODE)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&self.lock)?;
        match flock(&file, FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => Ok(Some(file)),
            Err(rustix::io::Errno::WOULDBLOCK) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

#[cfg(test)]
#[path = "endpoint_tests.rs"]
mod tests;
