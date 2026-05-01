use std::fs;
use std::path::Path;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TranscriptError {
    #[error("failed to read transcript at {path}: {source}")]
    Read { path: String, source: std::io::Error },

    #[error("failed to parse transcript line: {0}")]
    Parse(String),
}

const MAX_CONTENT_BYTES: usize = 100_000;
const MAX_TURNS: usize = 50;

/// Read a JSONL transcript file and extract conversation content.
///
/// Returns the extracted text, truncated to the last `MAX_TURNS` turns
/// and `MAX_CONTENT_BYTES` to stay within context limits.
pub fn read_transcript(path: &Path) -> Result<String, TranscriptError> {
    let content = fs::read_to_string(path).map_err(|source| TranscriptError::Read {
        path: path.display().to_string(),
        source,
    })?;

    let lines: Vec<&str> = content.lines().collect();
    extract_content(&lines)
}

fn extract_content(lines: &[&str]) -> Result<String, TranscriptError> {
    let mut turns = Vec::new();

    // Take the last MAX_TURNS lines
    let start = lines.len().saturating_sub(MAX_TURNS);
    for line in &lines[start..] {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let value: serde_json::Value = serde_json::from_str(trimmed)
            .map_err(|e| TranscriptError::Parse(format!("{e}: {trimmed}")))?;

        if let Some(text) = extract_text_from_entry(&value) {
            turns.push(text);
        }
    }

    let mut result = turns.join("\n\n");

    // Truncate to MAX_CONTENT_BYTES
    if result.len() > MAX_CONTENT_BYTES {
        result.truncate(MAX_CONTENT_BYTES);
        if let Some(last_newline) = result.rfind('\n') {
            result.truncate(last_newline);
        }
        result.push_str("\n\n[transcript truncated]");
    }

    Ok(result)
}

fn extract_text_from_entry(value: &serde_json::Value) -> Option<String> {
    let role = value.get("role").and_then(|r| r.as_str())?;

    // Handle different content formats
    let text = if let Some(content) = value.get("message").and_then(|m| m.get("content")) {
        extract_text_from_content(content)
    } else if let Some(content) = value.get("content") {
        extract_text_from_content(content)
    } else {
        None
    };

    text.map(|t| format!("[{role}]: {t}"))
}

fn extract_text_from_content(content: &serde_json::Value) -> Option<String> {
    // Content can be a string or an array of blocks
    if let Some(s) = content.as_str() {
        if s.is_empty() {
            return None;
        }
        return Some(s.to_string());
    }

    if let Some(arr) = content.as_array() {
        let texts: Vec<String> = arr
            .iter()
            .filter_map(|block| {
                if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                    block.get("text").and_then(|t| t.as_str()).map(String::from)
                } else {
                    None
                }
            })
            .collect();

        if texts.is_empty() {
            return None;
        }
        return Some(texts.join("\n"));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn extract_text_string_content() {
        let entry = serde_json::json!({
            "role": "user",
            "content": "Hello, world!"
        });
        let result = extract_text_from_entry(&entry);
        assert_eq!(result, Some("[user]: Hello, world!".to_string()));
    }

    #[test]
    fn extract_text_array_content() {
        let entry = serde_json::json!({
            "role": "assistant",
            "message": {
                "content": [
                    {"type": "text", "text": "Here is the answer."},
                    {"type": "tool_use", "name": "read_file"}
                ]
            }
        });
        let result = extract_text_from_entry(&entry);
        assert_eq!(result, Some("[assistant]: Here is the answer.".to_string()));
    }

    #[test]
    fn extract_text_no_role_returns_none() {
        let entry = serde_json::json!({"content": "no role"});
        assert!(extract_text_from_entry(&entry).is_none());
    }

    #[test]
    fn extract_text_tool_only_returns_none() {
        let entry = serde_json::json!({
            "role": "assistant",
            "message": {
                "content": [
                    {"type": "tool_use", "name": "bash"}
                ]
            }
        });
        assert!(extract_text_from_entry(&entry).is_none());
    }

    #[test]
    fn extract_content_from_lines() {
        let lines = vec![
            r#"{"role": "user", "content": "What is 2+2?"}"#,
            r#"{"role": "assistant", "message": {"content": [{"type": "text", "text": "4"}]}}"#,
        ];
        let result = extract_content(&lines).unwrap();
        assert!(result.contains("[user]: What is 2+2?"));
        assert!(result.contains("[assistant]: 4"));
    }

    #[test]
    fn extract_content_skips_empty_lines() {
        let lines = vec![
            r#"{"role": "user", "content": "hello"}"#,
            "",
            r#"{"role": "assistant", "message": {"content": [{"type": "text", "text": "hi"}]}}"#,
        ];
        let result = extract_content(&lines).unwrap();
        assert!(result.contains("[user]: hello"));
        assert!(result.contains("[assistant]: hi"));
    }

    #[test]
    fn read_transcript_from_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("transcript.jsonl");
        let content = r#"{"role": "user", "content": "test input"}
{"role": "assistant", "message": {"content": [{"type": "text", "text": "test output"}]}}"#;
        fs::write(&path, content).unwrap();

        let result = read_transcript(&path).unwrap();
        assert!(result.contains("[user]: test input"));
        assert!(result.contains("[assistant]: test output"));
    }

    #[test]
    fn read_transcript_missing_file() {
        let result = read_transcript(Path::new("/nonexistent/transcript.jsonl"));
        assert!(result.is_err());
    }

    #[test]
    fn truncation_works() {
        let mut lines = Vec::new();
        for i in 0..100 {
            lines.push(format!(
                r#"{{"role": "user", "content": "Message number {i} with some padding text to make it longer"}}"#
            ));
        }
        let line_refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        let result = extract_content(&line_refs).unwrap();

        // Should only have last MAX_TURNS entries
        assert!(!result.contains("Message number 0 "));
        assert!(result.contains("Message number 99 "));
    }
}
