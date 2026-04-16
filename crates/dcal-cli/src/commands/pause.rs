use anyhow::{Context, Result};
use chrono::Utc;

use dcal_core::lifecycle;
use dcal_core::paths::DcalPaths;
use dcal_core::project::{ProjectStatus, RegistryEntry};
use dcal_core::project_files;
use dcal_core::registry;

use crate::resolve::resolve_target;

pub fn run(target: String, note: Option<String>) -> Result<()> {
    let paths = DcalPaths::from_env();
    let entries = registry::load(&paths.registry())
        .with_context(|| "failed to load registry")?;

    let entry = resolve_target(&entries, &target)?;

    // Validate transition
    lifecycle::validate_transition(entry.status, ProjectStatus::Paused)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    // Load and update meta.json
    let meta_path = paths.project_meta(&entry.id);
    let mut meta = project_files::load_meta(&meta_path)
        .with_context(|| format!("failed to load project metadata for {}", entry.id))?;

    let now = Utc::now();
    meta.status = ProjectStatus::Paused;
    meta.last_active_at = now;

    project_files::save_meta(&meta_path, &meta)?;

    // Write journal entry
    let journal_path = paths.project_journal(&entry.id);
    let note_text = note.as_deref().unwrap_or("(no note)");
    let journal_entry = format!(
        "\n## Paused — {}\n\n{}\n",
        now.format("%Y-%m-%d %H:%M UTC"),
        note_text,
    );
    project_files::append_journal(&journal_path, &journal_entry)?;

    // Sync registry
    let updated_registry_entry = RegistryEntry::from(&meta);
    registry::update(&paths.registry(), &updated_registry_entry)?;

    println!("Paused '{}' [{}].", meta.name, meta.id);
    if let Some(ref n) = note {
        println!("Note: {n}");
    }

    Ok(())
}
