use std::fs;
use std::path::Path;
use std::process::Command;

use chrono::{DateTime, Utc};
use thiserror::Error;

use dcal_core::project::RegistryEntry;

#[derive(Debug, Error)]
pub enum ReaderError {
    #[error("failed to read {path}: {source}")]
    Read { path: String, source: std::io::Error },

    #[error("failed to run git command: {0}")]
    Git(String),
}

/// Information extracted from an existing project directory.
#[derive(Debug, Clone)]
pub struct ProjectInfo {
    pub description: Option<String>,
    pub last_commit_date: Option<DateTime<Utc>>,
}

/// Extract the first paragraph from a CLAUDE.md file as the project description.
pub fn read_claude_md_description(project_path: &Path) -> Result<Option<String>, ReaderError> {
    let claude_md = project_path.join("CLAUDE.md");

    if !claude_md.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&claude_md).map_err(|source| ReaderError::Read {
        path: claude_md.display().to_string(),
        source,
    })?;

    Ok(extract_first_paragraph(&content))
}

/// Get the date of the last git commit in the project directory.
pub fn read_last_commit_date(project_path: &Path) -> Result<Option<DateTime<Utc>>, ReaderError> {
    let output = Command::new("git")
        .args(["-C", &project_path.display().to_string(), "log", "-1", "--format=%aI"])
        .output()
        .map_err(|e| ReaderError::Git(e.to_string()))?;

    if !output.status.success() {
        return Ok(None);
    }

    let date_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if date_str.is_empty() {
        return Ok(None);
    }

    DateTime::parse_from_rfc3339(&date_str)
        .map(|dt| Some(dt.with_timezone(&Utc)))
        .map_err(|e| ReaderError::Git(format!("failed to parse git date '{date_str}': {e}")))
}

/// Read all available info from a project directory.
pub fn read_project_info(project_path: &Path) -> Result<ProjectInfo, ReaderError> {
    let description = read_claude_md_description(project_path)?;
    let last_commit_date = read_last_commit_date(project_path)?;

    Ok(ProjectInfo {
        description,
        last_commit_date,
    })
}

/// Check if a project path is already registered.
pub fn is_duplicate(entries: &[RegistryEntry], path: &str) -> bool {
    entries.iter().any(|e| e.path == path)
}

fn extract_first_paragraph(content: &str) -> Option<String> {
    let mut lines = content.lines().peekable();
    let mut paragraph = Vec::new();

    // Skip leading blank lines and heading lines
    while let Some(&line) = lines.peek() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('>') {
            lines.next();
        } else {
            break;
        }
    }

    // Collect lines until the next blank line or heading
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            break;
        }
        paragraph.push(trimmed);
    }

    if paragraph.is_empty() {
        None
    } else {
        Some(paragraph.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn extract_first_paragraph_basic() {
        let content = "# My Project\n\nThis is the description.\nIt spans two lines.\n\n## More stuff\n";
        let result = extract_first_paragraph(content);
        assert_eq!(
            result,
            Some("This is the description. It spans two lines.".to_string())
        );
    }

    #[test]
    fn extract_first_paragraph_skips_blockquote() {
        let content = "# Title\n> some quote\n\nActual description here.\n";
        let result = extract_first_paragraph(content);
        assert_eq!(result, Some("Actual description here.".to_string()));
    }

    #[test]
    fn extract_first_paragraph_empty_content() {
        let result = extract_first_paragraph("");
        assert_eq!(result, None);
    }

    #[test]
    fn extract_first_paragraph_only_headings() {
        let result = extract_first_paragraph("# Heading\n## Another\n");
        assert_eq!(result, None);
    }

    #[test]
    fn read_claude_md_description_no_file() {
        let dir = TempDir::new().unwrap();
        let result = read_claude_md_description(dir.path()).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn read_claude_md_description_with_file() {
        let dir = TempDir::new().unwrap();
        let claude_md = dir.path().join("CLAUDE.md");
        fs::write(&claude_md, "# Project\n\nA CLI tool for invoices.\n").unwrap();

        let result = read_claude_md_description(dir.path()).unwrap();
        assert_eq!(result, Some("A CLI tool for invoices.".to_string()));
    }

    #[test]
    fn read_last_commit_date_no_git() {
        let dir = TempDir::new().unwrap();
        let result = read_last_commit_date(dir.path()).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn read_last_commit_date_with_git() {
        let dir = TempDir::new().unwrap();
        let path = dir.path();

        // Init a repo and make a commit
        Command::new("git").args(["init", &path.display().to_string()]).output().unwrap();
        Command::new("git").args(["-C", &path.display().to_string(), "config", "user.email", "test@test.com"]).output().unwrap();
        Command::new("git").args(["-C", &path.display().to_string(), "config", "user.name", "Test"]).output().unwrap();
        fs::write(path.join("file.txt"), "hello").unwrap();
        Command::new("git").args(["-C", &path.display().to_string(), "add", "."]).output().unwrap();
        Command::new("git").args(["-C", &path.display().to_string(), "commit", "-m", "init"]).output().unwrap();

        let result = read_last_commit_date(path).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn is_duplicate_detects_match() {
        let entries = vec![RegistryEntry {
            id: "proj_aaa111".to_string(),
            name: "myapp".to_string(),
            path: "~/projects/myapp".to_string(),
            status: dcal_core::project::ProjectStatus::Active,
            created_at: Utc::now(),
            last_active_at: Utc::now(),
        }];

        assert!(is_duplicate(&entries, "~/projects/myapp"));
        assert!(!is_duplicate(&entries, "~/projects/other"));
    }
}
