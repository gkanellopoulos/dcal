use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Status of a project in its lifecycle.
///
/// Only `Active` and `Paused` have dedicated commands in v0.1.
/// The full enum is defined so the state machine is correct from the start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    Ideation,
    Active,
    Paused,
    Blocked,
    Completed,
    Archived,
}

impl fmt::Display for ProjectStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Ideation => "ideation",
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Blocked => "blocked",
            Self::Completed => "completed",
            Self::Archived => "archived",
        };
        f.write_str(s)
    }
}

impl FromStr for ProjectStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ideation" => Ok(Self::Ideation),
            "active" => Ok(Self::Active),
            "paused" => Ok(Self::Paused),
            "blocked" => Ok(Self::Blocked),
            "completed" => Ok(Self::Completed),
            "archived" => Ok(Self::Archived),
            _ => Err(format!("unknown status: {s}")),
        }
    }
}

/// Development phase of a project.
///
/// Automatically inferred by LLM during checkin, or set manually
/// via `dcal phase`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectPhase {
    Ideation,
    Design,
    Implementation,
    Testing,
    Maintenance,
    Unknown,
}

impl fmt::Display for ProjectPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Ideation => "ideation",
            Self::Design => "design",
            Self::Implementation => "implementation",
            Self::Testing => "testing",
            Self::Maintenance => "maintenance",
            Self::Unknown => "unknown",
        };
        f.write_str(s)
    }
}

impl FromStr for ProjectPhase {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ideation" => Ok(Self::Ideation),
            "design" => Ok(Self::Design),
            "implementation" => Ok(Self::Implementation),
            "testing" => Ok(Self::Testing),
            "maintenance" => Ok(Self::Maintenance),
            "unknown" => Ok(Self::Unknown),
            _ => Err(format!("unknown phase: {s}")),
        }
    }
}

/// Full project metadata, stored in `meta.json`.
///
/// This is the authoritative source for all project data.
/// `RegistryEntry` is a denormalized subset used for listing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub id: String,
    pub name: String,
    pub description: String,
    pub path: String,
    pub status: ProjectStatus,
    pub phase: ProjectPhase,
    pub created_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_priority")]
    pub priority: String,
    #[serde(default)]
    pub cc_session_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cc_model: String,
}

fn default_priority() -> String {
    "medium".to_string()
}

/// Denormalized project entry in `registry.json`.
///
/// A read-optimized subset of `ProjectMeta` used by `dcal list`.
/// When meta.json and registry.json diverge, meta.json wins.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub id: String,
    pub name: String,
    pub path: String,
    pub status: ProjectStatus,
    pub created_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
}

impl From<&ProjectMeta> for RegistryEntry {
    fn from(meta: &ProjectMeta) -> Self {
        Self {
            id: meta.id.clone(),
            name: meta.name.clone(),
            path: meta.path.clone(),
            status: meta.status,
            created_at: meta.created_at,
            last_active_at: meta.last_active_at,
        }
    }
}

/// A single session record, stored in `sessions.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionEntry {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub ended_at: DateTime<Utc>,
    pub summary: String,
    pub next_task: String,
    #[serde(default)]
    pub open_questions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub human_note: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_timestamp() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 14, 9, 23, 0).unwrap()
    }

    fn sample_meta() -> ProjectMeta {
        ProjectMeta {
            id: "proj_7f3a2c".to_string(),
            name: "invoice-parser".to_string(),
            description: "CLI tool that extracts line items from PDF invoices".to_string(),
            path: "~/projects/invoice-parser".to_string(),
            status: ProjectStatus::Paused,
            phase: ProjectPhase::Implementation,
            created_at: sample_timestamp(),
            last_active_at: Utc.with_ymd_and_hms(2026, 2, 28, 16, 45, 0).unwrap(),
            blocked_reason: None,
            tags: vec!["python".into(), "cli".into(), "finance".into()],
            priority: "medium".to_string(),
            cc_session_ids: vec![],
            cc_model: String::new(),
        }
    }

    // -- ProjectStatus tests --

    #[test]
    fn status_display() {
        assert_eq!(ProjectStatus::Active.to_string(), "active");
        assert_eq!(ProjectStatus::Paused.to_string(), "paused");
        assert_eq!(ProjectStatus::Ideation.to_string(), "ideation");
    }

    #[test]
    fn status_from_str_case_insensitive() {
        assert_eq!("Active".parse::<ProjectStatus>(), Ok(ProjectStatus::Active));
        assert_eq!("PAUSED".parse::<ProjectStatus>(), Ok(ProjectStatus::Paused));
        assert_eq!("blocked".parse::<ProjectStatus>(), Ok(ProjectStatus::Blocked));
        assert!("invalid".parse::<ProjectStatus>().is_err());
    }

    #[test]
    fn status_serde_roundtrip() {
        for status in [
            ProjectStatus::Ideation,
            ProjectStatus::Active,
            ProjectStatus::Paused,
            ProjectStatus::Blocked,
            ProjectStatus::Completed,
            ProjectStatus::Archived,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: ProjectStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, parsed);
        }
    }

    #[test]
    fn status_serializes_lowercase() {
        let json = serde_json::to_string(&ProjectStatus::Active).unwrap();
        assert_eq!(json, r#""active""#);
    }

    // -- ProjectPhase tests --

    #[test]
    fn phase_display() {
        assert_eq!(ProjectPhase::Design.to_string(), "design");
        assert_eq!(ProjectPhase::Implementation.to_string(), "implementation");
        assert_eq!(ProjectPhase::Unknown.to_string(), "unknown");
    }

    #[test]
    fn phase_from_str_case_insensitive() {
        assert_eq!("Design".parse::<ProjectPhase>(), Ok(ProjectPhase::Design));
        assert_eq!("TESTING".parse::<ProjectPhase>(), Ok(ProjectPhase::Testing));
        assert_eq!("unknown".parse::<ProjectPhase>(), Ok(ProjectPhase::Unknown));
        assert!("nonexistent".parse::<ProjectPhase>().is_err());
    }

    #[test]
    fn phase_serde_roundtrip() {
        for phase in [
            ProjectPhase::Ideation,
            ProjectPhase::Design,
            ProjectPhase::Implementation,
            ProjectPhase::Testing,
            ProjectPhase::Maintenance,
            ProjectPhase::Unknown,
        ] {
            let json = serde_json::to_string(&phase).unwrap();
            let parsed: ProjectPhase = serde_json::from_str(&json).unwrap();
            assert_eq!(phase, parsed);
        }
    }

    // -- ProjectMeta tests --

    #[test]
    fn meta_serde_roundtrip() {
        let meta = sample_meta();
        let json = serde_json::to_string_pretty(&meta).unwrap();
        let parsed: ProjectMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(meta, parsed);
    }

    #[test]
    fn meta_matches_spec_format() {
        let meta = sample_meta();
        let value: serde_json::Value = serde_json::to_value(&meta).unwrap();

        assert_eq!(value["id"], "proj_7f3a2c");
        assert_eq!(value["status"], "paused");
        assert_eq!(value["phase"], "implementation");
        assert!(value["blocked_reason"].is_null());
        assert!(value["tags"].is_array());
    }

    #[test]
    fn meta_skips_none_blocked_reason() {
        let meta = sample_meta();
        let json = serde_json::to_string(&meta).unwrap();
        assert!(!json.contains("blocked_reason"));
    }

    #[test]
    fn meta_includes_blocked_reason_when_set() {
        let mut meta = sample_meta();
        meta.blocked_reason = Some("waiting on API access".to_string());
        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("blocked_reason"));
        assert!(json.contains("waiting on API access"));
    }

    #[test]
    fn meta_defaults_for_missing_optional_fields() {
        let json = r#"{
            "id": "proj_abc123",
            "name": "test",
            "description": "a test project",
            "path": "/tmp/test",
            "status": "active",
            "phase": "ideation",
            "created_at": "2026-01-14T09:23:00Z",
            "last_active_at": "2026-01-14T09:23:00Z"
        }"#;
        let meta: ProjectMeta = serde_json::from_str(json).unwrap();
        assert!(meta.tags.is_empty());
        assert!(meta.cc_session_ids.is_empty());
        assert_eq!(meta.priority, "medium");
        assert!(meta.blocked_reason.is_none());
    }

    // -- RegistryEntry tests --

    #[test]
    fn registry_entry_serde_roundtrip() {
        let entry = RegistryEntry {
            id: "proj_7f3a2c".to_string(),
            name: "invoice-parser".to_string(),
            path: "~/projects/invoice-parser".to_string(),
            status: ProjectStatus::Paused,
            created_at: sample_timestamp(),
            last_active_at: Utc.with_ymd_and_hms(2026, 2, 28, 16, 45, 0).unwrap(),
        };
        let json = serde_json::to_string_pretty(&entry).unwrap();
        let parsed: RegistryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, parsed);
    }

    #[test]
    fn registry_entry_from_meta() {
        let meta = sample_meta();
        let entry = RegistryEntry::from(&meta);
        assert_eq!(entry.id, meta.id);
        assert_eq!(entry.name, meta.name);
        assert_eq!(entry.path, meta.path);
        assert_eq!(entry.status, meta.status);
        assert_eq!(entry.created_at, meta.created_at);
        assert_eq!(entry.last_active_at, meta.last_active_at);
    }

    // -- SessionEntry tests --

    #[test]
    fn session_entry_serde_roundtrip() {
        let entry = SessionEntry {
            id: "sess_a1b2c3".to_string(),
            session_id: Some("abc123".to_string()),
            ended_at: Utc.with_ymd_and_hms(2026, 2, 28, 16, 45, 0).unwrap(),
            summary: "Implemented basic PDF text extraction.".to_string(),
            next_task: "Implement table detection.".to_string(),
            open_questions: vec!["Should we support OCR?".to_string()],
            human_note: None,
        };
        let json = serde_json::to_string_pretty(&entry).unwrap();
        let parsed: SessionEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, parsed);
    }

    #[test]
    fn session_entry_manual_mode_null_session_id() {
        let entry = SessionEntry {
            id: "sess_d4e5f6".to_string(),
            session_id: None,
            ended_at: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
            summary: "Manual checkin after hook failure.".to_string(),
            next_task: "Fix the hook timeout.".to_string(),
            open_questions: vec![],
            human_note: Some("Hook timed out, entered manually.".to_string()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(!json.contains("session_id"));

        let parsed: SessionEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, parsed);
    }
}
