use std::fs::File;
use std::path::Path;

use crate::storage::file_lock_wait::{
    LockWaitFailure, abandoned_waiter_epoch, settle_abandoned_waiters, wait_for_lock,
};
use crate::{AdapterError, Deadline, ErrorCode};

/// A private state-file lock held until this guard is dropped.
///
/// Releases by closing rather than unlocking: a duplicated descriptor shares
/// the lock, so unlocking would release it for the other holder too.
pub struct FileLock {
    _file: File,
    #[cfg(unix)]
    contention_count: u64,
}

impl FileLock {
    pub fn acquire(path: &Path, deadline: Deadline, purpose: &str) -> Result<Self, AdapterError> {
        let file = crate::storage::private_file::open_private_lock(path, true).map_err(io_error)?;
        lock_file(file, deadline, purpose, path)
    }

    pub(crate) fn is_held(path: &Path) -> bool {
        let Ok(file) = crate::storage::private_file::open_private_lock(path, false) else {
            return false;
        };
        match file.try_lock() {
            Ok(()) => {
                let _ = file.unlock();
                false
            }
            Err(std::fs::TryLockError::WouldBlock) => true,
            Err(std::fs::TryLockError::Error(_)) => true,
        }
    }

    #[cfg(unix)]
    pub(crate) fn contention_count(&self) -> u64 {
        self.contention_count
    }

    #[cfg(unix)]
    pub(crate) fn duplicate_inheritable(&self) -> Result<std::os::fd::OwnedFd, AdapterError> {
        use std::os::fd::{AsRawFd, FromRawFd};

        let duplicated = unsafe { libc::fcntl(self._file.as_raw_fd(), libc::F_DUPFD, 3) };
        if duplicated < 0 {
            return Err(io_error(std::io::Error::last_os_error()));
        }
        Ok(unsafe { std::os::fd::OwnedFd::from_raw_fd(duplicated) })
    }

    #[cfg(unix)]
    pub(crate) fn adopt_inherited(
        raw_fd: std::os::fd::RawFd,
        canonical_path: &Path,
        deadline: Deadline,
        purpose: &str,
    ) -> Result<Self, AdapterError> {
        use std::os::fd::{FromRawFd, OwnedFd};
        use std::os::unix::fs::MetadataExt;

        let duplicated = unsafe { libc::fcntl(raw_fd, libc::F_DUPFD_CLOEXEC, 3) };
        if duplicated < 0 {
            return Err(io_error(std::io::Error::last_os_error()));
        }
        let original_flags = unsafe { libc::fcntl(raw_fd, libc::F_GETFD) };
        if original_flags < 0
            || unsafe { libc::fcntl(raw_fd, libc::F_SETFD, original_flags | libc::FD_CLOEXEC) } != 0
        {
            unsafe { libc::close(duplicated) };
            return Err(io_error(std::io::Error::last_os_error()));
        }
        let inherited = File::from(unsafe { OwnedFd::from_raw_fd(duplicated) });
        let inherited_meta =
            crate::storage::private_file::validate_private_regular(&inherited, canonical_path)
                .map_err(io_error)?;
        let canonical = crate::storage::private_file::open_private_lock(canonical_path, false)
            .map_err(io_error)?;
        let canonical_meta = canonical.metadata().map_err(io_error)?;
        if inherited_meta.dev() != canonical_meta.dev()
            || inherited_meta.ino() != canonical_meta.ino()
        {
            return Err(AdapterError::new(
                ErrorCode::PolicyDenied,
                "Inherited interaction lease does not identify the canonical lock file",
            ));
        }
        lock_file(inherited, deadline, purpose, canonical_path)
    }
}

/// Takes the lock, blocking in the kernel rather than polling. A poller that
/// sleeps between tries loses to a process that releases and asks again at once;
/// a blocked waiter is woken on release and takes its turn. Dropping the file
/// closes it without unlocking: shared file descriptions need drop, not unlock,
/// and on the adopt path shared descriptions exist and must be left alone.
fn lock_file(
    file: File,
    deadline: Deadline,
    purpose: &str,
    path: &Path,
) -> Result<FileLock, AdapterError> {
    if deadline.is_expired() {
        return Err(lock_timeout(deadline, purpose, path, 0));
    }
    let mut contention_count = 0_u64;
    let file = loop {
        let before_epoch = abandoned_waiter_epoch();
        match file.try_lock() {
            Ok(()) => break file,
            Err(std::fs::TryLockError::Error(error)) => {
                return Err(acquire_failure(purpose, &error.to_string()));
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                contention_count = 1;
                if settle_abandoned_waiters(path, deadline, before_epoch) {
                    continue;
                }
                break wait_for_lock(file, deadline, path).map_err(|failure| match failure {
                    LockWaitFailure::TimedOut => lock_timeout(deadline, purpose, path, 1),
                    LockWaitFailure::Failed(message) => acquire_failure(purpose, &message),
                })?;
            }
        }
    };
    if deadline.is_expired() {
        return Err(lock_timeout(deadline, purpose, path, contention_count));
    }
    Ok(FileLock {
        _file: file,
        #[cfg(unix)]
        contention_count,
    })
}

fn acquire_failure(purpose: &str, message: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::Internal,
        format!("Failed to acquire {purpose}: {message}"),
    )
}

fn lock_timeout(
    deadline: Deadline,
    purpose: &str,
    path: &Path,
    contention_count: u64,
) -> AdapterError {
    lock_wait_timeout(
        deadline,
        purpose,
        serde_json::json!({
            "kind": "lock_timeout",
            "purpose": purpose,
            "path": path.file_name().map(|name| name.to_string_lossy().into_owned()),
            "contention_count": contention_count,
        }),
    )
}

/// The `TIMEOUT` error for a lock that another agent-desktop command held for
/// the whole wait. The disposition stays unknown: only a caller that takes the
/// lock before any delivery can claim `not_delivered`.
pub fn lock_wait_timeout(
    deadline: Deadline,
    purpose: &str,
    details: serde_json::Value,
) -> AdapterError {
    AdapterError::timeout(format!(
        "Another agent-desktop command held the {purpose} for the whole {}ms wait",
        deadline.timeout_ms()
    ))
    .with_suggestion("Retry the command, or wait for the other agent-desktop command to finish")
    .with_details(serde_json::json!({
        "timeout_ms": deadline.timeout_ms(),
        "elapsed_ms": deadline.elapsed().as_millis(),
    }))
    .with_details(details)
}

fn io_error(error: std::io::Error) -> AdapterError {
    AdapterError::from_io(&error)
        .cloned()
        .unwrap_or_else(|| AdapterError::new(ErrorCode::Internal, error.to_string()))
}
