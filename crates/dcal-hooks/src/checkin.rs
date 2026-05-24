use std::path::Path;
use std::process::Command;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use dcal_core::id::generate_session_id;
use dcal_core::paths::{collapse_to_tilde, DcalPaths};
use dcal_core::project::{ProjectPhase, RegistryEntry, SessionEntry};
use dcal_core::project_files;
use dcal_core::registry;

use crate::transcript;

#[derive(Debug, Error)]
pub enum CheckinError {
    #[error("failed to read stdin: {0}")]
    Stdin(String),

    #[error("no project found for path: {0}")]
    ProjectNotFound(String),

    #[error("transcript error: {0}")]
    Transcript(#[from] transcript::TranscriptError),

    #[error("failed to generate summary: {0}")]
    Summary(String),

    #[error("failed to parse summary: {0}")]
    SummaryParse(String),

    #[error("project file error: {0}")]
    ProjectFile(#[from] project_files::ProjectFileError),

    #[error("registry error: {0}")]
    Registry(#[from] registry::RegistryError),
}

/// Input received from the SessionEnd hook via stdin.
#[derive(Debug, Deserialize)]
pub struct HookInput {
    pub session_id: String,
    pub transcript_path: String,
    pub cwd: String,
}

/// LLM-generated session summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub summary: String,
    pub next_task: String,
    #[serde(default)]
    pub open_questions: Vec<String>,
    #[serde(default)]
    pub blockers: Vec<String>,
    pub phase: String,
}

/// Run an automatic checkin from the SessionEnd hook.
///
/// Returns `Ok(true)` if a checkin was performed, `Ok(false)` if the
/// cwd doesn't match any registered project (non-dcal session).
pub fn auto_checkin(
    paths: &DcalPaths,
    input: &HookInput,
) -> Result<bool, CheckinError> {
    // Detect project from cwd
    let entries = registry::load(&paths.registry())
        .map_err(|e| CheckinError::Stdin(e.to_string()))?;

    let entry = match find_project_by_cwd(&entries, &input.cwd) {
        Some(e) => e.clone(),
        None => return Ok(false),
    };

    // Read transcript
    let transcript_path = Path::new(&input.transcript_path);
    let transcript_content = transcript::read_transcript(transcript_path)?;

    if transcript_content.trim().is_empty() {
        return Ok(false);
    }

    // Generate summary via claude -p
    let summary = generate_summary(&transcript_content)?;

    // Apply the checkin (hook mode fires at session end, so now is correct)
    apply_checkin(paths, &entry, Some(&input.session_id), &summary, None)?;

    Ok(true)
}

/// Apply a checkin with a pre-built summary (used by both auto and manual modes).
///
/// When `ended_at` is `Some`, that timestamp is used for the session entry and
/// journal header (e.g. the original CC session end time). When `None`, falls
/// back to the current time (appropriate for manual checkins).
pub fn apply_checkin(
    paths: &DcalPaths,
    entry: &RegistryEntry,
    cc_session_id: Option<&str>,
    summary: &SessionSummary,
    ended_at: Option<chrono::DateTime<Utc>>,
) -> Result<(), CheckinError> {
    let now = ended_at.unwrap_or_else(Utc::now);

    // Load current meta
    let meta_path = paths.project_meta(&entry.id);
    let mut meta = project_files::load_meta(&meta_path)?;

    // Check for phase change
    let new_phase: Option<ProjectPhase> = summary.phase.parse().ok();
    let phase_changed = new_phase
        .map(|p| p != meta.phase)
        .unwrap_or(false);

    if let Some(phase) = new_phase {
        if phase_changed {
            meta.phase = phase;
        }
    }

    // Write session entry
    let session_entry = SessionEntry {
        id: generate_session_id(),
        session_id: cc_session_id.map(String::from),
        ended_at: now,
        summary: summary.summary.clone(),
        next_task: summary.next_task.clone(),
        open_questions: summary.open_questions.clone(),
        human_note: None,
    };

    project_files::append_session(
        &paths.project_sessions(&entry.id),
        &session_entry,
    )?;

    // Write journal entry
    let mut journal_text = format!(
        "\n## Session — {}\n\n{}\n\n**Next:** {}\n",
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

    if phase_changed {
        if let Some(phase) = new_phase {
            journal_text.push_str(&format!(
                "\n**Phase:** {} → {}\n",
                entry.status, phase
            ));
        }
    }

    project_files::append_journal(
        &paths.project_journal(&entry.id),
        &journal_text,
    )?;

    // Update snapshot
    let snapshot = format!(
        "{}\n\nNext: {}\n",
        summary.summary, summary.next_task,
    );
    project_files::save_snapshot(
        &paths.project_snapshot(&entry.id),
        &snapshot,
    )?;

    // Update meta
    meta.last_active_at = now;
    project_files::save_meta(&meta_path, &meta)?;

    // Sync registry
    let updated_entry = RegistryEntry::from(&meta);
    registry::update(&paths.registry(), &updated_entry)?;

    Ok(())
}

/// Find a registered project whose path matches the given cwd.
fn find_project_by_cwd<'a>(
    entries: &'a [RegistryEntry],
    cwd: &str,
) -> Option<&'a RegistryEntry> {
    let cwd_collapsed = collapse_to_tilde(Path::new(cwd));

    entries.iter().find(|e| {
        e.path == cwd_collapsed || e.path == cwd
    })
}

/// Call `claude -p` to generate a session summary from transcript content.
fn generate_summary(transcript: &str) -> Result<SessionSummary, CheckinError> {
    let prompt = format!(
        "Summarize this Claude Code session in structured format.\n\
         Respond with JSON only, no other text.\n\n\
         {{\n  \
           \"summary\": \"2-3 sentences: what was accomplished\",\n  \
           \"next_task\": \"the single most important next concrete task\",\n  \
           \"open_questions\": [\"question 1\", \"question 2\"],\n  \
           \"blockers\": [],\n  \
           \"phase\": \"one of: ideation, design, implementation, testing, maintenance\"\n\
         }}\n\n\
         SESSION TRANSCRIPT:\n{transcript}"
    );

    let output = Command::new("claude")
        .args(["-p", "--output-format", "json", "--max-turns", "1"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(prompt.as_bytes())?;
            }
            child.wait_with_output()
        })
        .map_err(|e| CheckinError::Summary(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CheckinError::Summary(format!(
            "claude -p failed: {stderr}"
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // The --output-format json wraps the response; extract the result text
    let response_text = extract_claude_response(&stdout)?;

    parse_summary(&response_text)
}

/// Extract the text content from a claude --output-format json response.
fn extract_claude_response(json_str: &str) -> Result<String, CheckinError> {
    let value: serde_json::Value = serde_json::from_str(json_str.trim())
        .map_err(|e| CheckinError::SummaryParse(format!("invalid JSON from claude: {e}")))?;

    // The response may have a "result" field with the text
    if let Some(result) = value.get("result").and_then(|r| r.as_str()) {
        return Ok(result.to_string());
    }

    // Or it might be the raw text directly
    if let Some(s) = value.as_str() {
        return Ok(s.to_string());
    }

    // Try to find text in content blocks
    if let Some(content) = value.get("content").and_then(|c| c.as_array()) {
        for block in content {
            if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                return Ok(text.to_string());
            }
        }
    }

    Err(CheckinError::SummaryParse(
        "could not extract text from claude response".to_string(),
    ))
}

/// Parse a JSON summary string, handling markdown code fences.
pub fn parse_summary(text: &str) -> Result<SessionSummary, CheckinError> {
    let cleaned = strip_code_fences(text);
    serde_json::from_str(&cleaned)
        .map_err(|e| CheckinError::SummaryParse(format!("{e}: {cleaned}")))
}

fn strip_code_fences(s: &str) -> String {
    let trimmed = s.trim();
    if let Some(rest) = trimmed.strip_prefix("```json") {
        rest.trim()
            .strip_suffix("```")
            .unwrap_or(rest.trim())
            .trim()
            .to_string()
    } else if let Some(rest) = trimmed.strip_prefix("```") {
        rest.trim()
            .strip_suffix("```")
            .unwrap_or(rest.trim())
            .trim()
            .to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dcal_core::project::ProjectStatus;
    use tempfile::TempDir;

    fn setup() -> (TempDir, DcalPaths) {
        let dir = TempDir::new().unwrap();
        let paths = DcalPaths::new(dir.path().to_path_buf());
        std::fs::create_dir_all(paths.projects_dir()).unwrap();
        std::fs::write(paths.registry(), "[]").unwrap();
        (dir, paths)
    }

    fn create_test_project(paths: &DcalPaths) -> RegistryEntry {
        use dcal_core::project::{ProjectMeta, ProjectPhase};

        let id = "proj_test01".to_string();
        let now = Utc::now();
        let meta = ProjectMeta {
            id: id.clone(),
            name: "test-project".to_string(),
            description: "A test project".to_string(),
            path: "~/projects/test-project".to_string(),
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
            &paths.project_dir(&id),
            &meta,
            "test idea",
        )
        .unwrap();

        let entry = RegistryEntry::from(&meta);
        registry::add(&paths.registry(), entry.clone()).unwrap();
        entry
    }

    fn sample_summary() -> SessionSummary {
        SessionSummary {
            summary: "Implemented PDF text extraction.".to_string(),
            next_task: "Add table detection.".to_string(),
            open_questions: vec!["Support OCR?".to_string()],
            blockers: vec![],
            phase: "implementation".to_string(),
        }
    }

    #[test]
    fn parse_summary_basic() {
        let json = r#"{
            "summary": "Did some work.",
            "next_task": "Do more work.",
            "open_questions": [],
            "blockers": [],
            "phase": "implementation"
        }"#;
        let result = parse_summary(json).unwrap();
        assert_eq!(result.summary, "Did some work.");
        assert_eq!(result.next_task, "Do more work.");
    }

    #[test]
    fn parse_summary_with_code_fences() {
        let json = "```json\n{\"summary\": \"work\", \"next_task\": \"more\", \"open_questions\": [], \"blockers\": [], \"phase\": \"design\"}\n```";
        let result = parse_summary(json).unwrap();
        assert_eq!(result.summary, "work");
        assert_eq!(result.phase, "design");
    }

    #[test]
    fn parse_summary_invalid_json() {
        let result = parse_summary("not json");
        assert!(result.is_err());
    }

    #[test]
    fn find_project_by_cwd_match() {
        let entries = vec![RegistryEntry {
            id: "proj_abc123".to_string(),
            name: "myapp".to_string(),
            path: "~/projects/myapp".to_string(),
            status: ProjectStatus::Active,
            created_at: Utc::now(),
            last_active_at: Utc::now(),
        }];

        let home = std::env::var("HOME").unwrap();
        let abs_path = format!("{home}/projects/myapp");
        let result = find_project_by_cwd(&entries, &abs_path);
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, "proj_abc123");
    }

    #[test]
    fn find_project_by_cwd_no_match() {
        let entries = vec![RegistryEntry {
            id: "proj_abc123".to_string(),
            name: "myapp".to_string(),
            path: "~/projects/myapp".to_string(),
            status: ProjectStatus::Active,
            created_at: Utc::now(),
            last_active_at: Utc::now(),
        }];

        let result = find_project_by_cwd(&entries, "/some/other/path");
        assert!(result.is_none());
    }

    #[test]
    fn apply_checkin_writes_session() {
        let (_dir, paths) = setup();
        let entry = create_test_project(&paths);

        apply_checkin(&paths, &entry, Some("cc-123"), &sample_summary(), None).unwrap();

        let sessions = project_files::load_sessions(
            &paths.project_sessions(&entry.id),
        )
        .unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].summary, "Implemented PDF text extraction.");
        assert_eq!(sessions[0].session_id, Some("cc-123".to_string()));
    }

    #[test]
    fn apply_checkin_writes_journal() {
        let (_dir, paths) = setup();
        let entry = create_test_project(&paths);

        apply_checkin(&paths, &entry, None, &sample_summary(), None).unwrap();

        let journal = project_files::load_journal(
            &paths.project_journal(&entry.id),
        )
        .unwrap();
        assert!(journal.contains("Implemented PDF text extraction."));
        assert!(journal.contains("Add table detection."));
        assert!(journal.contains("Support OCR?"));
    }

    #[test]
    fn apply_checkin_updates_snapshot() {
        let (_dir, paths) = setup();
        let entry = create_test_project(&paths);

        apply_checkin(&paths, &entry, None, &sample_summary(), None).unwrap();

        let snapshot = project_files::load_snapshot(
            &paths.project_snapshot(&entry.id),
        )
        .unwrap();
        assert!(snapshot.contains("Implemented PDF text extraction."));
        assert!(snapshot.contains("Add table detection."));
    }

    #[test]
    fn apply_checkin_updates_registry() {
        let (_dir, paths) = setup();
        let entry = create_test_project(&paths);
        let before = entry.last_active_at;

        apply_checkin(&paths, &entry, None, &sample_summary(), None).unwrap();

        let entries = registry::load(&paths.registry()).unwrap();
        assert!(entries[0].last_active_at > before);
    }

    #[test]
    fn apply_checkin_manual_mode_null_session_id() {
        let (_dir, paths) = setup();
        let entry = create_test_project(&paths);

        apply_checkin(&paths, &entry, None, &sample_summary(), None).unwrap();

        let sessions = project_files::load_sessions(
            &paths.project_sessions(&entry.id),
        )
        .unwrap();
        assert!(sessions[0].session_id.is_none());
    }

    #[test]
    fn apply_checkin_detects_phase_change() {
        let (_dir, paths) = setup();
        let entry = create_test_project(&paths);

        let mut summary = sample_summary();
        summary.phase = "testing".to_string();

        apply_checkin(&paths, &entry, None, &summary, None).unwrap();

        let meta = project_files::load_meta(&paths.project_meta(&entry.id)).unwrap();
        assert_eq!(meta.phase, ProjectPhase::Testing);
    }

    #[test]
    fn apply_checkin_uses_provided_ended_at() {
        let (_dir, paths) = setup();
        let entry = create_test_project(&paths);

        let past = chrono::DateTime::parse_from_rfc3339("2026-01-15T14:30:00Z")
            .unwrap()
            .with_timezone(&Utc);

        apply_checkin(&paths, &entry, Some("cc-old"), &sample_summary(), Some(past)).unwrap();

        let sessions = project_files::load_sessions(
            &paths.project_sessions(&entry.id),
        ).unwrap();
        assert_eq!(sessions[0].ended_at, past);

        let journal = project_files::load_journal(
            &paths.project_journal(&entry.id),
        ).unwrap();
        assert!(journal.contains("2026-01-15 14:30 UTC"));
    }
}
