use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CredentialsError {
    #[error("failed to read credentials at {path}: {source}")]
    Read { path: String, source: std::io::Error },

    #[error("failed to write credentials at {path}: {source}")]
    Write { path: String, source: std::io::Error },

    #[error("failed to set file permissions: {0}")]
    Permissions(std::io::Error),
}

/// Load the Anthropic API key from the credentials file.
///
/// Returns `None` if the file doesn't exist or is empty.
/// Falls back to `ANTHROPIC_API_KEY` env var if the file is not found.
pub fn load_api_key(path: &Path) -> Result<Option<String>, CredentialsError> {
    if path.exists() {
        let content = fs::read_to_string(path).map_err(|source| CredentialsError::Read {
            path: path.display().to_string(),
            source,
        })?;
        let trimmed = content.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(Some(trimmed));
        }
    }

    // Fall back to environment variable
    Ok(std::env::var("ANTHROPIC_API_KEY").ok())
}

/// Save the API key to the credentials file with restricted permissions.
pub fn save_api_key(path: &Path, api_key: &str) -> Result<(), CredentialsError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CredentialsError::Write {
            path: parent.display().to_string(),
            source,
        })?;
    }

    fs::write(path, format!("{api_key}\n")).map_err(|source| CredentialsError::Write {
        path: path.display().to_string(),
        source,
    })?;

    set_owner_only_permissions(path)?;
    Ok(())
}

#[cfg(unix)]
fn set_owner_only_permissions(path: &Path) -> Result<(), CredentialsError> {
    use std::os::unix::fs::PermissionsExt;
    let perms = fs::Permissions::from_mode(0o600);
    fs::set_permissions(path, perms).map_err(CredentialsError::Permissions)
}

#[cfg(not(unix))]
fn set_owner_only_permissions(_path: &Path) -> Result<(), CredentialsError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("credentials");

        save_api_key(&path, "sk-ant-test123").unwrap();
        let loaded = load_api_key(&path).unwrap();
        assert_eq!(loaded, Some("sk-ant-test123".to_string()));
    }

    #[test]
    fn load_returns_none_when_no_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("credentials");

        // Clear env to test file-only path
        let orig = std::env::var("ANTHROPIC_API_KEY").ok();
        std::env::remove_var("ANTHROPIC_API_KEY");

        let result = load_api_key(&path).unwrap();
        assert!(result.is_none());

        if let Some(key) = orig {
            std::env::set_var("ANTHROPIC_API_KEY", key);
        }
    }

    #[test]
    fn load_returns_none_for_empty_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("credentials");
        fs::write(&path, "   \n").unwrap();

        let orig = std::env::var("ANTHROPIC_API_KEY").ok();
        std::env::remove_var("ANTHROPIC_API_KEY");

        let result = load_api_key(&path).unwrap();
        assert!(result.is_none());

        if let Some(key) = orig {
            std::env::set_var("ANTHROPIC_API_KEY", key);
        }
    }

    #[test]
    fn load_trims_whitespace() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("credentials");
        fs::write(&path, "  sk-ant-test123  \n").unwrap();

        let loaded = load_api_key(&path).unwrap();
        assert_eq!(loaded, Some("sk-ant-test123".to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn save_sets_600_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("credentials");

        save_api_key(&path, "sk-ant-test123").unwrap();

        let perms = fs::metadata(&path).unwrap().permissions();
        assert_eq!(perms.mode() & 0o777, 0o600);
    }
}
