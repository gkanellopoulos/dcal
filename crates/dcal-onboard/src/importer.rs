use chrono::{DateTime, Utc};
use thiserror::Error;

use dcal_core::id::generate_project_id;
use dcal_core::paths::DcalPaths;
use dcal_core::project::{ProjectMeta, ProjectPhase, ProjectStatus, RegistryEntry};
use dcal_core::project_files;
use dcal_core::registry;

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("failed to write project files: {0}")]
    ProjectFile(#[from] project_files::ProjectFileError),

    #[error("failed to update registry: {0}")]
    Registry(#[from] registry::RegistryError),
}

/// Parameters for importing a project into dcal.
pub struct ImportParams {
    pub name: String,
    pub description: String,
    pub path: String,
    pub status: ProjectStatus,
    pub last_active_at: Option<DateTime<Utc>>,
    pub cc_model: String,
}

/// Import result containing the generated project ID.
pub struct ImportResult {
    pub id: String,
    pub name: String,
}

/// Import an existing project into the dcal registry.
///
/// Creates the project data directory with initial files and appends
/// an entry to the registry. Phase is set to `Unknown` for onboarded
/// projects.
pub fn import(paths: &DcalPaths, params: ImportParams) -> Result<ImportResult, ImportError> {
    let id = generate_project_id();
    let now = Utc::now();

    let meta = ProjectMeta {
        id: id.clone(),
        name: params.name.clone(),
        description: params.description,
        path: params.path,
        status: params.status,
        phase: ProjectPhase::Unknown,
        created_at: now,
        last_active_at: params.last_active_at.unwrap_or(now),
        blocked_reason: None,
        tags: vec![],
        priority: "medium".to_string(),
        cc_session_ids: vec![],
        cc_model: params.cc_model,
    };

    let project_dir = paths.project_dir(&id);
    project_files::create_project_dir(&project_dir, &meta, "")?;

    let entry = RegistryEntry::from(&meta);
    registry::add(&paths.registry(), entry)?;

    Ok(ImportResult {
        id,
        name: params.name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, DcalPaths) {
        let dir = TempDir::new().unwrap();
        let paths = DcalPaths::new(dir.path().to_path_buf());
        std::fs::create_dir_all(paths.projects_dir()).unwrap();
        std::fs::write(paths.registry(), "[]").unwrap();
        (dir, paths)
    }

    #[test]
    fn import_creates_project_files() {
        let (_dir, paths) = setup();

        let result = import(
            &paths,
            ImportParams {
                name: "myapp".to_string(),
                description: "A test app".to_string(),
                path: "~/projects/myapp".to_string(),
                status: ProjectStatus::Active,
                last_active_at: None,
                cc_model: String::new(),
            },
        )
        .unwrap();

        let project_dir = paths.project_dir(&result.id);
        assert!(project_dir.join("meta.json").exists());
        assert!(project_dir.join("idea.md").exists());
        assert!(project_dir.join("snapshot.md").exists());
        assert!(project_dir.join("journal.md").exists());
        assert!(project_dir.join("sessions.json").exists());
    }

    #[test]
    fn import_writes_correct_meta() {
        let (_dir, paths) = setup();

        let result = import(
            &paths,
            ImportParams {
                name: "myapp".to_string(),
                description: "A test app".to_string(),
                path: "~/projects/myapp".to_string(),
                status: ProjectStatus::Paused,
                last_active_at: None,
                cc_model: String::new(),
            },
        )
        .unwrap();

        let meta = project_files::load_meta(&paths.project_meta(&result.id)).unwrap();
        assert_eq!(meta.name, "myapp");
        assert_eq!(meta.description, "A test app");
        assert_eq!(meta.status, ProjectStatus::Paused);
        assert_eq!(meta.phase, ProjectPhase::Unknown);
    }

    #[test]
    fn import_adds_registry_entry() {
        let (_dir, paths) = setup();

        let result = import(
            &paths,
            ImportParams {
                name: "myapp".to_string(),
                description: "A test app".to_string(),
                path: "~/projects/myapp".to_string(),
                status: ProjectStatus::Active,
                last_active_at: None,
                cc_model: String::new(),
            },
        )
        .unwrap();

        let entries = registry::load(&paths.registry()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, result.id);
        assert_eq!(entries[0].name, "myapp");
    }

    #[test]
    fn import_uses_last_active_when_provided() {
        let (_dir, paths) = setup();
        let custom_date = chrono::TimeZone::with_ymd_and_hms(&Utc, 2025, 6, 15, 12, 0, 0).unwrap();

        let result = import(
            &paths,
            ImportParams {
                name: "oldapp".to_string(),
                description: "An old project".to_string(),
                path: "~/projects/oldapp".to_string(),
                status: ProjectStatus::Paused,
                last_active_at: Some(custom_date),
                cc_model: String::new(),
            },
        )
        .unwrap();

        let meta = project_files::load_meta(&paths.project_meta(&result.id)).unwrap();
        assert_eq!(meta.last_active_at, custom_date);
    }

    #[test]
    fn import_generates_unique_ids() {
        let (_dir, paths) = setup();

        let r1 = import(
            &paths,
            ImportParams {
                name: "app1".to_string(),
                description: "First".to_string(),
                path: "~/projects/app1".to_string(),
                status: ProjectStatus::Active,
                last_active_at: None,
                cc_model: String::new(),
            },
        )
        .unwrap();

        let r2 = import(
            &paths,
            ImportParams {
                name: "app2".to_string(),
                description: "Second".to_string(),
                path: "~/projects/app2".to_string(),
                status: ProjectStatus::Active,
                last_active_at: None,
                cc_model: String::new(),
            },
        )
        .unwrap();

        assert_ne!(r1.id, r2.id);

        let entries = registry::load(&paths.registry()).unwrap();
        assert_eq!(entries.len(), 2);
    }
}
