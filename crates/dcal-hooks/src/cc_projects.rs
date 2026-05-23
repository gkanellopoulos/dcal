use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Derive the CC project directory slug from an absolute path.
///
/// CC stores project data under `~/.claude/projects/<slug>/` where the
/// slug is the absolute path with `/` replaced by `-`.
pub fn derive_cc_slug(absolute_path: &str) -> String {
    absolute_path.replace('/', "-")
}

/// Build the full CC project directory path.
///
/// Expands tilde before deriving the slug, since CC uses absolute paths.
pub fn cc_project_dir(cc_home: &Path, project_path: &str) -> PathBuf {
    let absolute = dcal_core::paths::expand_tilde(project_path);
    let slug = derive_cc_slug(&absolute.to_string_lossy());
    cc_home.join("projects").join(slug)
}

/// List all session transcripts in a CC project directory.
///
/// Returns `(session_id, path)` pairs where `session_id` is the JSONL
/// filename stem (a UUID assigned by CC).
pub fn list_transcripts(cc_dir: &Path) -> Vec<(String, PathBuf)> {
    let read_dir = match std::fs::read_dir(cc_dir) {
        Ok(rd) => rd,
        Err(_) => return Vec::new(),
    };

    let mut transcripts = Vec::new();

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.extension().and_then(OsStr::to_str) == Some("jsonl") {
            if let Some(stem) = path.file_stem().and_then(OsStr::to_str) {
                transcripts.push((stem.to_string(), path));
            }
        }
    }

    transcripts
}

/// Find transcripts that haven't been processed yet.
///
/// Compares the JSONL files in `cc_dir` against a set of known CC session
/// IDs (from sessions.json) and returns only the unprocessed ones.
pub fn find_unprocessed(
    cc_dir: &Path,
    known_session_ids: &HashSet<String>,
) -> Vec<(String, PathBuf)> {
    list_transcripts(cc_dir)
        .into_iter()
        .filter(|(id, _)| !known_session_ids.contains(id))
        .collect()
}

/// Build a set of known CC session IDs from session entries.
pub fn known_cc_session_ids(
    sessions: &[dcal_core::project::SessionEntry],
) -> HashSet<String> {
    sessions
        .iter()
        .filter_map(|s| s.session_id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn slug_from_absolute_path() {
        assert_eq!(
            derive_cc_slug("/home/dev/projects/foo"),
            "-home-dev-projects-foo"
        );
    }

    #[test]
    fn slug_from_root() {
        assert_eq!(derive_cc_slug("/"), "-");
    }

    #[test]
    fn slug_preserves_hyphens_in_name() {
        assert_eq!(
            derive_cc_slug("/home/user/my-project"),
            "-home-user-my-project"
        );
    }

    #[test]
    fn cc_project_dir_builds_path() {
        let home = Path::new("/home/dev/.claude");
        let result = cc_project_dir(home, "/home/dev/projects/foo");
        assert_eq!(
            result,
            PathBuf::from("/home/dev/.claude/projects/-home-dev-projects-foo")
        );
    }

    #[test]
    fn cc_project_dir_expands_tilde() {
        let home = Path::new("/home/dev/.claude");
        let result = cc_project_dir(home, "~/projects/foo");
        let result_str = result.to_string_lossy();
        assert!(!result_str.contains('~'), "tilde was not expanded: {result_str}");
        assert!(result_str.ends_with("-projects-foo"), "unexpected slug: {result_str}");
    }

    #[test]
    fn list_transcripts_empty_dir() {
        let dir = TempDir::new().unwrap();
        let result = list_transcripts(dir.path());
        assert!(result.is_empty());
    }

    #[test]
    fn list_transcripts_finds_jsonl() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("abc-123.jsonl"), "{}").unwrap();
        std::fs::write(dir.path().join("def-456.jsonl"), "{}").unwrap();
        std::fs::write(dir.path().join("readme.md"), "not a transcript").unwrap();

        let mut result = list_transcripts(dir.path());
        result.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "abc-123");
        assert_eq!(result[1].0, "def-456");
    }

    #[test]
    fn list_transcripts_nonexistent_dir() {
        let result = list_transcripts(Path::new("/nonexistent/path"));
        assert!(result.is_empty());
    }

    #[test]
    fn find_unprocessed_filters_known() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("session-aaa.jsonl"), "{}").unwrap();
        std::fs::write(dir.path().join("session-bbb.jsonl"), "{}").unwrap();
        std::fs::write(dir.path().join("session-ccc.jsonl"), "{}").unwrap();

        let known: HashSet<String> =
            ["session-aaa".to_string(), "session-ccc".to_string()]
                .into_iter()
                .collect();

        let result = find_unprocessed(dir.path(), &known);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "session-bbb");
    }

    #[test]
    fn find_unprocessed_all_known() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("session-aaa.jsonl"), "{}").unwrap();

        let known: HashSet<String> =
            ["session-aaa".to_string()].into_iter().collect();

        let result = find_unprocessed(dir.path(), &known);
        assert!(result.is_empty());
    }

    #[test]
    fn find_unprocessed_none_known() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("session-aaa.jsonl"), "{}").unwrap();
        std::fs::write(dir.path().join("session-bbb.jsonl"), "{}").unwrap();

        let known: HashSet<String> = HashSet::new();

        let result = find_unprocessed(dir.path(), &known);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn known_cc_session_ids_extracts_ids() {
        use dcal_core::project::SessionEntry;
        use chrono::Utc;

        let sessions = vec![
            SessionEntry {
                id: "sess_aaa".to_string(),
                session_id: Some("cc-111".to_string()),
                ended_at: Utc::now(),
                summary: "test".to_string(),
                next_task: "test".to_string(),
                open_questions: vec![],
                human_note: None,
            },
            SessionEntry {
                id: "sess_bbb".to_string(),
                session_id: None, // manual checkin
                ended_at: Utc::now(),
                summary: "test".to_string(),
                next_task: "test".to_string(),
                open_questions: vec![],
                human_note: None,
            },
            SessionEntry {
                id: "sess_ccc".to_string(),
                session_id: Some("cc-222".to_string()),
                ended_at: Utc::now(),
                summary: "test".to_string(),
                next_task: "test".to_string(),
                open_questions: vec![],
                human_note: None,
            },
        ];

        let known = known_cc_session_ids(&sessions);
        assert_eq!(known.len(), 2);
        assert!(known.contains("cc-111"));
        assert!(known.contains("cc-222"));
        assert!(!known.contains("sess_aaa"));
    }
}
