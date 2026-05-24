use std::path::Path;
use thiserror::Error;

use crate::loader;
use crate::model::{
    ClaudeMdConfig, Config, JournalConfig, ModelsConfig, PersonalConfig, Preferences,
    ProjectDefaults,
};

#[derive(Debug, Error)]
pub enum InitError {
    #[error("config wizard cancelled by user")]
    Cancelled,

    #[error("failed to save config: {0}")]
    Save(#[from] loader::ConfigError),

    #[error("interactive prompt failed: {0}")]
    Prompt(#[from] dialoguer::Error),
}

/// Run the interactive config wizard and save the result.
///
/// Prompts the user for personal info, preferences, and defaults.
/// Empty inputs fall back to default values.
pub fn run_wizard(config_path: &Path) -> Result<Config, InitError> {
    let personal = prompt_personal()?;
    let preferences = prompt_preferences()?;
    let defaults = prompt_defaults()?;
    let claude_md = prompt_claude_md()?;
    let journal = prompt_journal()?;

    let config = Config {
        version: "1.0".to_string(),
        personal,
        preferences,
        defaults,
        claude_md,
        journal,
        models: ModelsConfig::default(),
    };

    loader::save(config_path, &config)?;
    Ok(config)
}

fn prompt_personal() -> Result<PersonalConfig, InitError> {
    println!("\n── Personal Info ──\n");

    let name: String = dialoguer::Input::new()
        .with_prompt("Your name")
        .allow_empty(true)
        .default(String::new())
        .interact_text()?;

    let timezone: String = dialoguer::Input::new()
        .with_prompt("Timezone")
        .default("UTC".to_string())
        .interact_text()?;

    let github: String = dialoguer::Input::new()
        .with_prompt("GitHub username")
        .allow_empty(true)
        .default(String::new())
        .interact_text()?;

    Ok(PersonalConfig {
        name,
        timezone,
        github,
    })
}

fn prompt_preferences() -> Result<Preferences, InitError> {
    println!("\n── Development Preferences ──\n");

    let language_primary: String = dialoguer::Input::new()
        .with_prompt("Primary language (e.g. rust, python, typescript)")
        .allow_empty(true)
        .default(String::new())
        .interact_text()?;

    let language_secondary: String = dialoguer::Input::new()
        .with_prompt("Secondary language")
        .allow_empty(true)
        .default(String::new())
        .interact_text()?;

    let css_framework: String = dialoguer::Input::new()
        .with_prompt("CSS framework (e.g. tailwind, vanilla)")
        .allow_empty(true)
        .default(String::new())
        .interact_text()?;

    let testing_philosophy: String = dialoguer::Input::new()
        .with_prompt("Testing philosophy (e.g. TDD, integration-first)")
        .allow_empty(true)
        .default(String::new())
        .interact_text()?;

    let commit_style: String = dialoguer::Input::new()
        .with_prompt("Commit style")
        .default("conventional".to_string())
        .interact_text()?;

    let error_handling: String = dialoguer::Input::new()
        .with_prompt("Error handling approach")
        .allow_empty(true)
        .default(String::new())
        .interact_text()?;

    Ok(Preferences {
        language_primary,
        language_secondary,
        css_framework,
        testing_philosophy,
        commit_style,
        error_handling,
    })
}

fn prompt_defaults() -> Result<ProjectDefaults, InitError> {
    println!("\n── Project Defaults ──\n");

    let license: String = dialoguer::Input::new()
        .with_prompt("Default license")
        .default("MIT".to_string())
        .interact_text()?;

    let git_init = dialoguer::Confirm::new()
        .with_prompt("Run git init for new projects?")
        .default(true)
        .interact()?;

    let open_after_create = dialoguer::Confirm::new()
        .with_prompt("Launch Claude Code after project creation?")
        .default(true)
        .interact()?;

    Ok(ProjectDefaults {
        license,
        git_init,
        open_after_create,
    })
}

fn prompt_claude_md() -> Result<ClaudeMdConfig, InitError> {
    println!("\n── CLAUDE.md Generation ──\n");
    println!("Enter personal context to inject into every generated CLAUDE.md.");
    println!("This can include working style, constraints, or preferences.");
    println!("(Leave empty to skip, you can edit config.yml later)\n");

    let personal_context: String = dialoguer::Input::new()
        .with_prompt("Personal context")
        .allow_empty(true)
        .default(String::new())
        .interact_text()?;

    Ok(ClaudeMdConfig { personal_context })
}

fn prompt_journal() -> Result<JournalConfig, InitError> {
    println!("\n── Journal Settings ──\n");

    let auto_checkin = dialoguer::Confirm::new()
        .with_prompt("Enable automatic session check-ins?")
        .default(true)
        .interact()?;

    let prompt_for_human_note = dialoguer::Confirm::new()
        .with_prompt("Prompt for a note after each session?")
        .default(true)
        .interact()?;

    Ok(JournalConfig {
        auto_checkin,
        prompt_for_human_note,
    })
}
