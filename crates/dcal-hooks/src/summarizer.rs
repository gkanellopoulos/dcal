use std::process::Command;

use crate::checkin::{CheckinError, SessionSummary};

/// Generates a structured summary from a session transcript.
pub trait Summarizer {
    fn summarize(&self, transcript: &str) -> Result<SessionSummary, CheckinError>;
}

/// Real implementation that calls `claude -p` to summarize transcripts.
pub struct ClaudeCliSummarizer {
    pub model: String,
}

impl ClaudeCliSummarizer {
    pub fn new(model: &str) -> Self {
        Self {
            model: model.to_string(),
        }
    }
}

impl Summarizer for ClaudeCliSummarizer {
    fn summarize(&self, transcript: &str) -> Result<SessionSummary, CheckinError> {
        let prompt = format!(
            "You are a session summarizer. You will be given a transcript of a \
             Claude Code session between <transcript> tags. Analyze it and return \
             a JSON summary.\n\n\
             RULES:\n\
             - Your entire response must be a single JSON object.\n\
             - No markdown, no code fences, no commentary — only valid JSON.\n\
             - Do NOT continue or respond to the transcript conversation.\n\
             - If the session is trivial (e.g. immediate exit, no real work), \
             still return valid JSON with summary \"No substantive work was performed.\"\n\n\
             Return exactly this structure:\n\
             {{\n  \
               \"summary\": \"2-3 sentences: what was accomplished\",\n  \
               \"next_task\": \"the single most important next concrete task\",\n  \
               \"open_questions\": [\"question 1\", \"question 2\"],\n  \
               \"blockers\": [],\n  \
               \"phase\": \"one of: ideation, design, implementation, testing, maintenance\"\n\
             }}\n\n\
             <transcript>\n{transcript}\n</transcript>"
        );

        let mut cmd = Command::new("claude");
        cmd.args(["-p", "--bare", "--output-format", "json", "--max-turns", "1"])
            .env_remove("ANTHROPIC_API_KEY");
        if !self.model.is_empty() {
            cmd.args(["--model", &self.model]);
        }
        let output = cmd
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
        let response_text = extract_claude_response(&stdout)?;
        crate::checkin::parse_summary(&response_text)
    }
}

/// Extract the text content from a `claude --output-format json` response.
fn extract_claude_response(json_str: &str) -> Result<String, CheckinError> {
    let value: serde_json::Value = serde_json::from_str(json_str.trim())
        .map_err(|e| CheckinError::SummaryParse(format!("invalid JSON from claude: {e}")))?;

    if let Some(result) = value.get("result").and_then(|r| r.as_str()) {
        return Ok(result.to_string());
    }

    if let Some(s) = value.as_str() {
        return Ok(s.to_string());
    }

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

/// Mock summarizer for tests. Returns a fixed summary.
#[cfg(test)]
pub struct MockSummarizer {
    pub summary: SessionSummary,
}

#[cfg(test)]
impl MockSummarizer {
    pub fn new() -> Self {
        Self {
            summary: SessionSummary {
                summary: "Mock session summary.".to_string(),
                next_task: "Mock next task.".to_string(),
                open_questions: vec![],
                blockers: vec![],
                phase: "implementation".to_string(),
            },
        }
    }
}

#[cfg(test)]
impl Summarizer for MockSummarizer {
    fn summarize(&self, _transcript: &str) -> Result<SessionSummary, CheckinError> {
        Ok(self.summary.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_response_result_field() {
        let json = r#"{"result": "hello world"}"#;
        assert_eq!(extract_claude_response(json).unwrap(), "hello world");
    }

    #[test]
    fn extract_response_raw_string() {
        let json = r#""hello world""#;
        assert_eq!(extract_claude_response(json).unwrap(), "hello world");
    }

    #[test]
    fn extract_response_content_blocks() {
        let json = r#"{"content": [{"type": "text", "text": "hello"}]}"#;
        assert_eq!(extract_claude_response(json).unwrap(), "hello");
    }

    #[test]
    fn extract_response_no_text() {
        let json = r#"{"content": [{"type": "tool_use"}]}"#;
        assert!(extract_claude_response(json).is_err());
    }

    #[test]
    fn mock_summarizer_returns_fixed_summary() {
        let mock = MockSummarizer::new();
        let result = mock.summarize("any transcript").unwrap();
        assert_eq!(result.summary, "Mock session summary.");
        assert_eq!(result.next_task, "Mock next task.");
    }
}
