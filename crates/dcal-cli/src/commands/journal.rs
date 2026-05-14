use anyhow::{Context, Result};

use dcal_core::paths::DcalPaths;
use dcal_core::project_files;
use dcal_core::registry;

use crate::output::relative_time;
use crate::resolve::resolve_target;

pub fn run(target: String) -> Result<()> {
    let paths = DcalPaths::from_env();
    let entries = registry::load(&paths.registry())
        .with_context(|| "failed to load registry")?;

    let entry = resolve_target(&entries, &target)?;
    let meta = project_files::load_meta(&paths.project_meta(&entry.id))?;
    let journal = project_files::load_journal(&paths.project_journal(&entry.id))?;

    if journal.trim().is_empty() {
        println!("No journal entries for '{}'.", meta.name);
    } else {
        println!("{}", journal.trim());
    }

    let last_synced = relative_time(meta.last_active_at);
    eprintln!(
        "\nLast synced: {}. Run 'dcal info {}' or 'dcal sync' to refresh.",
        last_synced, meta.name,
    );

    Ok(())
}
