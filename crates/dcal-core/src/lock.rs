use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};
use thiserror::Error;

const LOCK_SUFFIX: &str = ".lock";
const RETRY_DELAY: Duration = Duration::from_secs(2);
const STALE_THRESHOLD: Duration = Duration::from_secs(120);

#[derive(Debug, Error)]
pub enum LockError {
    #[error("failed to acquire lock on {path}: file is locked by another process")]
    Contested { path: PathBuf },

    #[error("lock I/O error on {path}: {source}")]
    Io {
        path: PathBuf,
        source: io::Error,
    },
}

/// Advisory file lock with automatic cleanup on drop.
///
/// Creates a `.lock` file next to the target. If the lock file already
/// exists, retries once after a short delay. Lock files older than 120
/// seconds are treated as stale and reclaimed.
pub struct FileLock {
    lock_path: PathBuf,
}

impl FileLock {
    /// Acquire a lock for the given file path.
    ///
    /// Returns a `FileLock` guard that removes the lock file on drop.
    pub fn acquire(target: &Path) -> Result<Self, LockError> {
        let lock_path = lock_path_for(target);

        if try_create_lock(&lock_path)? {
            return Ok(Self { lock_path });
        }

        // Lock file exists — check if stale
        if is_stale(&lock_path) {
            reclaim_stale(&lock_path)?;
            if try_create_lock(&lock_path)? {
                return Ok(Self { lock_path });
            }
        }

        // Retry once after a delay
        thread::sleep(RETRY_DELAY);

        if try_create_lock(&lock_path)? {
            return Ok(Self { lock_path });
        }

        Err(LockError::Contested { path: lock_path })
    }

    /// Path to the lock file.
    pub fn path(&self) -> &Path {
        &self.lock_path
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}

fn lock_path_for(target: &Path) -> PathBuf {
    let mut name = target
        .file_name()
        .unwrap_or_default()
        .to_os_string();
    name.push(LOCK_SUFFIX);
    target.with_file_name(name)
}

/// Try to atomically create the lock file. Returns `true` if created,
/// `false` if it already exists.
fn try_create_lock(lock_path: &Path) -> Result<bool, LockError> {
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(lock_path)
    {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(source) => Err(LockError::Io {
            path: lock_path.to_path_buf(),
            source,
        }),
    }
}

fn is_stale(lock_path: &Path) -> bool {
    fs::metadata(lock_path)
        .and_then(|m| m.modified())
        .and_then(|modified| SystemTime::now().duration_since(modified).map_err(|e| {
            io::Error::other(e)
        }))
        .map(|age| age > STALE_THRESHOLD)
        .unwrap_or(false)
}

fn reclaim_stale(lock_path: &Path) -> Result<(), LockError> {
    fs::remove_file(lock_path).map_err(|source| LockError::Io {
        path: lock_path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("registry.json");
        fs::write(&target, "[]").unwrap();
        (dir, target)
    }

    #[test]
    fn acquire_creates_lock_file() {
        let (_dir, target) = setup();
        let lock = FileLock::acquire(&target).unwrap();
        assert!(lock.path().exists());
    }

    #[test]
    fn lock_path_has_correct_name() {
        let (_dir, target) = setup();
        let lock = FileLock::acquire(&target).unwrap();
        assert_eq!(
            lock.path().file_name().unwrap().to_str().unwrap(),
            "registry.json.lock"
        );
    }

    #[test]
    fn drop_removes_lock_file() {
        let (_dir, target) = setup();
        let lock_path;
        {
            let lock = FileLock::acquire(&target).unwrap();
            lock_path = lock.path().to_path_buf();
            assert!(lock_path.exists());
        }
        assert!(!lock_path.exists());
    }

    #[test]
    fn second_acquire_fails_when_locked() {
        let (_dir, target) = setup();
        let _lock = FileLock::acquire(&target).unwrap();

        // Manually create a fresh lock to simulate contention without
        // waiting for the retry delay on the real lock
        let other_target = _dir.path().join("other.json");
        fs::write(&other_target, "").unwrap();
        let lock_path = lock_path_for(&other_target);
        fs::write(&lock_path, "").unwrap();

        let result = FileLock::acquire(&other_target);
        assert!(result.is_err());
    }

    #[test]
    fn stale_lock_is_reclaimed() {
        let (_dir, target) = setup();
        let lock_path = lock_path_for(&target);

        // Create a lock file and backdate it past the stale threshold
        fs::write(&lock_path, "").unwrap();
        backdate_file(&lock_path, STALE_THRESHOLD + Duration::from_secs(10));

        assert!(is_stale(&lock_path));
        let lock = FileLock::acquire(&target).unwrap();
        assert!(lock.path().exists());
    }

    fn backdate_file(path: &Path, age: Duration) {
        let old_time = SystemTime::now() - age;
        let times = fs::FileTimes::new().set_modified(old_time);
        let file = fs::File::open(path).unwrap();
        file.set_times(times).unwrap();
    }

    #[test]
    fn acquire_on_nonexistent_parent_fails() {
        let target = PathBuf::from("/nonexistent/path/file.json");
        let result = FileLock::acquire(&target);
        assert!(result.is_err());
    }
}
