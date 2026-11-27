//! Single-instance lock: exactly one engine per data directory (PLAN §2.1).
//!
//! The lock is an OS advisory lock on `engine.lock` (`flock` on Unix, `LockFileEx` on Windows,
//! via `std::fs::File::try_lock`). The OS drops it when the process dies, so a crashed engine
//! never leaves a stale lock behind. The holder's pid and port go in a separate file,
//! `engine.holder`, for diagnostics only; nothing trusts that text. (It can't live in the lock
//! file itself: Windows locks are mandatory, so a second engine couldn't read it.)

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

/// Held for the life of the engine; dropping it releases the lock.
#[derive(Debug)]
pub struct InstanceLock {
    _file: File,
    path: PathBuf,
    holder: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum LockError {
    #[error(
        "another noPlayout engine is already running with this data directory ({path}); \
         {holder}"
    )]
    Held { path: PathBuf, holder: String },
    #[error("could not open the engine lock file {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl InstanceLock {
    /// Take the lock in `data_dir`, creating the directory if needed.
    pub fn acquire(data_dir: &Path) -> Result<Self, LockError> {
        let path = data_dir.join("engine.lock");
        let holder = data_dir.join("engine.holder");
        let io_err = |source| LockError::Io {
            path: path.clone(),
            source,
        };
        std::fs::create_dir_all(data_dir).map_err(io_err)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(io_err)?;
        match file.try_lock() {
            Ok(()) => Ok(Self {
                _file: file,
                path,
                holder,
            }),
            Err(TryLockError::WouldBlock) => {
                let holder = std::fs::read_to_string(&holder)
                    .ok()
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .map(|s| format!("holder: {s}"))
                    .unwrap_or_else(|| "holder unknown".into());
                Err(LockError::Held { path, holder })
            }
            Err(TryLockError::Error(e)) => Err(io_err(e)),
        }
    }

    /// Record who holds the lock (pid and HTTP port), for the "already running" message.
    pub fn record(&mut self, port: u16) -> std::io::Result<()> {
        let text = format!("pid {} port {port}\n", std::process::id());
        std::fs::write(&self.holder, text)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "noplayout-lock-{tag}-{}-{}",
            std::process::id(),
            crate::new_token()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn second_acquire_is_refused_until_the_first_is_dropped() {
        let dir = temp_dir("second");
        let mut first = InstanceLock::acquire(&dir).expect("first lock");
        first.record(8686).unwrap();
        let err = InstanceLock::acquire(&dir).expect_err("second lock must fail");
        let text = err.to_string();
        assert!(text.contains("already running"), "{text}");
        assert!(text.contains("port 8686"), "holder recorded: {text}");
        drop(first);
        InstanceLock::acquire(&dir).expect("lock is free again after drop");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn creates_the_data_directory() {
        let dir = temp_dir("create").join("nested").join("data");
        let lock = InstanceLock::acquire(&dir).expect("lock in a new directory");
        assert!(lock.path().exists());
    }
}
