use anyhow::{Context, Result};
use chrono::Utc;

use dcal_core::paths::DcalPaths;
use dcal_core::project::{ProjectPhase, ProjectStatus};
use dcal_core::registry;
use dcal_core::project_files;

use crate::output;

pub fn run(status: Option<String>, stale: Option<String>) -> Result<()> {
    let paths = DcalPaths::from_env();

    let mut entries = registry::load(&paths.registry())
        .with_context(|| "failed to load registry")?;

    // Filter by status
    if let Some(ref status_str) = status {
        let filter: ProjectStatus = status_str
            .parse()
            .map_err(|e: String| anyhow::anyhow!(e))?;
        entries.retain(|e| e.status == filter);
    }

    // Filter by stale threshold
    if let Some(ref stale_str) = stale {
        let days = output::parse_stale_days(stale_str)
            .ok_or_else(|| anyhow::anyhow!("invalid stale value: {stale_str} (expected e.g. 30d)"))?;
        let cutoff = Utc::now() - chrono::Duration::days(days);
        entries.retain(|e| e.last_active_at < cutoff);
    }

    // Sort by last_active_at descending
    entries.sort_by_key(|e| std::cmp::Reverse(e.last_active_at));

    // Load phases from each project's meta.json
    let phases: Vec<ProjectPhase> = entries
        .iter()
        .map(|entry| {
            let meta_path = paths.project_meta(&entry.id);
            project_files::load_meta(&meta_path)
                .map(|m| m.phase)
                .unwrap_or(ProjectPhase::Unknown)
        })
        .collect();

    output::render_table(&entries, &phases);
    Ok(())
}
