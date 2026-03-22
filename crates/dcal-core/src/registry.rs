use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::lock::FileLock;
use crate::project::RegistryEntry;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("registry file not found: {0}")]
    NotFound(PathBuf),

    #[error("failed to read registry at {path}: {source}")]
    Read { path: PathBuf, source: std::io::Error },

    #[error("failed to write registry at {path}: {source}")]
    Write { path: PathBuf, source: std::io::Error },

    #[error("failed to parse registry at {path}: {source}")]
    Parse { path: PathBuf, source: serde_json::Error },

    #[error("failed to serialize registry: {0}")]
    Serialize(serde_json::Error),

    #[error("failed to lock registry: {0}")]
    Lock(#[from] crate::lock::LockError),

    #[error("project not found: {0}")]
    ProjectNotFound(String),

    #[error("duplicate project ID: {0}")]
    DuplicateId(String),
}

/// Load all registry entries from disk.
///
/// Returns an empty vec if the file doesn't exist yet.
pub fn load(registry_path: &Path) -> Result<Vec<RegistryEntry>, RegistryError> {
    if !registry_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(registry_path).map_err(|source| RegistryError::Read {
        path: registry_path.to_path_buf(),
        source,
    })?;

    serde_json::from_str(&content).map_err(|source| RegistryError::Parse {
        path: registry_path.to_path_buf(),
        source,
    })
}

/// Save registry entries to disk, acquiring a file lock first.
pub fn save(registry_path: &Path, entries: &[RegistryEntry]) -> Result<(), RegistryError> {
    let _lock = FileLock::acquire(registry_path)?;

    let json = serde_json::to_string_pretty(entries).map_err(RegistryError::Serialize)?;

    fs::write(registry_path, json).map_err(|source| RegistryError::Write {
        path: registry_path.to_path_buf(),
        source,
    })
}

/// Add a new entry to the registry.
///
/// Fails if an entry with the same ID already exists.
pub fn add(registry_path: &Path, entry: RegistryEntry) -> Result<(), RegistryError> {
    let _lock = FileLock::acquire(registry_path)?;

    let mut entries = load_unlocked(registry_path)?;

    if entries.iter().any(|e| e.id == entry.id) {
        return Err(RegistryError::DuplicateId(entry.id));
    }

    entries.push(entry);
    save_unlocked(registry_path, &entries)
}

/// Find a registry entry by project ID.
pub fn find_by_id<'a>(entries: &'a [RegistryEntry], id: &str) -> Option<&'a RegistryEntry> {
    entries.iter().find(|e| e.id == id)
}

/// Find all registry entries matching a name (case-insensitive).
pub fn find_by_name<'a>(entries: &'a [RegistryEntry], name: &str) -> Vec<&'a RegistryEntry> {
    let lower = name.to_lowercase();
    entries
        .iter()
        .filter(|e| e.name.to_lowercase() == lower)
        .collect()
}

/// Update an existing registry entry, matched by ID.
///
/// Returns an error if the entry is not found.
pub fn update(registry_path: &Path, updated: &RegistryEntry) -> Result<(), RegistryError> {
    let _lock = FileLock::acquire(registry_path)?;

    let mut entries = load_unlocked(registry_path)?;

    let pos = entries
        .iter()
        .position(|e| e.id == updated.id)
        .ok_or_else(|| RegistryError::ProjectNotFound(updated.id.clone()))?;

    entries[pos] = updated.clone();
    save_unlocked(registry_path, &entries)
}

/// Remove a registry entry by project ID.
///
/// Returns an error if the entry is not found.
pub fn remove(registry_path: &Path, project_id: &str) -> Result<(), RegistryError> {
    let _lock = FileLock::acquire(registry_path)?;

    let mut entries = load_unlocked(registry_path)?;
    let initial_len = entries.len();
    entries.retain(|e| e.id != project_id);

    if entries.len() == initial_len {
        return Err(RegistryError::ProjectNotFound(project_id.to_string()));
    }

    save_unlocked(registry_path, &entries)
}

/// Load without acquiring a lock (caller already holds it).
fn load_unlocked(registry_path: &Path) -> Result<Vec<RegistryEntry>, RegistryError> {
    if !registry_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(registry_path).map_err(|source| RegistryError::Read {
        path: registry_path.to_path_buf(),
        source,
    })?;

    serde_json::from_str(&content).map_err(|source| RegistryError::Parse {
        path: registry_path.to_path_buf(),
        source,
    })
}

/// Save without acquiring a lock (caller already holds it).
fn save_unlocked(
    registry_path: &Path,
    entries: &[RegistryEntry],
) -> Result<(), RegistryError> {
    let json = serde_json::to_string_pretty(entries).map_err(RegistryError::Serialize)?;

    fs::write(registry_path, json).map_err(|source| RegistryError::Write {
        path: registry_path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::ProjectStatus;
    use chrono::{TimeZone, Utc};
    use tempfile::TempDir;

    fn sample_entry(id: &str, name: &str) -> RegistryEntry {
        RegistryEntry {
            id: id.to_string(),
            name: name.to_string(),
            path: format!("~/projects/{name}"),
            status: ProjectStatus::Active,
            created_at: Utc.with_ymd_and_hms(2026, 1, 14, 9, 0, 0).unwrap(),
            last_active_at: Utc.with_ymd_and_hms(2026, 2, 28, 16, 0, 0).unwrap(),
        }
    }

    fn setup() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("registry.json");
        (dir, path)
    }

    #[test]
    fn load_returns_empty_when_no_file() {
        let (_dir, path) = setup();
        let entries = load(&path).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let (_dir, path) = setup();
        let entries = vec![
            sample_entry("proj_aaa111", "alpha"),
            sample_entry("proj_bbb222", "beta"),
        ];
        save(&path, &entries).unwrap();

        let loaded = load(&path).unwrap();
        assert_eq!(entries, loaded);
    }

    #[test]
    fn add_appends_entry() {
        let (_dir, path) = setup();
        fs::write(&path, "[]").unwrap();

        add(&path, sample_entry("proj_aaa111", "alpha")).unwrap();
        add(&path, sample_entry("proj_bbb222", "beta")).unwrap();

        let loaded = load(&path).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "alpha");
        assert_eq!(loaded[1].name, "beta");
    }

    #[test]
    fn add_rejects_duplicate_id() {
        let (_dir, path) = setup();
        fs::write(&path, "[]").unwrap();

        add(&path, sample_entry("proj_aaa111", "alpha")).unwrap();
        let result = add(&path, sample_entry("proj_aaa111", "different-name"));
        assert!(matches!(result, Err(RegistryError::DuplicateId(_))));
    }

    #[test]
    fn add_creates_file_if_missing() {
        let (_dir, path) = setup();
        add(&path, sample_entry("proj_aaa111", "alpha")).unwrap();

        let loaded = load(&path).unwrap();
        assert_eq!(loaded.len(), 1);
    }

    #[test]
    fn find_by_id_returns_match() {
        let entries = vec![
            sample_entry("proj_aaa111", "alpha"),
            sample_entry("proj_bbb222", "beta"),
        ];
        let found = find_by_id(&entries, "proj_bbb222");
        assert_eq!(found.unwrap().name, "beta");
    }

    #[test]
    fn find_by_id_returns_none() {
        let entries = vec![sample_entry("proj_aaa111", "alpha")];
        assert!(find_by_id(&entries, "proj_zzz999").is_none());
    }

    #[test]
    fn find_by_name_case_insensitive() {
        let entries = vec![
            sample_entry("proj_aaa111", "Alpha"),
            sample_entry("proj_bbb222", "beta"),
        ];
        let found = find_by_name(&entries, "alpha");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "proj_aaa111");
    }

    #[test]
    fn find_by_name_returns_multiple_matches() {
        let entries = vec![
            sample_entry("proj_aaa111", "myapp"),
            sample_entry("proj_bbb222", "myapp"),
        ];
        let found = find_by_name(&entries, "myapp");
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn find_by_name_returns_empty_on_no_match() {
        let entries = vec![sample_entry("proj_aaa111", "alpha")];
        let found = find_by_name(&entries, "nonexistent");
        assert!(found.is_empty());
    }

    #[test]
    fn update_modifies_entry() {
        let (_dir, path) = setup();
        let entries = vec![sample_entry("proj_aaa111", "alpha")];
        save(&path, &entries).unwrap();

        let mut updated = sample_entry("proj_aaa111", "alpha");
        updated.status = ProjectStatus::Paused;
        update(&path, &updated).unwrap();

        let loaded = load(&path).unwrap();
        assert_eq!(loaded[0].status, ProjectStatus::Paused);
    }

    #[test]
    fn update_fails_for_unknown_id() {
        let (_dir, path) = setup();
        save(&path, &[sample_entry("proj_aaa111", "alpha")]).unwrap();

        let unknown = sample_entry("proj_zzz999", "ghost");
        let result = update(&path, &unknown);
        assert!(matches!(result, Err(RegistryError::ProjectNotFound(_))));
    }

    #[test]
    fn remove_deletes_entry() {
        let (_dir, path) = setup();
        let entries = vec![
            sample_entry("proj_aaa111", "alpha"),
            sample_entry("proj_bbb222", "beta"),
        ];
        save(&path, &entries).unwrap();

        remove(&path, "proj_aaa111").unwrap();

        let loaded = load(&path).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "proj_bbb222");
    }

    #[test]
    fn remove_fails_for_unknown_id() {
        let (_dir, path) = setup();
        save(&path, &[sample_entry("proj_aaa111", "alpha")]).unwrap();

        let result = remove(&path, "proj_zzz999");
        assert!(matches!(result, Err(RegistryError::ProjectNotFound(_))));
    }
}
