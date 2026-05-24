use std::path::PathBuf;

use anyhow::{Context, Result};

use dcal_core::paths::{collapse_to_tilde, DcalPaths};
use dcal_core::project::ProjectStatus;
use dcal_core::registry;
use dcal_onboard::importer::{self, ImportParams};
use dcal_onboard::reader;

pub fn run(path: PathBuf) -> Result<()> {
    let paths = DcalPaths::from_env();

    // Resolve to absolute path
    let abs_path = std::fs::canonicalize(&path)
        .with_context(|| format!("path not found: {}", path.display()))?;

    if !abs_path.is_dir() {
        anyhow::bail!("{} is not a directory", abs_path.display());
    }

    let collapsed = collapse_to_tilde(&abs_path);

    // Check for duplicates
    let entries = registry::load(&paths.registry())
        .with_context(|| "failed to load registry")?;

    if reader::is_duplicate(&entries, &collapsed) {
        anyhow::bail!("'{}' is already registered", collapsed);
    }

    // Read project info
    let info = reader::read_project_info(&abs_path)
        .with_context(|| "failed to read project info")?;

    // Prompt for project name
    let dir_name = abs_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let name: String = dialoguer::Input::new()
        .with_prompt("Project name")
        .default(dir_name)
        .interact_text()?;

    // Prompt for status
    let status_idx = dialoguer::Select::new()
        .with_prompt("Current status")
        .items(&["active", "paused"])
        .default(0)
        .interact()?;
    let status = if status_idx == 0 {
        ProjectStatus::Active
    } else {
        ProjectStatus::Paused
    };

    // Prompt for description if none found
    let description = if let Some(ref desc) = info.description {
        println!("Description (from CLAUDE.md): {desc}");
        desc.clone()
    } else {
        dialoguer::Input::new()
            .with_prompt("Brief description")
            .allow_empty(true)
            .default(String::new())
            .interact_text()?
    };

    // Prompt for CC model
    let cc_model: String = dialoguer::Input::new()
        .with_prompt("Preferred CC model (e.g. opus, sonnet, or empty for default)")
        .allow_empty(true)
        .default(String::new())
        .interact_text()?;

    // Import
    let result = importer::import(
        &paths,
        ImportParams {
            name,
            description,
            path: collapsed.clone(),
            status,
            last_active_at: info.last_commit_date,
            cc_model,
        },
    )?;

    println!(
        "\nRegistered as {} [{}].",
        result.name, result.id
    );
    println!("Run 'dcal resume {}' to reengage.", result.name);

    Ok(())
}
