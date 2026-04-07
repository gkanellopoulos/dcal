use std::fs;
use std::path::Path;
use serde_json::{json, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HookInstallError {
    #[error("failed to read {path}: {source}")]
    Read { path: String, source: std::io::Error },

    #[error("failed to write {path}: {source}")]
    Write { path: String, source: std::io::Error },

    #[error("failed to parse {path}: {source}")]
    Parse { path: String, source: serde_json::Error },
}

const DCAL_HOOK_COMMAND: &str = "dcal checkin --auto --project-from-cwd";
const HOOK_TIMEOUT: u64 = 60;

/// Install the dcal SessionEnd hook into a Claude Code settings file.
///
/// Reads the file (or starts from an empty object), merges the dcal hook
/// into the `hooks.SessionEnd` array without overwriting existing hooks,
/// and writes the result back.
///
/// If the dcal hook is already present, this is a no-op.
pub fn install_session_end_hook(settings_path: &Path) -> Result<bool, HookInstallError> {
    let mut settings = load_or_create(settings_path)?;

    if has_dcal_hook(&settings) {
        return Ok(false);
    }

    let dcal_hook_entry = json!({
        "matcher": "other",
        "hooks": [
            {
                "type": "command",
                "command": DCAL_HOOK_COMMAND,
                "timeout": HOOK_TIMEOUT
            }
        ]
    });

    let hooks = settings
        .as_object_mut()
        .expect("settings is an object")
        .entry("hooks")
        .or_insert_with(|| json!({}));

    let session_end = hooks
        .as_object_mut()
        .expect("hooks is an object")
        .entry("SessionEnd")
        .or_insert_with(|| json!([]));

    session_end
        .as_array_mut()
        .expect("SessionEnd is an array")
        .push(dcal_hook_entry);

    save(settings_path, &settings)?;
    Ok(true)
}

/// Check whether the dcal hook is already installed.
fn has_dcal_hook(settings: &Value) -> bool {
    settings
        .get("hooks")
        .and_then(|h| h.get("SessionEnd"))
        .and_then(|se| se.as_array())
        .map(|entries| {
            entries.iter().any(|entry| {
                entry
                    .get("hooks")
                    .and_then(|h| h.as_array())
                    .map(|hooks| {
                        hooks.iter().any(|hook| {
                            hook.get("command")
                                .and_then(|c| c.as_str())
                                .is_some_and(|cmd| cmd.contains("dcal checkin"))
                        })
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn load_or_create(path: &Path) -> Result<Value, HookInstallError> {
    if !path.exists() {
        return Ok(json!({}));
    }

    let content = fs::read_to_string(path).map_err(|source| HookInstallError::Read {
        path: path.display().to_string(),
        source,
    })?;

    if content.trim().is_empty() {
        return Ok(json!({}));
    }

    serde_json::from_str(&content).map_err(|source| HookInstallError::Parse {
        path: path.display().to_string(),
        source,
    })
}

fn save(path: &Path, value: &Value) -> Result<(), HookInstallError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| HookInstallError::Write {
            path: parent.display().to_string(),
            source,
        })?;
    }

    let json = serde_json::to_string_pretty(value).expect("valid JSON");
    fs::write(path, json).map_err(|source| HookInstallError::Write {
        path: path.display().to_string(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn install_creates_file_if_missing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");

        let installed = install_session_end_hook(&path).unwrap();
        assert!(installed);
        assert!(path.exists());

        let content: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let hooks = &content["hooks"]["SessionEnd"];
        assert!(hooks.is_array());
        assert_eq!(hooks.as_array().unwrap().len(), 1);
    }

    #[test]
    fn install_into_empty_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{}").unwrap();

        let installed = install_session_end_hook(&path).unwrap();
        assert!(installed);

        let content: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let hook = &content["hooks"]["SessionEnd"][0]["hooks"][0];
        assert_eq!(hook["command"], DCAL_HOOK_COMMAND);
        assert_eq!(hook["timeout"], HOOK_TIMEOUT);
    }

    #[test]
    fn install_preserves_existing_hooks() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");

        let existing = json!({
            "hooks": {
                "SessionEnd": [
                    {
                        "matcher": "other",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "some-other-tool --cleanup"
                            }
                        ]
                    }
                ]
            }
        });
        fs::write(&path, serde_json::to_string_pretty(&existing).unwrap()).unwrap();

        let installed = install_session_end_hook(&path).unwrap();
        assert!(installed);

        let content: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let session_end = content["hooks"]["SessionEnd"].as_array().unwrap();
        assert_eq!(session_end.len(), 2);

        // Original hook still there
        assert_eq!(
            session_end[0]["hooks"][0]["command"],
            "some-other-tool --cleanup"
        );
        // dcal hook appended
        assert_eq!(session_end[1]["hooks"][0]["command"], DCAL_HOOK_COMMAND);
    }

    #[test]
    fn install_preserves_other_hook_types() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");

        let existing = json!({
            "hooks": {
                "Stop": [
                    {
                        "matcher": "other",
                        "hooks": [{ "type": "command", "command": "lint-check" }]
                    }
                ]
            },
            "other_setting": true
        });
        fs::write(&path, serde_json::to_string_pretty(&existing).unwrap()).unwrap();

        install_session_end_hook(&path).unwrap();

        let content: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(content["hooks"]["Stop"].is_array());
        assert!(content["hooks"]["SessionEnd"].is_array());
        assert_eq!(content["other_setting"], true);
    }

    #[test]
    fn install_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");

        install_session_end_hook(&path).unwrap();
        let second = install_session_end_hook(&path).unwrap();
        assert!(!second);

        let content: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let session_end = content["hooks"]["SessionEnd"].as_array().unwrap();
        assert_eq!(session_end.len(), 1);
    }

    #[test]
    fn install_sets_correct_matcher() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");

        install_session_end_hook(&path).unwrap();

        let content: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(content["hooks"]["SessionEnd"][0]["matcher"], "other");
    }

    #[test]
    fn install_handles_empty_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "").unwrap();

        let installed = install_session_end_hook(&path).unwrap();
        assert!(installed);
    }
}
