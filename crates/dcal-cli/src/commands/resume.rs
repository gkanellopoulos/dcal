use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use chrono::Utc;

use dcal_core::brief;
use dcal_core::paths::{expand_tilde, DcalPaths};
use dcal_core::project::{ProjectStatus, RegistryEntry};
use dcal_core::project_files;
use dcal_core::registry;

use crate::output::relative_time;
use crate::resolve::resolve_target;

pub fn run(target: String) -> Result<()> {
    let paths = DcalPaths::from_env();
    let entries = registry::load(&paths.registry())
        .with_context(|| "failed to load registry")?;

    let entry = resolve_target(&entries, &target)?;

    check_hook_health();

    // Load project data
    let meta = project_files::load_meta(&paths.project_meta(&entry.id))
        .with_context(|| format!("failed to load metadata for {}", entry.id))?;
    let snapshot = project_files::load_snapshot(&paths.project_snapshot(&entry.id))?;
    let sessions = project_files::load_sessions(&paths.project_sessions(&entry.id))?;

    let last_active = relative_time(meta.last_active_at);

    // Build and display the brief
    let reengagement = brief::build(&meta, &snapshot, &sessions, &last_active);
    let terminal_output = brief::format_terminal(&reengagement);
    println!("\n{terminal_output}\n");

    // Confirm launch
    let launch = dialoguer::Confirm::new()
        .with_prompt("Launch CC with this context?")
        .default(true)
        .interact()?;

    if !launch {
        println!("Cancelled.");
        return Ok(());
    }

    // Update status to Active if paused
    if meta.status == ProjectStatus::Paused {
        let mut updated_meta = meta.clone();
        updated_meta.status = ProjectStatus::Active;
        updated_meta.last_active_at = Utc::now();
        project_files::save_meta(&paths.project_meta(&entry.id), &updated_meta)?;

        let updated_entry = RegistryEntry::from(&updated_meta);
        registry::update(&paths.registry(), &updated_entry)?;
    }

    // Write brief to temp file
    let brief_text = brief::format_system_prompt(&reengagement);
    let brief_file = std::env::temp_dir().join(format!("dcal-brief-{}.md", entry.id));
    fs::write(&brief_file, &brief_text)
        .with_context(|| "failed to write brief temp file")?;

    // Launch CC
    let project_path = expand_tilde(&meta.path);
    let status = Command::new("claude")
        .arg("--append-system-prompt-file")
        .arg(&brief_file)
        .current_dir(&project_path)
        .status();

    // Clean up temp file
    let _ = fs::remove_file(&brief_file);

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => eprintln!("Claude Code exited with status: {s}"),
        Err(e) => eprintln!("Failed to launch Claude Code: {e}"),
    }

    Ok(())
}

fn check_hook_health() {
    let home = std::env::var("HOME").unwrap_or_default();
    let settings_path = PathBuf::from(&home).join(".claude").join("settings.json");

    match dcal_hooks::install::get_hook_binary_path(&settings_path) {
        None => {
            eprintln!("Warning: no dcal SessionEnd hook found.");
            eprintln!("  Session journaling is disabled. Run 'dcal init' to install it.\n");
        }
        Some(bin_path) => {
            if !PathBuf::from(&bin_path).exists() {
                eprintln!("Warning: dcal hook points to '{bin_path}' which no longer exists.");
                eprintln!("  Session journaling will not work. Run 'dcal init' to fix.\n");
            }
        }
    }
}
