use std::path::PathBuf;

use anyhow::{Context, Result};

use dcal_core::paths::{DcalPaths, expand_tilde};
use dcal_core::project::RegistryEntry;
use dcal_core::project_files;
use dcal_core::registry;

use crate::output::{colorize_status, format_phase, relative_time};
use crate::resolve::resolve_target;

pub fn run(target: String) -> Result<()> {
    let paths = DcalPaths::from_env();
    let entries = registry::load(&paths.registry())
        .with_context(|| "failed to load registry")?;

    let entry = resolve_target(&entries, &target)?;

    run_sync(&entry, &paths);

    let meta = project_files::load_meta(&paths.project_meta(&entry.id))
        .with_context(|| format!("failed to load metadata for {}", entry.id))?;
    let snapshot = project_files::load_snapshot(&paths.project_snapshot(&entry.id))?;
    let journal = project_files::load_journal(&paths.project_journal(&entry.id))?;
    let sessions = project_files::load_sessions(&paths.project_sessions(&entry.id))?;

    let last_active = relative_time(meta.last_active_at);
    let status_str = colorize_status(meta.status);
    let phase_str = format_phase(meta.phase);

    // Header
    println!();
    let bar = "━".repeat(50);
    println!("  {bar}");
    println!("  {} [{}]", meta.name, meta.id);
    println!(
        "  Status: {}    Phase: {}    Last active: {}",
        status_str, phase_str, last_active,
    );
    println!("  {bar}");

    // Description
    println!("\n  {}\n", meta.description);

    // Snapshot
    let snapshot_text = snapshot.trim();
    if snapshot_text.is_empty() {
        println!("  SNAPSHOT");
        println!("  No session history yet.\n");
    } else {
        println!("  SNAPSHOT");
        for line in snapshot_text.lines() {
            println!("  {line}");
        }
        println!();
    }

    // Recent journal entries
    let recent = extract_recent_entries(&journal, 3);
    if !recent.is_empty() {
        println!("  RECENT JOURNAL");
        for entry_text in &recent {
            println!("  {entry_text}");
        }
        println!();
    }

    // Session count and path
    println!("  Sessions: {} total", sessions.len());

    let project_path = expand_tilde(&meta.path);
    println!("  Path: {}", project_path.display());
    println!("  {bar}\n");

    Ok(())
}

/// Extract the last N journal entries as one-line summaries.
///
/// Journal entries start with `## Session — ` headers. We grab the header
/// date and the first non-empty body line as a summary.
fn extract_recent_entries(journal: &str, count: usize) -> Vec<String> {
    let mut entries: Vec<String> = Vec::new();
    let mut current_date = String::new();
    let mut current_summary = String::new();

    for line in journal.lines() {
        if let Some(rest) = line.strip_prefix("## Session — ") {
            if !current_date.is_empty() {
                let summary = if current_summary.is_empty() {
                    "(no summary)".to_string()
                } else {
                    truncate(&current_summary, 72)
                };
                entries.push(format!("{current_date} — {summary}"));
            }
            current_date = rest.trim().to_string();
            current_summary.clear();
        } else if !current_date.is_empty() && current_summary.is_empty() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with("**") {
                current_summary = trimmed.to_string();
            }
        }
    }

    // Flush last entry
    if !current_date.is_empty() {
        let summary = if current_summary.is_empty() {
            "(no summary)".to_string()
        } else {
            truncate(&current_summary, 72)
        };
        entries.push(format!("{current_date} — {summary}"));
    }

    entries.into_iter().rev().take(count).collect()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}

fn run_sync(entry: &RegistryEntry, paths: &DcalPaths) {
    let home = std::env::var("HOME").unwrap_or_default();
    let cc_home = PathBuf::from(&home).join(".claude");

    let sync_model = dcal_config::loader::load(&paths.config())
        .map(|c| c.models.sync.clone())
        .unwrap_or_default();
    let summarizer = dcal_hooks::summarizer::ClaudeCliSummarizer::new(&sync_model);

    match dcal_hooks::sync::sync_unprocessed_sessions(entry, paths, &cc_home, &summarizer) {
        Ok(result) if result.synced > 0 => {
            eprintln!("Synced {} new session(s).\n", result.synced);
        }
        Ok(_) => {}
        Err(e) => {
            eprintln!("Warning: session sync failed: {e}");
            eprintln!("  Proceeding with existing data.\n");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_recent_from_journal() {
        let journal = "\
## Session — 2026-05-10 10:00 UTC

Set up project scaffold.

**Next:** Build the API client.

## Session — 2026-05-11 14:00 UTC

Implemented API client with retry logic.

**Next:** Add tests.

## Session — 2026-05-12 09:00 UTC

Added unit and integration tests.

**Next:** Deploy.
";

        let recent = extract_recent_entries(journal, 2);
        assert_eq!(recent.len(), 2);
        assert!(recent[0].contains("2026-05-12"));
        assert!(recent[0].contains("Added unit and integration tests."));
        assert!(recent[1].contains("2026-05-11"));
    }

    #[test]
    fn extract_recent_empty_journal() {
        let recent = extract_recent_entries("", 3);
        assert!(recent.is_empty());
    }

    #[test]
    fn extract_recent_more_requested_than_available() {
        let journal = "\
## Session — 2026-05-10 10:00 UTC

Only one session.

**Next:** More work.
";

        let recent = extract_recent_entries(journal, 5);
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn truncate_short_string() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_long_string() {
        let long = "a".repeat(100);
        let result = truncate(&long, 20);
        assert!(result.len() <= 20);
        assert!(result.ends_with("..."));
    }
}
