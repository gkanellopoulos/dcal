use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

use dcal_config::loader;
use dcal_core::paths::DcalPaths;
use dcal_generate::client::ReqwestClient;
use dcal_generate::scaffold::{self, ScaffoldParams};
use dcal_generate::{generate, intake, resolve, validate};

pub fn run(name: Option<String>, path: Option<PathBuf>, cc_model: Option<String>) -> Result<()> {
    let paths = DcalPaths::from_env();

    if !paths.config().exists() {
        eprintln!("Note: no config found. Using defaults. Run 'dcal init' to personalize.\n");
    }

    // Resolve project path early, before any API calls
    let cwd = std::env::current_dir().context("failed to get current directory")?;
    let explicit_path = match (&path, &name) {
        (Some(p), _) => {
            let resolved = if p.is_absolute() { p.clone() } else { cwd.join(p) };
            Some(resolved)
        }
        (None, Some(n)) => Some(cwd.join(n)),
        (None, None) => None,
    };

    if let Some(ref p) = explicit_path {
        let parent = p.parent().unwrap_or(p);
        if !parent.exists() {
            anyhow::bail!(
                "parent directory '{}' does not exist. Create it first.",
                parent.display()
            );
        }
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

    // Load API key from credentials file (falls back to env)
    let api_key = dcal_config::credentials::load_api_key(&paths.credentials())
        .ok()
        .flatten()
        .context("API key not configured. Run 'dcal init' to set it up.")?;
    let client = ReqwestClient::new(api_key);

    // Run the async pipeline
    let rt = tokio::runtime::Runtime::new()
        .context("failed to start async runtime")?;

    let (claude_md, brief) = rt.block_on(async {
        // Stage 1: Intake
        let spinner = indicatif::ProgressBar::new_spinner();
        spinner.set_message("Analyzing idea...");
        spinner.enable_steady_tick(std::time::Duration::from_millis(100));

        let brief = intake::run(&client, &idea, &config.models.intake).await
            .context("intake stage failed")?;

        spinner.set_message("Generating CLAUDE.md...");

        // Stage 2: Resolve
        let spec = resolve::run(&brief, &config);

        // Stage 3: Generate
        let claude_md = generate::run(&client, &spec, &config.models.generate).await
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

    // Determine project name and path
    let project_name = name.unwrap_or_else(|| brief.name.clone());
    let project_path = explicit_path.unwrap_or_else(|| cwd.join(&brief.name));
    let project_path_str = project_path.display().to_string();

    // Stage 5: Scaffold
    let model = cc_model.unwrap_or_default();
    let result = scaffold::run(
        &paths,
        ScaffoldParams {
            name: project_name.clone(),
            description: brief.goals.first().cloned().unwrap_or_default(),
            project_path: project_path_str.clone(),
            claude_md_content: claude_md,
            idea_text: idea,
            git_init: config.defaults.git_init,
            cc_model: model.clone(),
        },
    )
    .context("scaffold stage failed")?;

    println!("\nProject '{}' created at {}", project_name, project_path_str);
    println!("Registered as {} [{}]", project_name, result.id);

    // Launch Claude Code if configured
    if config.defaults.open_after_create {
        println!("\nLaunching Claude Code...\n");
        let mut cmd = Command::new("claude");
        cmd.current_dir(&project_path_str)
            .env_remove("ANTHROPIC_API_KEY");
        if !model.is_empty() {
            cmd.args(["--model", &model]);
        }
        let status = cmd.status();

        match status {
            Ok(s) if s.success() => {}
            Ok(s) => eprintln!("Claude Code exited with status: {s}"),
            Err(e) => eprintln!("Failed to launch Claude Code: {e}"),
        }
    }

    Ok(())
}
