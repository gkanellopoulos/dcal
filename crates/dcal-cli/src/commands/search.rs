use anyhow::{Context, Result};

use dcal_core::paths::DcalPaths;
use dcal_core::project::ProjectPhase;
use dcal_core::project_files;
use dcal_core::registry;

use crate::output;

pub fn run(query: String) -> Result<()> {
    let paths = DcalPaths::from_env();
    let entries = registry::load(&paths.registry())
        .with_context(|| "failed to load registry")?;

    let query_lower = query.to_lowercase();
    let matches: Vec<_> = entries
        .into_iter()
        .filter(|e| e.name.to_lowercase().contains(&query_lower))
        .collect();

    if matches.is_empty() {
        println!("  No projects matching '{query}'.");
        return Ok(());
    }

    let phases: Vec<ProjectPhase> = matches
        .iter()
        .map(|entry| {
            let meta_path = paths.project_meta(&entry.id);
            project_files::load_meta(&meta_path)
                .map(|m| m.phase)
                .unwrap_or(ProjectPhase::Unknown)
        })
        .collect();

    output::render_table(&matches, &phases);
    Ok(())
}
