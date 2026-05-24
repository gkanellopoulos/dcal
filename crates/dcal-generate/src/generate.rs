use thiserror::Error;

use crate::client::{AnthropicClient, ApiError, ApiRequest, Message};
use crate::resolve::ResolvedSpec;

#[derive(Debug, Error)]
pub enum GenerateError {
    #[error("API call failed: {0}")]
    Api(#[from] ApiError),
}

const GENERATE_MODEL: &str = "claude-sonnet-4-5";
const GENERATE_MAX_TOKENS: u32 = 2048;

/// Run Stage 3: generate a CLAUDE.md from a ResolvedSpec.
///
/// `model_override` replaces the default model when non-empty.
pub async fn run<C: AnthropicClient>(
    client: &C,
    spec: &ResolvedSpec,
    model_override: &str,
) -> Result<String, GenerateError> {
    let system = build_system_prompt(spec);
    let user_message = build_user_message(spec);

    let model = if model_override.is_empty() {
        GENERATE_MODEL
    } else {
        model_override
    };
    let request = ApiRequest {
        model: model.to_string(),
        system: Some(system),
        messages: vec![Message {
            role: "user".to_string(),
            content: user_message,
        }],
        max_tokens: GENERATE_MAX_TOKENS,
    };

    let response = client.send(request).await?;
    Ok(strip_code_fences(&response.content))
}

fn build_system_prompt(spec: &ResolvedSpec) -> String {
    let mut prompt = String::from(
        "You are a technical writer creating a CLAUDE.md file for a software project. \
         CLAUDE.md is the primary instruction file that guides Claude Code sessions.\n\n\
         Requirements:\n\
         1. Open with a one-paragraph project description\n\
         2. Include these sections: ## Goals, ## Stack, ## Architecture, \
            ## Working Conventions, ## Current Phase, ## Open Questions, ## Do Not Do\n\
         3. Use imperative language (\"Always\", \"Never\", \"When X, do Y\")\n\
         4. Be concise — target 300-500 words, no padding\n\
         5. Output raw markdown only — no code fences around the whole document\n",
    );

    if !spec.personal_context.is_empty() {
        prompt.push_str("\nPersonal context to incorporate:\n");
        prompt.push_str(&spec.personal_context);
        prompt.push('\n');
    }

    prompt
}

fn build_user_message(spec: &ResolvedSpec) -> String {
    let mut msg = format!(
        "Generate a CLAUDE.md for the project \"{name}\".\n\n\
         Domain: {domain}\n",
        name = spec.name,
        domain = spec.domain,
    );

    if !spec.goals.is_empty() {
        msg.push_str("\nGoals:\n");
        for goal in &spec.goals {
            msg.push_str(&format!("- {goal}\n"));
        }
    }

    if !spec.risks.is_empty() {
        msg.push_str("\nRisks:\n");
        for risk in &spec.risks {
            msg.push_str(&format!("- {risk}\n"));
        }
    }

    if !spec.constraints.is_empty() {
        msg.push_str("\nConstraints:\n");
        for c in &spec.constraints {
            msg.push_str(&format!("- {c}\n"));
        }
    }

    if !spec.language_primary.is_empty() {
        msg.push_str(&format!("\nPrimary language: {}\n", spec.language_primary));
    }
    if !spec.language_secondary.is_empty() {
        msg.push_str(&format!("Secondary language: {}\n", spec.language_secondary));
    }
    if !spec.css_framework.is_empty() {
        msg.push_str(&format!("CSS framework: {}\n", spec.css_framework));
    }
    if !spec.testing_philosophy.is_empty() {
        msg.push_str(&format!("Testing approach: {}\n", spec.testing_philosophy));
    }
    if !spec.error_handling.is_empty() {
        msg.push_str(&format!("Error handling: {}\n", spec.error_handling));
    }

    msg.push_str(&format!("Commit style: {}\n", spec.commit_style));
    msg.push_str(&format!("License: {}\n", spec.license));

    msg
}

fn strip_code_fences(content: &str) -> String {
    let trimmed = content.trim();
    if let Some(rest) = trimmed.strip_prefix("```markdown") {
        rest.trim()
            .strip_suffix("```")
            .unwrap_or(rest.trim())
            .trim()
            .to_string()
    } else if let Some(rest) = trimmed.strip_prefix("```md") {
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
    use crate::client::mock::MockClient;

    fn sample_spec() -> ResolvedSpec {
        ResolvedSpec {
            name: "invoice-parser".to_string(),
            domain: "document processing".to_string(),
            goals: vec!["extract line items".to_string()],
            risks: vec!["PDF complexity".to_string()],
            constraints: vec!["offline only".to_string()],
            language_primary: "rust".to_string(),
            language_secondary: "".to_string(),
            css_framework: "".to_string(),
            testing_philosophy: "TDD".to_string(),
            commit_style: "conventional".to_string(),
            error_handling: "anyhow".to_string(),
            license: "MIT".to_string(),
            personal_context: "".to_string(),
        }
    }

    const SAMPLE_CLAUDE_MD: &str = "\
# invoice-parser

A CLI tool that extracts line items from PDF invoices for automated processing.

## Goals

- Extract line items from PDF invoices accurately
- Support multiple PDF layouts

## Stack

- **Language:** Rust
- **Key libraries:** pdf-extract, serde

## Architecture

TBD — single binary CLI for v0.1.

## Working Conventions

- Use conventional commits
- Always handle errors with anyhow
- Write tests before implementation (TDD)

## Current Phase

Ideation — defining scope and constraints.

## Open Questions

- Should we support scanned PDFs via OCR?

## Do Not Do

- Do not add a GUI in v0.1
- Never silently drop parsing errors
";

    #[tokio::test]
    async fn generate_returns_claude_md() {
        let client = MockClient::with_response(SAMPLE_CLAUDE_MD);
        let spec = sample_spec();

        let result = run(&client, &spec, "").await.unwrap();
        assert!(result.contains("# invoice-parser"));
        assert!(result.contains("## Goals"));
    }

    #[tokio::test]
    async fn generate_strips_code_fences() {
        let fenced = format!("```markdown\n{SAMPLE_CLAUDE_MD}\n```");
        let client = MockClient::with_response(&fenced);

        let result = run(&client, &sample_spec(), "").await.unwrap();
        assert!(!result.starts_with("```"));
        assert!(result.contains("# invoice-parser"));
    }

    #[tokio::test]
    async fn generate_api_error_propagates() {
        let client = MockClient::new(vec![Err(ApiError::MissingApiKey)]);
        let result = run(&client, &sample_spec(), "").await;
        assert!(result.is_err());
    }

    #[test]
    fn build_user_message_includes_spec_fields() {
        let spec = sample_spec();
        let msg = build_user_message(&spec);

        assert!(msg.contains("invoice-parser"));
        assert!(msg.contains("document processing"));
        assert!(msg.contains("extract line items"));
        assert!(msg.contains("rust"));
        assert!(msg.contains("TDD"));
        assert!(msg.contains("conventional"));
    }

    #[test]
    fn build_system_prompt_includes_personal_context() {
        let mut spec = sample_spec();
        spec.personal_context = "Always use async Rust.".to_string();
        let prompt = build_system_prompt(&spec);
        assert!(prompt.contains("Always use async Rust."));
    }

    #[test]
    fn build_system_prompt_omits_empty_personal_context() {
        let spec = sample_spec();
        let prompt = build_system_prompt(&spec);
        assert!(!prompt.contains("Personal context"));
    }

    #[test]
    fn strip_code_fences_markdown() {
        let input = "```markdown\n# Hello\n```";
        assert_eq!(strip_code_fences(input), "# Hello");
    }

    #[test]
    fn strip_code_fences_md() {
        let input = "```md\n# Hello\n```";
        assert_eq!(strip_code_fences(input), "# Hello");
    }

    #[test]
    fn strip_code_fences_none() {
        let input = "# Hello";
        assert_eq!(strip_code_fences(input), "# Hello");
    }
}
