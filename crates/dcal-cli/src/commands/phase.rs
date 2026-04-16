use anyhow::{Context, Result};
use chrono::Utc;

use dcal_core::paths::DcalPaths;
use dcal_core::project::{ProjectPhase, RegistryEntry};
use dcal_core::project_files;
use dcal_core::registry;

use crate::resolve::resolve_target;

pub fn run(target: String, phase: String) -> Result<()> {
    let paths = DcalPaths::from_env();
    let entries = registry::load(&paths.registry())
        .with_context(|| "failed to load registry")?;

    let entry = resolve_target(&entries, &target)?;

    // Parse and validate the new phase
    let new_phase: ProjectPhase = phase
        .parse()
        .map_err(|e: String| anyhow::anyhow!("{e}"))?;

    // Load and update meta.json
    let meta_path = paths.project_meta(&entry.id);
    let mut meta = project_files::load_meta(&meta_path)
        .with_context(|| format!("failed to load project metadata for {}", entry.id))?;

    let old_phase = meta.phase;
    if old_phase == new_phase {
        println!("'{}' is already in phase '{}'.", meta.name, new_phase);
        return Ok(());
    }

    let now = Utc::now();
    meta.phase = new_phase;
    meta.last_active_at = now;

    project_files::save_meta(&meta_path, &meta)?;

    // Write journal entry
    let journal_path = paths.project_journal(&entry.id);
    let journal_entry = format!(
        "\n## Phase Change — {}\n\n**Phase:** {} → {}\n",
        now.format("%Y-%m-%d %H:%M UTC"),
        old_phase,
        new_phase,
    );
    project_files::append_journal(&journal_path, &journal_entry)?;

    // Sync registry
    let updated_registry_entry = RegistryEntry::from(&meta);
    registry::update(&paths.registry(), &updated_registry_entry)?;

    println!(
        "Updated '{}' [{}]: {} → {}",
        meta.name, meta.id, old_phase, new_phase
    );

    Ok(())
}
