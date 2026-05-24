use std::process::Command;

use crate::project::{ProjectMeta, SessionEntry};

/// A reengagement brief, ready for display or system prompt injection.
#[derive(Debug, Clone)]
pub struct ReengagementBrief {
    pub name: String,
    pub id: String,
    pub status: String,
    pub description: String,
    pub last_active_relative: String,
    pub snapshot: String,
    pub open_questions: Vec<String>,
    pub next_task: String,
    pub last_commit: String,
    pub session_count: usize,
}

/// Build a reengagement brief from project data.
pub fn build(
    meta: &ProjectMeta,
    snapshot: &str,
    sessions: &[SessionEntry],
    last_active_relative: &str,
) -> ReengagementBrief {
    let last_session = sessions.last();

    let open_questions = last_session
        .map(|s| s.open_questions.clone())
        .unwrap_or_default();

    let next_task = last_session
        .map(|s| s.next_task.clone())
        .unwrap_or_default();

    let snapshot_text = if snapshot.trim().is_empty() {
        "No session history yet. Start working and the journal will populate automatically."
            .to_string()
    } else {
        snapshot.trim().to_string()
    };

    let last_commit = read_last_commit(&meta.path);

    ReengagementBrief {
        name: meta.name.clone(),
        id: meta.id.clone(),
        status: meta.status.to_string(),
        description: meta.description.clone(),
        last_active_relative: last_active_relative.to_string(),
        snapshot: snapshot_text,
        open_questions,
        next_task,
        last_commit,
        session_count: sessions.len(),
    }
}

/// Format the brief for terminal display.
pub fn format_terminal(brief: &ReengagementBrief) -> String {
    let separator = "━".repeat(50);
    let questions = if brief.open_questions.is_empty() {
        "  None recorded".to_string()
    } else {
        brief
            .open_questions
            .iter()
            .map(|q| format!("  - {q}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let next = if brief.next_task.is_empty() {
        "  None recorded".to_string()
    } else {
        format!("  {}", brief.next_task)
    };

    format!(
        "\
{separator}
  REENGAGING: {name}  [{id}]
  Status: {status} | Last active: {last_active}
{separator}

  WHAT THIS IS
  {description}

  WHERE YOU LEFT OFF
  {snapshot}

  OPEN QUESTIONS
{questions}

  NEXT TASK
{next}

  LAST COMMIT
  {last_commit}

  SESSIONS: {session_count} total

{separator}",
        name = brief.name,
        id = brief.id,
        status = brief.status,
        last_active = brief.last_active_relative,
        description = brief.description,
        snapshot = brief.snapshot,
        last_commit = brief.last_commit,
        session_count = brief.session_count,
    )
}

/// Format the brief as plain text for a system prompt file.
///
/// No terminal formatting characters. Kept under 500 words.
pub fn format_system_prompt(brief: &ReengagementBrief) -> String {
    let questions = if brief.open_questions.is_empty() {
        "None".to_string()
    } else {
        brief
            .open_questions
            .iter()
            .map(|q| format!("- {q}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let next = if brief.next_task.is_empty() {
        "None recorded".to_string()
    } else {
        brief.next_task.clone()
    };

    format!(
        "\
Reengagement context for project: {name} [{id}]
Status: {status}

Description: {description}

Where you left off:
{snapshot}

Open questions:
{questions}

Next task: {next}

Last commit: {last_commit}

Total sessions: {session_count}",
        name = brief.name,
        id = brief.id,
        status = brief.status,
        description = brief.description,
        snapshot = brief.snapshot,
        last_commit = brief.last_commit,
        session_count = brief.session_count,
    )
}

fn read_last_commit(project_path: &str) -> String {
    let path = crate::paths::expand_tilde(project_path);
    Command::new("git")
        .args(["-C", &path.display().to_string(), "log", "-1", "--oneline"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "No commits yet".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{ProjectMeta, ProjectPhase, ProjectStatus, SessionEntry};
    use chrono::{TimeZone, Utc};

    fn sample_meta() -> ProjectMeta {
        ProjectMeta {
            id: "proj_abc123".to_string(),
            name: "invoice-parser".to_string(),
            description: "CLI tool that extracts line items from PDF invoices".to_string(),
            path: "/tmp/nonexistent-project".to_string(),
            status: ProjectStatus::Active,
            phase: ProjectPhase::Implementation,
            created_at: Utc.with_ymd_and_hms(2026, 1, 14, 9, 0, 0).unwrap(),
            last_active_at: Utc.with_ymd_and_hms(2026, 2, 28, 16, 0, 0).unwrap(),
            blocked_reason: None,
            tags: vec![],
            priority: "medium".to_string(),
            cc_session_ids: vec![],
            cc_model: String::new(),
        }
    }

    fn sample_session() -> SessionEntry {
        SessionEntry {
            id: "sess_aaa111".to_string(),
            session_id: Some("cc-1".to_string()),
            ended_at: Utc.with_ymd_and_hms(2026, 2, 28, 16, 0, 0).unwrap(),
            summary: "Implemented PDF text extraction.".to_string(),
            next_task: "Add table detection.".to_string(),
            open_questions: vec![
                "Support scanned PDFs?".to_string(),
                "JSON or CSV output?".to_string(),
            ],
            human_note: None,
        }
    }

    #[test]
    fn build_with_sessions() {
        let meta = sample_meta();
        let sessions = vec![sample_session()];
        let brief = build(&meta, "Working on PDF parsing.", &sessions, "3 days ago");

        assert_eq!(brief.name, "invoice-parser");
        assert_eq!(brief.id, "proj_abc123");
        assert_eq!(brief.next_task, "Add table detection.");
        assert_eq!(brief.open_questions.len(), 2);
        assert_eq!(brief.session_count, 1);
        assert_eq!(brief.snapshot, "Working on PDF parsing.");
    }

    #[test]
    fn build_with_empty_snapshot() {
        let meta = sample_meta();
        let brief = build(&meta, "", &[], "5 days ago");
        assert!(brief.snapshot.contains("No session history yet"));
    }

    #[test]
    fn build_with_no_sessions() {
        let meta = sample_meta();
        let brief = build(&meta, "Some snapshot.", &[], "1 day ago");

        assert!(brief.open_questions.is_empty());
        assert!(brief.next_task.is_empty());
        assert_eq!(brief.session_count, 0);
    }

    #[test]
    fn format_terminal_contains_key_sections() {
        let meta = sample_meta();
        let sessions = vec![sample_session()];
        let brief = build(&meta, "Working on PDF parsing.", &sessions, "3 days ago");
        let output = format_terminal(&brief);

        assert!(output.contains("REENGAGING: invoice-parser"));
        assert!(output.contains("[proj_abc123]"));
        assert!(output.contains("WHAT THIS IS"));
        assert!(output.contains("WHERE YOU LEFT OFF"));
        assert!(output.contains("OPEN QUESTIONS"));
        assert!(output.contains("NEXT TASK"));
        assert!(output.contains("LAST COMMIT"));
        assert!(output.contains("SESSIONS: 1 total"));
    }

    #[test]
    fn format_terminal_no_questions() {
        let meta = sample_meta();
        let brief = build(&meta, "Snapshot.", &[], "1 day ago");
        let output = format_terminal(&brief);
        assert!(output.contains("None recorded"));
    }

    #[test]
    fn format_system_prompt_is_plain_text() {
        let meta = sample_meta();
        let sessions = vec![sample_session()];
        let brief = build(&meta, "Working on PDF.", &sessions, "3 days ago");
        let output = format_system_prompt(&brief);

        assert!(!output.contains('━'));
        assert!(output.contains("invoice-parser"));
        assert!(output.contains("Add table detection."));
        assert!(output.contains("Support scanned PDFs?"));
    }

    #[test]
    fn uses_last_session_for_questions() {
        let meta = sample_meta();
        let mut s1 = sample_session();
        s1.open_questions = vec!["Old question?".to_string()];
        s1.next_task = "Old task".to_string();

        let mut s2 = sample_session();
        s2.id = "sess_bbb222".to_string();
        s2.open_questions = vec!["New question?".to_string()];
        s2.next_task = "New task".to_string();

        let brief = build(&meta, "snapshot", &[s1, s2], "1 hour ago");
        assert_eq!(brief.next_task, "New task");
        assert_eq!(brief.open_questions, vec!["New question?"]);
    }
}
