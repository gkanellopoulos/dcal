use dcal_config::model::Config;
use serde::{Deserialize, Serialize};

use crate::intake::ProjectBrief;

/// Fully resolved project specification, ready for CLAUDE.md generation.
///
/// Merges the LLM-extracted ProjectBrief with the user's personal config.
/// Each field tracks its provenance (brief vs config vs default).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedSpec {
    pub name: String,
    pub domain: String,
    pub goals: Vec<String>,
    pub risks: Vec<String>,
    pub constraints: Vec<String>,
    pub language_primary: String,
    pub language_secondary: String,
    pub css_framework: String,
    pub testing_philosophy: String,
    pub commit_style: String,
    pub error_handling: String,
    pub license: String,
    pub personal_context: String,
}

/// Run Stage 2: merge a ProjectBrief with personal config into a ResolvedSpec.
///
/// Config values take precedence when set. Brief values fill in gaps.
pub fn run(brief: &ProjectBrief, config: &Config) -> ResolvedSpec {
    let language_primary = if !config.preferences.language_primary.is_empty() {
        config.preferences.language_primary.clone()
    } else {
        brief.stack_hints.first().cloned().unwrap_or_default()
    };

    let language_secondary = if !config.preferences.language_secondary.is_empty() {
        config.preferences.language_secondary.clone()
    } else {
        brief.stack_hints.get(1).cloned().unwrap_or_default()
    };

    ResolvedSpec {
        name: brief.name.clone(),
        domain: brief.domain.clone(),
        goals: brief.goals.clone(),
        risks: brief.risks.clone(),
        constraints: brief.constraints.clone(),
        language_primary,
        language_secondary,
        css_framework: config.preferences.css_framework.clone(),
        testing_philosophy: config.preferences.testing_philosophy.clone(),
        commit_style: config.preferences.commit_style.clone(),
        error_handling: config.preferences.error_handling.clone(),
        license: config.defaults.license.clone(),
        personal_context: config.claude_md.personal_context.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dcal_config::model::{ClaudeMdConfig, Config, Preferences};

    fn sample_brief() -> ProjectBrief {
        ProjectBrief {
            name: "myapp".to_string(),
            domain: "web scraping".to_string(),
            stack_hints: vec!["python".to_string(), "beautifulsoup".to_string()],
            constraints: vec!["must work offline".to_string()],
            goals: vec!["scrape product prices".to_string()],
            risks: vec!["site structure changes".to_string()],
        }
    }

    #[test]
    fn resolve_uses_config_language_when_set() {
        let brief = sample_brief();
        let config = Config {
            preferences: Preferences {
                language_primary: "rust".to_string(),
                ..Preferences::default()
            },
            ..Config::default()
        };

        let spec = run(&brief, &config);
        assert_eq!(spec.language_primary, "rust");
    }

    #[test]
    fn resolve_falls_back_to_brief_stack_hints() {
        let brief = sample_brief();
        let config = Config::default();

        let spec = run(&brief, &config);
        assert_eq!(spec.language_primary, "python");
        assert_eq!(spec.language_secondary, "beautifulsoup");
    }

    #[test]
    fn resolve_carries_brief_fields() {
        let brief = sample_brief();
        let config = Config::default();

        let spec = run(&brief, &config);
        assert_eq!(spec.name, "myapp");
        assert_eq!(spec.domain, "web scraping");
        assert_eq!(spec.goals, vec!["scrape product prices"]);
        assert_eq!(spec.constraints, vec!["must work offline"]);
    }

    #[test]
    fn resolve_includes_personal_context() {
        let brief = sample_brief();
        let config = Config {
            claude_md: ClaudeMdConfig {
                personal_context: "Always use async.".to_string(),
            },
            ..Config::default()
        };

        let spec = run(&brief, &config);
        assert_eq!(spec.personal_context, "Always use async.");
    }

    #[test]
    fn resolve_uses_config_defaults() {
        let brief = sample_brief();
        let config = Config::default();

        let spec = run(&brief, &config);
        assert_eq!(spec.commit_style, "conventional");
        assert_eq!(spec.license, "MIT");
    }

    #[test]
    fn resolve_empty_brief_stack_hints() {
        let brief = ProjectBrief {
            name: "minimal".to_string(),
            domain: "unknown".to_string(),
            stack_hints: vec![],
            constraints: vec![],
            goals: vec![],
            risks: vec![],
        };
        let config = Config::default();

        let spec = run(&brief, &config);
        assert_eq!(spec.language_primary, "");
        assert_eq!(spec.language_secondary, "");
    }
}
