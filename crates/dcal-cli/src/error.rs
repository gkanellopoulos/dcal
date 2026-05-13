use std::process;

/// Print a user-facing error message and exit with code 1.
///
/// Prints the full error chain so the user sees the root cause,
/// not just the outermost context.
pub fn exit_with_error(err: &anyhow::Error) -> ! {
    eprintln!("dcal: error: {err}");
    for cause in err.chain().skip(1) {
        eprintln!("  caused by: {cause}");
    }
    process::exit(1);
}

/// Print a warning message to stderr.
#[allow(dead_code)]
pub fn warn(msg: &str) {
    eprintln!("dcal: warning: {msg}");
}
