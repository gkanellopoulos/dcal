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

pub fn run(target: String, cc_model: Option<String>) -> Result<()> {
    let paths = DcalPaths::from_env();
    let entries = registry::load(&paths.registry())
        .with_context(|| "failed to load registry")?;

    let entry = resolve_target(&entries, &target)?;

    // Sync unprocessed CC sessions before loading project data
    run_sync(&entry, &paths);

    // Load project data (after sync so it reflects latest state)
    let meta = project_files::load_meta(&paths.project_meta(&entry.id))
        .with_context(|| format!("failed to load metadata for {}", entry.id))?;
    let snapshot = project_files::load_snapshot(&paths.project_snapshot(&entry.id))?;
    let sessions = project_files::load_sessions(&paths.project_sessions(&entry.id))?;

    let last_active = relative_time(meta.last_active_at);

    // Build and display the brief
    let reengagement = brief::build(&meta, &snapshot, &sessions, &last_active);
    let terminal_output = brief::format_terminal(&reengagement);
    println!("\n{terminal_output}\n");

    // Resolve effective CC model: --model flag overrides and persists
    let effective_model = cc_model.unwrap_or_default();
    let model_to_use = if effective_model.is_empty() {
        meta.cc_model.clone()
    } else {
        effective_model
    };

    // Reactivate project and persist model override
    {
        let mut updated_meta = meta.clone();
        if meta.status == ProjectStatus::Paused {
            updated_meta.status = ProjectStatus::Active;
            updated_meta.last_active_at = Utc::now();
        }
        if !model_to_use.is_empty() {
            updated_meta.cc_model = model_to_use.clone();
        }
        if updated_meta != meta {
            project_files::save_meta(&paths.project_meta(&entry.id), &updated_meta)?;
            let updated_entry = RegistryEntry::from(&updated_meta);
            registry::update(&paths.registry(), &updated_entry)?;
        }
    }

    // Confirm launch
    let launch = dialoguer::Confirm::new()
        .with_prompt("Launch CC with this context?")
        .default(true)
        .interact()?;

    if !launch {
        println!("Cancelled.");
        return Ok(());
    }

    // Write brief to temp file
    let brief_text = brief::format_system_prompt(&reengagement);
    let brief_file = std::env::temp_dir().join(format!("dcal-brief-{}.md", entry.id));
    fs::write(&brief_file, &brief_text)
        .with_context(|| "failed to write brief temp file")?;

    // Launch CC
    let project_path = expand_tilde(&meta.path);
    let mut cmd = Command::new("claude");
    cmd.arg("--append-system-prompt-file")
        .arg(&brief_file)
        .current_dir(&project_path)
        .env_remove("ANTHROPIC_API_KEY");
    if !model_to_use.is_empty() {
        cmd.args(["--model", &model_to_use]);
    }
    let status = cmd.status();

    // Clean up temp file
    let _ = fs::remove_file(&brief_file);

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => eprintln!("Claude Code exited with status: {s}"),
        Err(e) => eprintln!("Failed to launch Claude Code: {e}"),
    }

    Ok(())
}

fn run_sync(entry: &RegistryEntry, paths: &DcalPaths) {
    let home = std::env::var("HOME").unwrap_or_default();
    let cc_home = PathBuf::from(&home).join(".claude");

    let sync_model = dcal_config::loader::load(&paths.config())
        .map(|c| c.models.sync.clone())
        .unwrap_or_default();
    let summarizer = dcal_hooks::summarizer::ClaudeCliSummarizer::new(&sync_model);

    match dcal_hooks::sync::sync_unprocessed_sessions(entry, paths, &cc_home, &summarizer) {
        Ok(result) if result.synced > 0 || result.updated > 0 => {
            let mut parts = Vec::new();
            if result.synced > 0 {
                parts.push(format!("synced {}", result.synced));
            }
            if result.updated > 0 {
                parts.push(format!("updated {}", result.updated));
            }
            eprintln!("{} session(s).\n", parts.join(", "));
        }
        Ok(_) => {}
        Err(e) => {
            eprintln!("Warning: session sync failed: {e}");
            eprintln!("  Proceeding with existing data.\n");
        }
    }
}
