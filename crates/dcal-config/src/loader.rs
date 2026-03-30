use std::fs;
use std::path::Path;
use thiserror::Error;

use crate::model::Config;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config at {path}: {source}")]
    Read { path: String, source: std::io::Error },

    #[error("failed to write config at {path}: {source}")]
    Write { path: String, source: std::io::Error },

    #[error("failed to parse config at {path}: {source}")]
    Parse { path: String, source: serde_yaml::Error },

    #[error("failed to serialize config: {0}")]
    Serialize(serde_yaml::Error),
}

/// Load config from a YAML file, falling back to defaults.
///
/// If the file doesn't exist, returns `Config::default()`.
/// If the file exists but has missing fields, defaults fill in the gaps.
pub fn load(path: &Path) -> Result<Config, ConfigError> {
    if !path.exists() {
        return Ok(Config::default());
    }

    let content = fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.display().to_string(),
        source,
    })?;

    if content.trim().is_empty() {
        return Ok(Config::default());
    }

    serde_yaml::from_str(&content).map_err(|source| ConfigError::Parse {
        path: path.display().to_string(),
        source,
    })
}

/// Save config to a YAML file.
pub fn save(path: &Path, config: &Config) -> Result<(), ConfigError> {
    let yaml = serde_yaml::to_string(config).map_err(ConfigError::Serialize)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ConfigError::Write {
            path: parent.display().to_string(),
            source,
        })?;
    }

    fs::write(path, yaml).map_err(|source| ConfigError::Write {
        path: path.display().to_string(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PersonalConfig, Preferences};
    use tempfile::TempDir;

    #[test]
    fn load_returns_defaults_when_no_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.yml");
        let config = load(&path).unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn load_returns_defaults_for_empty_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.yml");
        fs::write(&path, "").unwrap();
        let config = load(&path).unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.yml");

        let config = Config {
            personal: PersonalConfig {
                name: "Alice".to_string(),
                timezone: "Europe/London".to_string(),
                github: "alice".to_string(),
            },
            preferences: Preferences {
                language_primary: "rust".to_string(),
                ..Preferences::default()
            },
            ..Config::default()
        };

        save(&path, &config).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(config, loaded);
    }

    #[test]
    fn save_creates_parent_directories() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nested").join("dir").join("config.yml");

        save(&path, &Config::default()).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn load_partial_yaml_fills_defaults() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.yml");

        fs::write(
            &path,
            "personal:\n  name: Bob\ndefaults:\n  license: GPL-3.0\n",
        )
        .unwrap();

        let config = load(&path).unwrap();
        assert_eq!(config.personal.name, "Bob");
        assert_eq!(config.personal.timezone, "UTC");
        assert_eq!(config.defaults.license, "GPL-3.0");
        assert!(config.defaults.git_init);
        assert_eq!(config.preferences.commit_style, "conventional");
    }

    #[test]
    fn load_full_spec_yaml() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.yml");

        let yaml = r#"
version: "1.0"
personal:
  name: "Test User"
  timezone: "US/Pacific"
  github: "testuser"
preferences:
  language_primary: "typescript"
  language_secondary: "python"
  css_framework: "tailwind"
  testing_philosophy: "TDD"
  commit_style: "freeform"
  error_handling: "Result types"
defaults:
  license: "Apache-2.0"
  git_init: false
  open_after_create: false
claude_md:
  personal_context: "Always prefer async patterns.\n"
journal:
  auto_checkin: false
  prompt_for_human_note: false
"#;
        fs::write(&path, yaml).unwrap();

        let config = load(&path).unwrap();
        assert_eq!(config.personal.name, "Test User");
        assert_eq!(config.preferences.css_framework, "tailwind");
        assert!(!config.defaults.git_init);
        assert!(!config.journal.auto_checkin);
        assert_eq!(
            config.claude_md.personal_context,
            "Always prefer async patterns.\n"
        );
    }

    #[test]
    fn load_invalid_yaml_returns_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.yml");
        fs::write(&path, "{{invalid yaml").unwrap();

        let result = load(&path);
        assert!(result.is_err());
    }

    #[test]
    fn save_overwrites_existing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.yml");

        let mut config = Config::default();
        config.personal.name = "First".to_string();
        save(&path, &config).unwrap();

        config.personal.name = "Second".to_string();
        save(&path, &config).unwrap();

        let loaded = load(&path).unwrap();
        assert_eq!(loaded.personal.name, "Second");
    }
}
