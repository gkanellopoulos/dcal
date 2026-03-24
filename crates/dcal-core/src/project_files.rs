use std::fs;
use std::path::Path;
use thiserror::Error;

use crate::lock::FileLock;
use crate::project::{ProjectMeta, SessionEntry};

#[derive(Debug, Error)]
pub enum ProjectFileError {
    #[error("failed to read {path}: {source}")]
    Read { path: String, source: std::io::Error },

    #[error("failed to write {path}: {source}")]
    Write { path: String, source: std::io::Error },

    #[error("failed to parse {path}: {source}")]
    Parse { path: String, source: serde_json::Error },

    #[error("failed to serialize: {0}")]
    Serialize(serde_json::Error),

    #[error("failed to lock file: {0}")]
    Lock(#[from] crate::lock::LockError),
}

// -- meta.json --

/// Load project metadata from `meta.json`.
pub fn load_meta(path: &Path) -> Result<ProjectMeta, ProjectFileError> {
    let content = fs::read_to_string(path).map_err(|source| ProjectFileError::Read {
        path: path.display().to_string(),
        source,
    })?;

    serde_json::from_str(&content).map_err(|source| ProjectFileError::Parse {
        path: path.display().to_string(),
        source,
    })
}

/// Save project metadata to `meta.json`, acquiring a file lock first.
pub fn save_meta(path: &Path, meta: &ProjectMeta) -> Result<(), ProjectFileError> {
    let _lock = FileLock::acquire(path)?;
    let json = serde_json::to_string_pretty(meta).map_err(ProjectFileError::Serialize)?;
    fs::write(path, json).map_err(|source| ProjectFileError::Write {
        path: path.display().to_string(),
        source,
    })
}

// -- sessions.json --

/// Load all session entries from `sessions.json`.
///
/// Returns an empty vec if the file doesn't exist.
pub fn load_sessions(path: &Path) -> Result<Vec<SessionEntry>, ProjectFileError> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path).map_err(|source| ProjectFileError::Read {
        path: path.display().to_string(),
        source,
    })?;

    serde_json::from_str(&content).map_err(|source| ProjectFileError::Parse {
        path: path.display().to_string(),
        source,
    })
}

/// Append a session entry to `sessions.json`, acquiring a file lock first.
pub fn append_session(path: &Path, entry: &SessionEntry) -> Result<(), ProjectFileError> {
    let _lock = FileLock::acquire(path)?;

    let mut sessions = if path.exists() {
        let content = fs::read_to_string(path).map_err(|source| ProjectFileError::Read {
            path: path.display().to_string(),
            source,
        })?;
        serde_json::from_str(&content).map_err(|source| ProjectFileError::Parse {
            path: path.display().to_string(),
            source,
        })?
    } else {
        Vec::new()
    };

    sessions.push(entry.clone());

    let json = serde_json::to_string_pretty(&sessions).map_err(ProjectFileError::Serialize)?;
    fs::write(path, json).map_err(|source| ProjectFileError::Write {
        path: path.display().to_string(),
        source,
    })
}

// -- journal.md --

/// Read the full journal content.
///
/// Returns an empty string if the file doesn't exist.
pub fn load_journal(path: &Path) -> Result<String, ProjectFileError> {
    if !path.exists() {
        return Ok(String::new());
    }

    fs::read_to_string(path).map_err(|source| ProjectFileError::Read {
        path: path.display().to_string(),
        source,
    })
}

/// Append text to the journal file, acquiring a file lock first.
pub fn append_journal(path: &Path, text: &str) -> Result<(), ProjectFileError> {
    use std::io::Write;

    let _lock = FileLock::acquire(path)?;

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| ProjectFileError::Write {
            path: path.display().to_string(),
            source,
        })?;

    file.write_all(text.as_bytes()).map_err(|source| ProjectFileError::Write {
        path: path.display().to_string(),
        source,
    })
}

// -- snapshot.md --

/// Read the current snapshot.
///
/// Returns an empty string if the file doesn't exist.
pub fn load_snapshot(path: &Path) -> Result<String, ProjectFileError> {
    if !path.exists() {
        return Ok(String::new());
    }

    fs::read_to_string(path).map_err(|source| ProjectFileError::Read {
        path: path.display().to_string(),
        source,
    })
}

/// Overwrite the snapshot file with new content.
pub fn save_snapshot(path: &Path, content: &str) -> Result<(), ProjectFileError> {
    fs::write(path, content).map_err(|source| ProjectFileError::Write {
        path: path.display().to_string(),
        source,
    })
}

// -- idea.md --

/// Read the idea file.
///
/// Returns an empty string if the file doesn't exist.
pub fn load_idea(path: &Path) -> Result<String, ProjectFileError> {
    if !path.exists() {
        return Ok(String::new());
    }

    fs::read_to_string(path).map_err(|source| ProjectFileError::Read {
        path: path.display().to_string(),
        source,
    })
}

/// Write the idea file. This is written once at project creation.
pub fn save_idea(path: &Path, content: &str) -> Result<(), ProjectFileError> {
    fs::write(path, content).map_err(|source| ProjectFileError::Write {
        path: path.display().to_string(),
        source,
    })
}

/// Create the full project directory structure with initial files.
pub fn create_project_dir(
    project_dir: &Path,
    meta: &ProjectMeta,
    idea_text: &str,
) -> Result<(), ProjectFileError> {
    fs::create_dir_all(project_dir).map_err(|source| ProjectFileError::Write {
        path: project_dir.display().to_string(),
        source,
    })?;

    save_meta(&project_dir.join("meta.json"), meta)?;
    save_idea(&project_dir.join("idea.md"), idea_text)?;
    save_snapshot(&project_dir.join("snapshot.md"), "")?;
    append_journal(&project_dir.join("journal.md"), "")?;

    let empty_sessions: Vec<SessionEntry> = Vec::new();
    let json = serde_json::to_string_pretty(&empty_sessions).map_err(ProjectFileError::Serialize)?;
    fs::write(project_dir.join("sessions.json"), json).map_err(|source| {
        ProjectFileError::Write {
            path: project_dir.join("sessions.json").display().to_string(),
            source,
        }
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{ProjectPhase, ProjectStatus};
    use chrono::{TimeZone, Utc};
    use tempfile::TempDir;

    fn sample_meta() -> ProjectMeta {
        ProjectMeta {
            id: "proj_abc123".to_string(),
            name: "test-project".to_string(),
            description: "A test project".to_string(),
            path: "~/projects/test-project".to_string(),
            status: ProjectStatus::Active,
            phase: ProjectPhase::Ideation,
            created_at: Utc.with_ymd_and_hms(2026, 1, 14, 9, 0, 0).unwrap(),
            last_active_at: Utc.with_ymd_and_hms(2026, 1, 14, 9, 0, 0).unwrap(),
            blocked_reason: None,
            tags: vec![],
            priority: "medium".to_string(),
            cc_session_ids: vec![],
        }
    }

    fn sample_session() -> SessionEntry {
        SessionEntry {
            id: "sess_aaa111".to_string(),
            session_id: Some("cc-session-1".to_string()),
            ended_at: Utc.with_ymd_and_hms(2026, 2, 1, 12, 0, 0).unwrap(),
            summary: "Did some work.".to_string(),
            next_task: "Do more work.".to_string(),
            open_questions: vec![],
            human_note: None,
        }
    }

    // -- meta.json tests --

    #[test]
    fn meta_save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("meta.json");

        let meta = sample_meta();
        save_meta(&path, &meta).unwrap();

        let loaded = load_meta(&path).unwrap();
        assert_eq!(meta, loaded);
    }

    // -- sessions.json tests --

    #[test]
    fn sessions_load_returns_empty_when_no_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sessions.json");
        let sessions = load_sessions(&path).unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn sessions_append_and_load() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sessions.json");

        let s1 = sample_session();
        let mut s2 = sample_session();
        s2.id = "sess_bbb222".to_string();
        s2.summary = "Second session.".to_string();

        append_session(&path, &s1).unwrap();
        append_session(&path, &s2).unwrap();

        let loaded = load_sessions(&path).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].id, "sess_aaa111");
        assert_eq!(loaded[1].id, "sess_bbb222");
    }

    // -- journal.md tests --

    #[test]
    fn journal_load_returns_empty_when_no_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.md");
        let content = load_journal(&path).unwrap();
        assert!(content.is_empty());
    }

    #[test]
    fn journal_append_accumulates() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.md");

        append_journal(&path, "## Session 1\nDid things.\n\n").unwrap();
        append_journal(&path, "## Session 2\nDid more things.\n\n").unwrap();

        let content = load_journal(&path).unwrap();
        assert!(content.contains("Session 1"));
        assert!(content.contains("Session 2"));
    }

    // -- snapshot.md tests --

    #[test]
    fn snapshot_save_and_load() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("snapshot.md");

        save_snapshot(&path, "Current state of things.").unwrap();
        let content = load_snapshot(&path).unwrap();
        assert_eq!(content, "Current state of things.");
    }

    #[test]
    fn snapshot_overwrite() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("snapshot.md");

        save_snapshot(&path, "Old state.").unwrap();
        save_snapshot(&path, "New state.").unwrap();

        let content = load_snapshot(&path).unwrap();
        assert_eq!(content, "New state.");
    }

    // -- idea.md tests --

    #[test]
    fn idea_save_and_load() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("idea.md");

        save_idea(&path, "Build a CLI tool for invoices.").unwrap();
        let content = load_idea(&path).unwrap();
        assert_eq!(content, "Build a CLI tool for invoices.");
    }

    // -- create_project_dir tests --

    #[test]
    fn create_project_dir_writes_all_files() {
        let dir = TempDir::new().unwrap();
        let project_dir = dir.path().join("proj_abc123");

        let meta = sample_meta();
        create_project_dir(&project_dir, &meta, "My project idea.").unwrap();

        assert!(project_dir.join("meta.json").exists());
        assert!(project_dir.join("idea.md").exists());
        assert!(project_dir.join("snapshot.md").exists());
        assert!(project_dir.join("journal.md").exists());
        assert!(project_dir.join("sessions.json").exists());

        let loaded_meta = load_meta(&project_dir.join("meta.json")).unwrap();
        assert_eq!(loaded_meta.name, "test-project");

        let idea = load_idea(&project_dir.join("idea.md")).unwrap();
        assert_eq!(idea, "My project idea.");

        let sessions = load_sessions(&project_dir.join("sessions.json")).unwrap();
        assert!(sessions.is_empty());
    }
}
