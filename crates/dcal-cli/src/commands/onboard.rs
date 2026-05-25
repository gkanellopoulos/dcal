use std::path::PathBuf;

use anyhow::{Context, Result};

use dcal_core::paths::{collapse_to_tilde, DcalPaths};
use dcal_core::project::ProjectStatus;
use dcal_core::registry;
use dcal_generate::client::{AnthropicClient, ApiRequest, Message, ReqwestClient};
use dcal_generate::intake::extract_json;
use dcal_hooks::cc_projects;
use dcal_onboard::importer::{self, ImportParams};
use dcal_onboard::reader;

const DESCRIBE_SYSTEM: &str = r#"You are a project description writer. You will receive the contents of a Claude Code project's memory index file. From it, determine what the project is and write a single-sentence description.

Respond with JSON only, no other text.

If you can determine what the project is:
{"description": "one sentence describing what the project is and does"}

If the content is too vague, contains only personal preferences or feedback with no project identity, or you cannot confidently determine what the project is:
{"description": null}"#;

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

    // Compute CC memory path
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    let cc_home = PathBuf::from(&home).join(".claude");
    let cc_dir = cc_projects::cc_project_dir(&cc_home, &abs_path.to_string_lossy());
    let memory_path = cc_dir.join("memory").join("MEMORY.md");

    // Read project info
    let info = reader::read_project_info(&abs_path, Some(&memory_path))
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

    // Resolve description: MEMORY.md + Haiku > CLAUDE.md > manual
    let description = resolve_description(&info)?;

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

fn resolve_description(info: &reader::ProjectInfo) -> Result<String> {
    // Try MEMORY.md + Haiku first
    if let Some(ref memory_content) = info.memory_content {
        if let Ok(client) = ReqwestClient::from_env() {
            let rt = tokio::runtime::Runtime::new()
                .context("failed to start async runtime")?;

            match rt.block_on(describe_from_memory(&client, memory_content)) {
                Some(ai_desc) => {
                    println!("Description (from project memory): {ai_desc}");
                    let accept = dialoguer::Confirm::new()
                        .with_prompt("Accept this description?")
                        .default(true)
                        .interact()?;
                    if accept {
                        return Ok(ai_desc);
                    }
                    let custom: String = dialoguer::Input::new()
                        .with_prompt("Description")
                        .interact_text()?;
                    return Ok(custom);
                }
                None => {
                    eprintln!("Could not derive description from project memory.");
                }
            }
        }
    }

    // Fall back to CLAUDE.md
    if let Some(ref desc) = info.description {
        println!("Description (from CLAUDE.md): {desc}");
        let accept = dialoguer::Confirm::new()
            .with_prompt("Accept this description?")
            .default(true)
            .interact()?;
        if accept {
            return Ok(desc.clone());
        }
        let custom: String = dialoguer::Input::new()
            .with_prompt("Description")
            .interact_text()?;
        return Ok(custom);
    }

    // Manual entry
    let desc: String = dialoguer::Input::new()
        .with_prompt("Brief description")
        .allow_empty(true)
        .default(String::new())
        .interact_text()?;
    Ok(desc)
}

async fn describe_from_memory(client: &ReqwestClient, memory_content: &str) -> Option<String> {
    let request = ApiRequest {
        model: "claude-haiku-4-5".to_string(),
        system: Some(DESCRIBE_SYSTEM.to_string()),
        messages: vec![Message {
            role: "user".to_string(),
            content: memory_content.to_string(),
        }],
        max_tokens: 256,
    };

    let response = client.send(request).await.ok()?;

    let json_str = extract_json(&response.content);
    let value: serde_json::Value = serde_json::from_str(json_str).ok()?;
    value.get("description")?.as_str().map(String::from)
}
