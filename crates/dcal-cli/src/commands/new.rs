use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

use dcal_config::loader;
use dcal_core::paths::DcalPaths;
use dcal_generate::client::ReqwestClient;
use dcal_generate::scaffold::{self, ScaffoldParams};
use dcal_generate::{generate, intake, resolve, validate};

pub fn run(path: Option<PathBuf>) -> Result<()> {
    let paths = DcalPaths::from_env();

    if !paths.config().exists() {
        anyhow::bail!("dcal is not initialized. Run 'dcal init' first.");
    }

    let config = loader::load(&paths.config())
        .with_context(|| "failed to load config")?;

    // Get idea from user
    println!("Describe your project idea:\n");
    let idea: String = dialoguer::Input::new()
        .with_prompt("Idea")
        .interact_text()?;

    if idea.trim().is_empty() {
        anyhow::bail!("idea cannot be empty");
    }

    // Create API client
    let client = ReqwestClient::from_env()
        .context("ANTHROPIC_API_KEY is required for project creation")?;

    // Run the async pipeline
    let rt = tokio::runtime::Runtime::new()
        .context("failed to start async runtime")?;

    let (claude_md, brief) = rt.block_on(async {
        // Stage 1: Intake
        let spinner = indicatif::ProgressBar::new_spinner();
        spinner.set_message("Analyzing idea...");
        spinner.enable_steady_tick(std::time::Duration::from_millis(100));

        let brief = intake::run(&client, &idea).await
            .context("intake stage failed")?;

        spinner.set_message("Generating CLAUDE.md...");

        // Stage 2: Resolve
        let spec = resolve::run(&brief, &config);

        // Stage 3: Generate
        let claude_md = generate::run(&client, &spec).await
            .context("generation stage failed")?;

        spinner.finish_and_clear();
        Ok::<_, anyhow::Error>((claude_md, brief))
    })?;

    // Stage 4: Validate
    let validation = validate::run(&claude_md);
    if !validation.pass {
        println!("\nCLAUDE.md validation warnings:");
        for w in &validation.warnings {
            println!("  - {w}");
        }
        println!();
    }

    // Determine project path
    let project_path = if let Some(p) = path {
        p
    } else {
        std::env::current_dir()?.join(&brief.name)
    };

    let project_path_str = project_path.display().to_string();

    // Stage 5: Scaffold
    let result = scaffold::run(
        &paths,
        ScaffoldParams {
            name: brief.name.clone(),
            description: brief.goals.first().cloned().unwrap_or_default(),
            project_path: project_path_str.clone(),
            claude_md_content: claude_md,
            idea_text: idea,
            git_init: config.defaults.git_init,
        },
    )
    .context("scaffold stage failed")?;

    println!("\nProject '{}' created at {}", brief.name, project_path_str);
    println!("Registered as {} [{}]", brief.name, result.id);

    // Launch Claude Code if configured
    if config.defaults.open_after_create {
        println!("\nLaunching Claude Code...\n");
        let status = Command::new("claude")
            .current_dir(&project_path_str)
            .status();

        match status {
            Ok(s) if s.success() => {}
            Ok(s) => eprintln!("Claude Code exited with status: {s}"),
            Err(e) => eprintln!("Failed to launch Claude Code: {e}"),
        }
    }

    Ok(())
}
