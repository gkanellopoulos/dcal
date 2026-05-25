use std::fs;
use std::process::Command;

use anyhow::{Context, Result};

use dcal_core::paths::DcalPaths;

/// Run `dcal init`: create directory structure and config wizard.
pub fn run() -> Result<()> {
    let paths = DcalPaths::from_env();

    if paths.config().exists() {
        anyhow::bail!(
            "dcal is already initialized at {}",
            paths.root().display()
        );
    }

    println!("Initializing dcal at {}\n", paths.root().display());

    // Create directory structure
    fs::create_dir_all(paths.projects_dir())
        .with_context(|| format!("failed to create {}", paths.projects_dir().display()))?;

    // Initialize empty registry
    if !paths.registry().exists() {
        fs::write(paths.registry(), "[]")
            .with_context(|| format!("failed to create {}", paths.registry().display()))?;
    }

    // Run interactive config wizard
    dcal_config::init::run_wizard(&paths.config(), &paths.credentials())?;

    // Check for claude on PATH
    if !is_claude_on_path() {
        eprintln!("\nWarning: 'claude' not found on PATH.");
        eprintln!("Install Claude Code before using dcal new or dcal resume.");
    }

    println!("\ndcal initialized successfully.");
    Ok(())
}

fn is_claude_on_path() -> bool {
    let cmd = if cfg!(windows) { "where" } else { "which" };
    Command::new(cmd)
        .arg("claude")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
