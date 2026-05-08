use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use chrono::Utc;

const ENV_DCAL_LOG: &str = "DCAL_LOG";

/// Append a timestamped error entry to the errors.log file.
///
/// Silently ignores write failures — this function must never itself
/// cause a visible error.
pub fn append_error_log(errors_log_path: &Path, error: &dyn std::fmt::Display) {
    let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    let line = format!("[{timestamp}] {error}\n");

    let result = OpenOptions::new()
        .create(true)
        .append(true)
        .open(errors_log_path)
        .and_then(|mut f| f.write_all(line.as_bytes()));

    if let Err(e) = result {
        eprintln!("dcal: could not write to errors.log: {e}");
    }
}

/// Print a debug message to stderr if DCAL_LOG=debug is set.
pub fn debug(msg: &str) {
    if env::var(ENV_DCAL_LOG).as_deref() == Ok("debug") {
        eprintln!("[dcal debug] {msg}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn append_error_log_creates_file() {
        let dir = TempDir::new().unwrap();
        let log_path = dir.path().join("errors.log");

        append_error_log(&log_path, &"something broke");

        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("something broke"));
        assert!(content.contains("UTC"));
    }

    #[test]
    fn append_error_log_appends_multiple() {
        let dir = TempDir::new().unwrap();
        let log_path = dir.path().join("errors.log");

        append_error_log(&log_path, &"first error");
        append_error_log(&log_path, &"second error");

        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("first error"));
        assert!(content.contains("second error"));
        assert_eq!(content.lines().count(), 2);
    }

    #[test]
    fn append_error_log_bad_path_does_not_panic() {
        let bad_path = Path::new("/nonexistent/dir/errors.log");
        append_error_log(bad_path, &"should not panic");
    }

    #[test]
    fn debug_does_not_panic_when_unset() {
        env::remove_var("DCAL_LOG");
        debug("this should be silent");
    }

    #[test]
    fn debug_prints_when_enabled() {
        env::set_var("DCAL_LOG", "debug");
        debug("visible message");
        env::remove_var("DCAL_LOG");
    }
}
