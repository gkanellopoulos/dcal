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
    let sessions = project_files::load_sessions(&paths.project_sessions(&entry.id))?;

    if sessions.is_empty() {
        println!("No sessions for '{}'.", meta.name);
    } else {
        println!("Sessions for '{}' ({} total):\n", meta.name, sessions.len());
        for session in &sessions {
            let date = session.ended_at.format("%Y-%m-%d %H:%M UTC");
            let source = if session.session_id.is_some() {
                "auto"
            } else {
                "manual"
            };
            println!("  {} [{}] — {}", date, source, session.summary);
            println!("    Next: {}", session.next_task);
            if !session.open_questions.is_empty() {
                for q in &session.open_questions {
                    println!("    ? {q}");
                }
            }
            println!();
        }
    }

    let last_synced = relative_time(meta.last_active_at);
    eprintln!(
        "Last synced: {}. Run 'dcal info {}' or 'dcal sync' to refresh.",
        last_synced, meta.name,
    );

    Ok(())
}
