use std::fmt;
use std::process;

/// Print a user-facing error message and exit with code 1.
pub fn exit_with_error(err: &dyn fmt::Display) -> ! {
    eprintln!("dcal: error: {err}");
    process::exit(1);
}

/// Print a warning message to stderr.
#[allow(dead_code)]
pub fn warn(msg: &str) {
    eprintln!("dcal: warning: {msg}");
}
