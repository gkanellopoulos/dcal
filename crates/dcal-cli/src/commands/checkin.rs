use std::io::Read as _;

use anyhow::{bail, Context, Result};

use dcal_core::logging;
use dcal_core::paths::DcalPaths;
use dcal_core::registry;
use dcal_hooks::checkin::{self, HookInput, SessionSummary};

use crate::resolve::resolve_target;

pub fn run(target: Option<String>, auto: bool, project_from_cwd: bool) -> Result<()> {
    if auto || project_from_cwd {
        run_hook_mode()
    } else if let Some(target) = target {
        run_manual_mode(&target)
    } else {
        bail!("usage: dcal checkin <name|id> or dcal checkin --auto --project-from-cwd")
    }
}

/// Hook mode: log errors to errors.log and propagate them.
///
/// SessionEnd hooks are non-blocking by design — CC shows the first
/// line of stderr to the user and continues exiting. Propagating the
/// error lets CC surface the failure instead of silencing it.
fn run_hook_mode() -> Result<()> {
    let paths = DcalPaths::from_env();

    if let Err(e) = run_hook_mode_inner(&paths) {
        logging::append_error_log(&paths.errors_log(), &e);
        return Err(e);
    }

    Ok(())
}

fn run_hook_mode_inner(paths: &DcalPaths) -> Result<()> {
    logging::debug("hook mode: reading stdin");

    let mut stdin_buf = String::new();
    std::io::stdin()
        .read_to_string(&mut stdin_buf)
        .with_context(|| "failed to read stdin")?;

    let input: HookInput = serde_json::from_str(&stdin_buf)
        .with_context(|| "failed to parse hook input from stdin")?;

    logging::debug(&format!("hook mode: cwd={}, session={}", input.cwd, input.session_id));

    let performed = checkin::auto_checkin(paths, &input)?;

    if performed {
        logging::debug("hook mode: checkin recorded");
    } else {
        logging::debug("hook mode: no matching project, skipped");
    }

    Ok(())
}

fn run_manual_mode(target: &str) -> Result<()> {
    let paths = DcalPaths::from_env();
    let entries = registry::load(&paths.registry())
        .with_context(|| "failed to load registry")?;

    let entry = resolve_target(&entries, target)?;

    println!("Recording checkin for '{}' [{}]\n", entry.name, entry.id);

    let summary_text: String = dialoguer::Input::new()
        .with_prompt("Summary (what was accomplished)")
        .interact_text()?;

    let next_task: String = dialoguer::Input::new()
        .with_prompt("Next task")
        .interact_text()?;

    let questions_text: String = dialoguer::Input::new()
        .with_prompt("Open questions (comma-separated, or empty)")
        .allow_empty(true)
        .interact_text()?;

    let open_questions: Vec<String> = questions_text
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let phase_options = &[
        "ideation",
        "design",
        "implementation",
        "testing",
        "maintenance",
    ];

    let meta = dcal_core::project_files::load_meta(&paths.project_meta(&entry.id))
        .with_context(|| "failed to load project metadata")?;

    let current_phase = meta.phase.to_string().to_lowercase();
    let default_idx = phase_options
        .iter()
        .position(|p| *p == current_phase)
        .unwrap_or(2);

    let phase_idx = dialoguer::Select::new()
        .with_prompt("Current phase")
        .items(phase_options)
        .default(default_idx)
        .interact()?;

    let summary = SessionSummary {
        summary: summary_text,
        next_task,
        open_questions,
        blockers: vec![],
        phase: phase_options[phase_idx].to_string(),
    };

    checkin::apply_checkin(&paths, &entry, None, &summary)?;

    println!("\nCheckin recorded for '{}'.", entry.name);

    Ok(())
}
