use std::path::PathBuf;

use anyhow::{Context, Result};

use dcal_core::paths::DcalPaths;
use dcal_core::project::RegistryEntry;
use dcal_core::registry;

use crate::resolve::resolve_target;

pub fn run(target: Option<String>) -> Result<()> {
    let paths = DcalPaths::from_env();
    let entries = registry::load(&paths.registry())
        .with_context(|| "failed to load registry")?;

    let home = std::env::var("HOME").unwrap_or_default();
    let cc_home = PathBuf::from(&home).join(".claude");
    let sync_model = dcal_config::loader::load(&paths.config())
        .map(|c| c.models.sync.clone())
        .unwrap_or_default();
    let summarizer = dcal_hooks::summarizer::ClaudeCliSummarizer::new(&sync_model);

    match target {
        Some(t) => {
            let entry = resolve_target(&entries, &t)?;
            sync_one(&entry, &paths, &cc_home, &summarizer)?;
        }
        None => {
            if entries.is_empty() {
                println!("No projects to sync.");
                return Ok(());
            }
            for entry in &entries {
                sync_one(entry, &paths, &cc_home, &summarizer)?;
            }
        }
    }

    Ok(())
}

fn sync_one(
    entry: &RegistryEntry,
    paths: &DcalPaths,
    cc_home: &std::path::Path,
    summarizer: &dyn dcal_hooks::summarizer::Summarizer,
) -> Result<()> {
    match dcal_hooks::sync::sync_unprocessed_sessions(entry, paths, cc_home, summarizer) {
        Ok(result) if result.synced > 0 && result.skipped > 0 => {
            println!(
                "'{}': synced {} session(s), {} skipped (errors).",
                entry.name, result.synced, result.skipped
            );
        }
        Ok(result) if result.synced > 0 => {
            println!(
                "'{}': synced {} session(s).",
                entry.name, result.synced
            );
        }
        Ok(result) if result.skipped > 0 => {
            println!(
                "'{}': {} session(s) skipped (errors).",
                entry.name, result.skipped
            );
        }
        Ok(_) => {
            println!("'{}': up to date.", entry.name);
        }
        Err(e) => {
            eprintln!("Warning: sync failed for '{}': {e}", entry.name);
        }
    }
    Ok(())
}
