use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

use dcal_core::paths::DcalPaths;

/// Run `dcal init`: create directory structure, config wizard, hook install.
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
    dcal_config::init::run_wizard(&paths.config())?;

    // Install SessionEnd hook
    let settings_path = claude_settings_path();
    match dcal_hooks::install::install_session_end_hook(&settings_path) {
        Ok(true) => println!("\nSessionEnd hook installed."),
        Ok(false) => println!("\nSessionEnd hook already present."),
        Err(e) => {
            eprintln!("\nWarning: failed to install SessionEnd hook: {e}");
            eprintln!("You can install it manually later or re-run dcal init.");
        }
    }

    // Check for claude on PATH
    if !is_claude_on_path() {
        eprintln!("\nWarning: 'claude' not found on PATH.");
        eprintln!("Install Claude Code before using dcal new or dcal resume.");
    }

    println!("\ndcal initialized successfully.");
    Ok(())
}

fn claude_settings_path() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME not set");
    PathBuf::from(home)
        .join(".claude")
        .join("settings.json")
}

fn is_claude_on_path() -> bool {
    Command::new("which")
        .arg("claude")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
