use std::path::Path;

use thiserror::Error;

use dcal_core::paths::DcalPaths;
use dcal_core::project::RegistryEntry;
use dcal_core::project_files;

use crate::cc_projects;
use crate::summarizer::Summarizer;
use crate::transcript;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("failed to load sessions: {0}")]
    Sessions(#[from] project_files::ProjectFileError),

    #[error("failed to read transcript: {0}")]
    Transcript(#[from] transcript::TranscriptError),

    #[error("failed to generate summary: {0}")]
    Checkin(#[from] crate::checkin::CheckinError),

    #[error("failed to update registry: {0}")]
    Registry(#[from] dcal_core::registry::RegistryError),
}

/// Result of a sync operation.
#[derive(Debug)]
pub struct SyncResult {
    pub synced: usize,
    pub updated: usize,
    pub skipped: usize,
}

/// Sync unprocessed and updated CC sessions for a project.
///
/// Finds JSONL transcripts in the CC project directory that aren't already
/// in sessions.json, summarizes each, and writes journal/snapshot/session entries.
/// Also detects resumed sessions whose transcript has been modified since the
/// last sync and re-processes them.
pub fn sync_unprocessed_sessions(
    entry: &RegistryEntry,
    paths: &DcalPaths,
    cc_home: &Path,
    summarizer: &dyn Summarizer,
) -> Result<SyncResult, SyncError> {
    let cc_dir = cc_projects::cc_project_dir(cc_home, &entry.path);

    if !cc_dir.exists() {
        return Ok(SyncResult { synced: 0, updated: 0, skipped: 0 });
    }

    let sessions = project_files::load_sessions(&paths.project_sessions(&entry.id))?;
    let known_ids = cc_projects::known_cc_session_ids(&sessions);
    let mut unprocessed = cc_projects::find_unprocessed(&cc_dir, &known_ids);
    let updated = cc_projects::find_updated(&cc_dir, &sessions);

    if unprocessed.is_empty() && updated.is_empty() {
        return Ok(SyncResult { synced: 0, updated: 0, skipped: 0 });
    }

    // Sort new sessions by transcript timestamp so the snapshot reflects the latest
    unprocessed.sort_by_key(|(_, path)| {
        transcript::last_timestamp(path).unwrap_or_default()
    });

    let total_new = unprocessed.len();
    let mut synced = 0;
    let mut skipped = 0;

    for (i, (session_id, transcript_path)) in unprocessed.iter().enumerate() {
        eprintln!(
            "  Syncing session {} of {}...",
            i + 1,
            total_new
        );

        let transcript_content = match transcript::read_transcript(transcript_path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("  warning: skipping session '{session_id}': {e}");
                skipped += 1;
                continue;
            }
        };

        if transcript_content.trim().is_empty() {
            eprintln!("  warning: skipping session '{session_id}': empty transcript");
            skipped += 1;
            continue;
        }

        let summary = match summarizer.summarize(&transcript_content) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("  warning: skipping session '{session_id}': {e}");
                skipped += 1;
                continue;
            }
        };

        let ended_at = transcript::last_timestamp(transcript_path);
        crate::checkin::apply_checkin(paths, entry, Some(session_id), &summary, ended_at)?;
        synced += 1;
    }

    // Re-process resumed sessions whose transcript was modified
    let mut updated_count = 0;

    for (session_id, transcript_path) in &updated {
        eprintln!("  Updating resumed session '{session_id}'...");

        let transcript_content = match transcript::read_transcript(transcript_path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("  warning: skipping update for '{session_id}': {e}");
                skipped += 1;
                continue;
            }
        };

        if transcript_content.trim().is_empty() {
            skipped += 1;
            continue;
        }

        let summary = match summarizer.summarize(&transcript_content) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("  warning: skipping update for '{session_id}': {e}");
                skipped += 1;
                continue;
            }
        };

        let ended_at = transcript::last_timestamp(transcript_path);
        update_checkin(paths, entry, session_id, &summary, ended_at)?;
        updated_count += 1;
    }

    Ok(SyncResult { synced, updated: updated_count, skipped })
}

/// Apply an updated checkin for a resumed session.
///
/// Replaces the existing session entry and appends an update note to the journal.
fn update_checkin(
    paths: &DcalPaths,
    entry: &RegistryEntry,
    cc_session_id: &str,
    summary: &crate::checkin::SessionSummary,
    ended_at: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<(), SyncError> {
    use dcal_core::project::{ProjectPhase, SessionEntry};

    let now = ended_at.unwrap_or_else(chrono::Utc::now);
    let meta_path = paths.project_meta(&entry.id);
    let mut meta = project_files::load_meta(&meta_path)?;

    let new_phase: Option<ProjectPhase> = summary.phase.parse().ok();
    if let Some(phase) = new_phase {
        if phase != meta.phase {
            meta.phase = phase;
        }
    }

    let new_entry = SessionEntry {
        id: dcal_core::id::generate_session_id(),
        session_id: Some(cc_session_id.to_string()),
        ended_at: now,
        summary: summary.summary.clone(),
        next_task: summary.next_task.clone(),
        open_questions: summary.open_questions.clone(),
        human_note: None,
    };

    let sessions_path = paths.project_sessions(&entry.id);
    let replaced = project_files::replace_session(&sessions_path, cc_session_id, &new_entry)?;
    if !replaced {
        project_files::append_session(&sessions_path, &new_entry)?;
    }

    let mut journal_text = format!(
        "\n## Session (resumed) — {}\n\n{}\n\n**Next:** {}\n",
        now.format("%Y-%m-%d %H:%M UTC"),
        summary.summary,
        summary.next_task,
    );
    if !summary.open_questions.is_empty() {
        journal_text.push_str("**Open questions:**\n");
        for q in &summary.open_questions {
            journal_text.push_str(&format!("- {q}\n"));
        }
    }
    project_files::append_journal(&paths.project_journal(&entry.id), &journal_text)?;

    let snapshot = format!("{}\n\nNext: {}\n", summary.summary, summary.next_task);
    project_files::save_snapshot(&paths.project_snapshot(&entry.id), &snapshot)?;

    meta.last_active_at = now;
    project_files::save_meta(&meta_path, &meta)?;

    let updated_entry = dcal_core::project::RegistryEntry::from(&meta);
    dcal_core::registry::update(&paths.registry(), &updated_entry)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::summarizer::MockSummarizer;
    use dcal_core::project::{ProjectMeta, ProjectPhase, ProjectStatus};
    use chrono::Utc;
    use tempfile::TempDir;

    fn setup() -> (TempDir, DcalPaths) {
        let dir = TempDir::new().unwrap();
        let paths = DcalPaths::new(dir.path().to_path_buf());
        std::fs::create_dir_all(paths.projects_dir()).unwrap();
        std::fs::write(paths.registry(), "[]").unwrap();
        (dir, paths)
    }

    fn create_project(paths: &DcalPaths) -> RegistryEntry {
        let now = Utc::now();
        let meta = ProjectMeta {
            id: "proj_test01".to_string(),
            name: "test-project".to_string(),
            description: "A test project".to_string(),
            path: "/tmp/test-project".to_string(),
            status: ProjectStatus::Active,
            phase: ProjectPhase::Implementation,
            created_at: now,
            last_active_at: now,
            blocked_reason: None,
            tags: vec![],
            priority: "medium".to_string(),
            cc_session_ids: vec![],
            cc_model: String::new(),
        };

        project_files::create_project_dir(
            &paths.project_dir(&meta.id),
            &meta,
            "test idea",
        )
        .unwrap();

        let entry = RegistryEntry::from(&meta);
        dcal_core::registry::add(&paths.registry(), entry.clone()).unwrap();
        entry
    }

    fn create_cc_dir(cc_home: &Path, project_path: &str) -> std::path::PathBuf {
        let cc_dir = cc_projects::cc_project_dir(cc_home, project_path);
        std::fs::create_dir_all(&cc_dir).unwrap();
        cc_dir
    }

    fn write_transcript(cc_dir: &Path, session_id: &str) {
        let content = format!(
            r#"{{"type": "user", "message": {{"role": "user", "content": "Build a feature"}}, "uuid": "a"}}{nl}{{"type": "assistant", "message": {{"role": "assistant", "content": [{{"type": "text", "text": "Done."}}]}}, "uuid": "b"}}"#,
            nl = "\n"
        );
        std::fs::write(cc_dir.join(format!("{session_id}.jsonl")), content).unwrap();
    }

    #[test]
    fn sync_no_cc_dir() {
        let (_dir, paths) = setup();
        let entry = create_project(&paths);
        let cc_home = TempDir::new().unwrap();
        let mock = MockSummarizer::new();

        let result = sync_unprocessed_sessions(
            &entry, &paths, cc_home.path(), &mock,
        ).unwrap();

        assert_eq!(result.synced, 0);
        assert_eq!(result.skipped, 0);
    }

    #[test]
    fn sync_no_new_sessions() {
        let (_dir, paths) = setup();
        let entry = create_project(&paths);
        let cc_home = TempDir::new().unwrap();
        create_cc_dir(cc_home.path(), &entry.path);
        let mock = MockSummarizer::new();

        let result = sync_unprocessed_sessions(
            &entry, &paths, cc_home.path(), &mock,
        ).unwrap();

        assert_eq!(result.synced, 0);
    }

    #[test]
    fn sync_processes_new_sessions() {
        let (_dir, paths) = setup();
        let entry = create_project(&paths);
        let cc_home = TempDir::new().unwrap();
        let cc_dir = create_cc_dir(cc_home.path(), &entry.path);
        write_transcript(&cc_dir, "session-aaa");
        write_transcript(&cc_dir, "session-bbb");

        let mock = MockSummarizer::new();
        let result = sync_unprocessed_sessions(
            &entry, &paths, cc_home.path(), &mock,
        ).unwrap();

        assert_eq!(result.synced, 2);
        assert_eq!(result.skipped, 0);

        let sessions = project_files::load_sessions(
            &paths.project_sessions(&entry.id),
        ).unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn sync_skips_already_processed() {
        let (_dir, paths) = setup();
        let entry = create_project(&paths);
        let cc_home = TempDir::new().unwrap();
        let cc_dir = create_cc_dir(cc_home.path(), &entry.path);
        write_transcript(&cc_dir, "session-aaa");
        write_transcript(&cc_dir, "session-bbb");

        let mock = MockSummarizer::new();

        // First sync
        sync_unprocessed_sessions(&entry, &paths, cc_home.path(), &mock).unwrap();

        // Add one more transcript
        write_transcript(&cc_dir, "session-ccc");

        // Second sync
        let result = sync_unprocessed_sessions(
            &entry, &paths, cc_home.path(), &mock,
        ).unwrap();

        assert_eq!(result.synced, 1);
        assert_eq!(result.skipped, 0);

        let sessions = project_files::load_sessions(
            &paths.project_sessions(&entry.id),
        ).unwrap();
        assert_eq!(sessions.len(), 3);
    }

    #[test]
    fn sync_skips_empty_transcripts() {
        let (_dir, paths) = setup();
        let entry = create_project(&paths);
        let cc_home = TempDir::new().unwrap();
        let cc_dir = create_cc_dir(cc_home.path(), &entry.path);

        // Write an empty transcript
        std::fs::write(cc_dir.join("session-empty.jsonl"), "").unwrap();
        write_transcript(&cc_dir, "session-good");

        let mock = MockSummarizer::new();
        let result = sync_unprocessed_sessions(
            &entry, &paths, cc_home.path(), &mock,
        ).unwrap();

        assert_eq!(result.synced, 1);
        assert_eq!(result.skipped, 1);
    }

    #[test]
    fn sync_detects_updated_session() {
        let (_dir, paths) = setup();
        let entry = create_project(&paths);
        let cc_home = TempDir::new().unwrap();
        let cc_dir = create_cc_dir(cc_home.path(), &entry.path);
        write_transcript(&cc_dir, "session-aaa");

        let mock = MockSummarizer::new();

        // First sync processes the session
        let result = sync_unprocessed_sessions(
            &entry, &paths, cc_home.path(), &mock,
        ).unwrap();
        assert_eq!(result.synced, 1);
        assert_eq!(result.updated, 0);

        // Simulate a resumed session by touching the transcript file
        let transcript_path = cc_dir.join("session-aaa.jsonl");
        let content = std::fs::read_to_string(&transcript_path).unwrap();
        let appended = format!(
            "{content}\n{}\n{}",
            r#"{"type": "user", "message": {"role": "user", "content": "Resume work"}, "uuid": "c"}"#,
            r#"{"type": "assistant", "message": {"role": "assistant", "content": [{"type": "text", "text": "Resumed."}]}, "uuid": "d"}"#,
        );
        // Small delay to ensure mtime differs from ended_at
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(&transcript_path, appended).unwrap();

        // Second sync detects the updated transcript
        let result = sync_unprocessed_sessions(
            &entry, &paths, cc_home.path(), &mock,
        ).unwrap();
        assert_eq!(result.synced, 0);
        assert_eq!(result.updated, 1);

        // Session entry should be replaced, not duplicated
        let sessions = project_files::load_sessions(
            &paths.project_sessions(&entry.id),
        ).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, Some("session-aaa".to_string()));
    }

    #[test]
    fn sync_updated_preserves_other_sessions() {
        let (_dir, paths) = setup();
        let entry = create_project(&paths);
        let cc_home = TempDir::new().unwrap();
        let cc_dir = create_cc_dir(cc_home.path(), &entry.path);
        write_transcript(&cc_dir, "session-aaa");
        write_transcript(&cc_dir, "session-bbb");

        let mock = MockSummarizer::new();

        // First sync processes both
        sync_unprocessed_sessions(&entry, &paths, cc_home.path(), &mock).unwrap();

        // Touch only session-aaa
        std::thread::sleep(std::time::Duration::from_millis(50));
        let transcript_path = cc_dir.join("session-aaa.jsonl");
        let content = std::fs::read_to_string(&transcript_path).unwrap();
        std::fs::write(&transcript_path, format!("{content}\n")).unwrap();

        let result = sync_unprocessed_sessions(
            &entry, &paths, cc_home.path(), &mock,
        ).unwrap();
        assert_eq!(result.updated, 1);

        // Both sessions should still exist
        let sessions = project_files::load_sessions(
            &paths.project_sessions(&entry.id),
        ).unwrap();
        assert_eq!(sessions.len(), 2);
    }
}
