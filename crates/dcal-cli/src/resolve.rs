use anyhow::{bail, Result};

use dcal_core::project::RegistryEntry;
use dcal_core::registry;

/// Resolve a user-provided target (name or ID) to a single registry entry.
///
/// Tries ID match first, then case-insensitive name match. If multiple
/// projects share the same name, lists them and asks the user to use
/// the project ID instead.
pub fn resolve_target(entries: &[RegistryEntry], target: &str) -> Result<RegistryEntry> {
    // Try ID match first
    if let Some(entry) = registry::find_by_id(entries, target) {
        return Ok(entry.clone());
    }

    // Try name match
    let matches = registry::find_by_name(entries, target);

    match matches.len() {
        0 => bail!("no project found matching '{target}'"),
        1 => Ok(matches[0].clone()),
        _ => {
            eprintln!("Multiple projects match '{target}':\n");
            for entry in &matches {
                eprintln!("  {}  {}", entry.id, entry.name);
            }
            eprintln!();
            bail!("use the project ID to specify which one")
        }
    }
}
