use serde::{Deserialize, Serialize};

/// Top-level personal configuration, stored in `config.yml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: String,

    #[serde(default)]
    pub personal: PersonalConfig,

    #[serde(default)]
    pub preferences: Preferences,

    #[serde(default)]
    pub defaults: ProjectDefaults,

    #[serde(default)]
    pub claude_md: ClaudeMdConfig,

    #[serde(default)]
    pub journal: JournalConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: default_version(),
            personal: PersonalConfig::default(),
            preferences: Preferences::default(),
            defaults: ProjectDefaults::default(),
            claude_md: ClaudeMdConfig::default(),
            journal: JournalConfig::default(),
        }
    }
}

/// User identity fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonalConfig {
    #[serde(default)]
    pub name: String,

    #[serde(default = "default_timezone")]
    pub timezone: String,

    #[serde(default)]
    pub github: String,
}

impl Default for PersonalConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            timezone: default_timezone(),
            github: String::new(),
        }
    }
}

/// Development preferences injected into generated CLAUDE.md files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preferences {
    #[serde(default)]
    pub language_primary: String,

    #[serde(default)]
    pub language_secondary: String,

    #[serde(default)]
    pub css_framework: String,

    #[serde(default)]
    pub testing_philosophy: String,

    #[serde(default = "default_commit_style")]
    pub commit_style: String,

    #[serde(default)]
    pub error_handling: String,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            language_primary: String::new(),
            language_secondary: String::new(),
            css_framework: String::new(),
            testing_philosophy: String::new(),
            commit_style: default_commit_style(),
            error_handling: String::new(),
        }
    }
}

/// Defaults applied when creating new projects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectDefaults {
    #[serde(default = "default_license")]
    pub license: String,

    #[serde(default = "default_true")]
    pub git_init: bool,

    #[serde(default = "default_true")]
    pub open_after_create: bool,
}

impl Default for ProjectDefaults {
    fn default() -> Self {
        Self {
            license: default_license(),
            git_init: true,
            open_after_create: true,
        }
    }
}

/// Configuration for CLAUDE.md generation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ClaudeMdConfig {
    #[serde(default)]
    pub personal_context: String,
}

/// Configuration for session journaling behavior.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JournalConfig {
    #[serde(default = "default_true")]
    pub auto_checkin: bool,

    #[serde(default = "default_true")]
    pub prompt_for_human_note: bool,
}

impl Default for JournalConfig {
    fn default() -> Self {
        Self {
            auto_checkin: true,
            prompt_for_human_note: true,
        }
    }
}

fn default_version() -> String {
    "1.0".to_string()
}

fn default_timezone() -> String {
    "UTC".to_string()
}

fn default_commit_style() -> String {
    "conventional".to_string()
}

fn default_license() -> String {
    "MIT".to_string()
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_expected_values() {
        let config = Config::default();
        assert_eq!(config.version, "1.0");
        assert_eq!(config.personal.timezone, "UTC");
        assert_eq!(config.preferences.commit_style, "conventional");
        assert_eq!(config.defaults.license, "MIT");
        assert!(config.defaults.git_init);
        assert!(config.defaults.open_after_create);
        assert!(config.journal.auto_checkin);
        assert!(config.journal.prompt_for_human_note);
    }

    #[test]
    fn yaml_roundtrip() {
        let config = Config::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        let parsed: Config = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config, parsed);
    }

    #[test]
    fn yaml_with_populated_fields() {
        let config = Config {
            version: "1.0".to_string(),
            personal: PersonalConfig {
                name: "Alice".to_string(),
                timezone: "America/New_York".to_string(),
                github: "alice".to_string(),
            },
            preferences: Preferences {
                language_primary: "rust".to_string(),
                language_secondary: "python".to_string(),
                css_framework: "tailwind".to_string(),
                testing_philosophy: "integration-first".to_string(),
                commit_style: "conventional".to_string(),
                error_handling: "anyhow + thiserror".to_string(),
            },
            defaults: ProjectDefaults {
                license: "Apache-2.0".to_string(),
                git_init: true,
                open_after_create: false,
            },
            claude_md: ClaudeMdConfig {
                personal_context: "Always use async Rust.\n".to_string(),
            },
            journal: JournalConfig {
                auto_checkin: true,
                prompt_for_human_note: false,
            },
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        let parsed: Config = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config, parsed);
    }

    #[test]
    fn empty_yaml_produces_defaults() {
        let parsed: Config = serde_yaml::from_str("{}").unwrap();
        let default = Config::default();
        assert_eq!(parsed, default);
    }

    #[test]
    fn partial_yaml_fills_defaults() {
        let yaml = r#"
personal:
  name: "Bob"
defaults:
  license: "GPL-3.0"
"#;
        let parsed: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(parsed.personal.name, "Bob");
        assert_eq!(parsed.personal.timezone, "UTC");
        assert_eq!(parsed.defaults.license, "GPL-3.0");
        assert!(parsed.defaults.git_init);
        assert_eq!(parsed.preferences.commit_style, "conventional");
    }

    #[test]
    fn yaml_matches_spec_format() {
        let yaml = r#"
version: "1.0"

personal:
  name: "Your Name"
  timezone: "UTC"
  github: ""

preferences:
  language_primary: ""
  language_secondary: ""
  css_framework: ""
  testing_philosophy: ""
  commit_style: conventional
  error_handling: ""

defaults:
  license: MIT
  git_init: true
  open_after_create: true

claude_md:
  personal_context: ""

journal:
  auto_checkin: true
  prompt_for_human_note: true
"#;
        let parsed: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(parsed.version, "1.0");
        assert_eq!(parsed.personal.name, "Your Name");
        assert!(parsed.journal.auto_checkin);
    }
}
