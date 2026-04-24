use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::client::{AnthropicClient, ApiError, ApiRequest, Message};

#[derive(Debug, Error)]
pub enum IntakeError {
    #[error("API call failed: {0}")]
    Api(#[from] ApiError),

    #[error("failed to parse project brief: {0}")]
    Parse(String),
}

/// Structured output from the intake stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectBrief {
    pub name: String,
    pub domain: String,
    pub stack_hints: Vec<String>,
    pub constraints: Vec<String>,
    pub goals: Vec<String>,
    pub risks: Vec<String>,
}

const INTAKE_MODEL: &str = "claude-haiku-4-5";
const INTAKE_MAX_TOKENS: u32 = 1024;

const INTAKE_SYSTEM: &str = r#"You are a project intake assistant. Given a raw project idea, extract structured information.
Respond with JSON only, no other text.

{
  "name": "kebab-case project name",
  "domain": "short domain description (e.g. 'web scraping', 'CLI tooling')",
  "stack_hints": ["languages or frameworks mentioned or implied"],
  "constraints": ["any constraints mentioned"],
  "goals": ["primary goals of the project"],
  "risks": ["potential risks or challenges"]
}"#;

/// Run Stage 1: extract a structured ProjectBrief from raw idea text.
pub async fn run<C: AnthropicClient>(client: &C, idea: &str) -> Result<ProjectBrief, IntakeError> {
    let request = ApiRequest {
        model: INTAKE_MODEL.to_string(),
        system: Some(INTAKE_SYSTEM.to_string()),
        messages: vec![Message {
            role: "user".to_string(),
            content: idea.to_string(),
        }],
        max_tokens: INTAKE_MAX_TOKENS,
    };

    let response = client.send(request).await?;

    // Try to extract JSON from the response, handling markdown code fences
    let json_str = extract_json(&response.content);

    serde_json::from_str(json_str)
        .map_err(|e| IntakeError::Parse(format!("{e}: {json_str}")))
}

/// Strip markdown code fences if present.
fn extract_json(content: &str) -> &str {
    let trimmed = content.trim();
    if let Some(rest) = trimmed.strip_prefix("```json") {
        let inner = rest.trim();
        inner.strip_suffix("```").map(str::trim).unwrap_or(inner)
    } else if let Some(rest) = trimmed.strip_prefix("```") {
        let inner = rest.trim();
        inner.strip_suffix("```").map(str::trim).unwrap_or(inner)
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::mock::MockClient;

    const SAMPLE_BRIEF: &str = r#"{
        "name": "invoice-parser",
        "domain": "document processing",
        "stack_hints": ["rust", "pdf"],
        "constraints": ["must work offline"],
        "goals": ["extract line items from PDF invoices"],
        "risks": ["PDF format complexity"]
    }"#;

    #[tokio::test]
    async fn intake_parses_brief() {
        let client = MockClient::with_response(SAMPLE_BRIEF);
        let brief = run(&client, "Build a tool to parse invoices from PDF files").await.unwrap();

        assert_eq!(brief.name, "invoice-parser");
        assert_eq!(brief.domain, "document processing");
        assert_eq!(brief.stack_hints, vec!["rust", "pdf"]);
        assert!(!brief.goals.is_empty());
    }

    #[tokio::test]
    async fn intake_handles_code_fenced_json() {
        let fenced = format!("```json\n{SAMPLE_BRIEF}\n```");
        let client = MockClient::with_response(&fenced);
        let brief = run(&client, "some idea").await.unwrap();
        assert_eq!(brief.name, "invoice-parser");
    }

    #[tokio::test]
    async fn intake_api_error_propagates() {
        let client = MockClient::new(vec![
            Err(ApiError::MissingApiKey),
        ]);
        let result = run(&client, "some idea").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn intake_bad_json_gives_parse_error() {
        let client = MockClient::with_response("this is not json");
        let result = run(&client, "some idea").await;
        assert!(matches!(result, Err(IntakeError::Parse(_))));
    }

    #[test]
    fn extract_json_plain() {
        let input = r#"{"name": "test"}"#;
        assert_eq!(extract_json(input), input);
    }

    #[test]
    fn extract_json_fenced() {
        let input = "```json\n{\"name\": \"test\"}\n```";
        assert_eq!(extract_json(input), "{\"name\": \"test\"}");
    }
}
