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
}

/// Result of a sync operation.
#[derive(Debug)]
pub struct SyncResult {
    pub synced: usize,
    pub skipped: usize,
}

/// Sync unprocessed CC sessions for a project.
///
/// Finds JSONL transcripts in the CC project directory that aren't already
/// in sessions.json, summarizes each, and writes journal/snapshot/session entries.
pub fn sync_unprocessed_sessions(
    entry: &RegistryEntry,
    paths: &DcalPaths,
    cc_home: &Path,
    summarizer: &dyn Summarizer,
) -> Result<SyncResult, SyncError> {
    let cc_dir = cc_projects::cc_project_dir(cc_home, &entry.path);

    if !cc_dir.exists() {
        return Ok(SyncResult { synced: 0, skipped: 0 });
    }

    let sessions = project_files::load_sessions(&paths.project_sessions(&entry.id))?;
    let known_ids = cc_projects::known_cc_session_ids(&sessions);
    let unprocessed = cc_projects::find_unprocessed(&cc_dir, &known_ids);

    if unprocessed.is_empty() {
        return Ok(SyncResult { synced: 0, skipped: 0 });
    }

    let total = unprocessed.len();
    let mut synced = 0;
    let mut skipped = 0;

    for (i, (session_id, transcript_path)) in unprocessed.iter().enumerate() {
        eprintln!(
            "  Syncing session {} of {}...",
            i + 1,
            total
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

        crate::checkin::apply_checkin(paths, entry, Some(session_id), &summary)?;
        synced += 1;
    }

    Ok(SyncResult { synced, skipped })
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
}
