use std::fs;
use std::path::Path;
use std::process::Command;

use thiserror::Error;

use dcal_core::id::generate_project_id;
use dcal_core::paths::DcalPaths;
use dcal_core::project::{ProjectMeta, ProjectPhase, ProjectStatus, RegistryEntry};
use dcal_core::project_files;
use dcal_core::registry;

#[derive(Debug, Error)]
pub enum ScaffoldError {
    #[error("failed to create directory {path}: {source}")]
    CreateDir { path: String, source: std::io::Error },

    #[error("failed to write file {path}: {source}")]
    WriteFile { path: String, source: std::io::Error },

    #[error("git init failed: {0}")]
    GitInit(String),

    #[error("project file error: {0}")]
    ProjectFile(#[from] project_files::ProjectFileError),

    #[error("registry error: {0}")]
    Registry(#[from] registry::RegistryError),
}

/// Parameters for scaffolding a new project.
pub struct ScaffoldParams {
    pub name: String,
    pub description: String,
    pub project_path: String,
    pub claude_md_content: String,
    pub idea_text: String,
    pub git_init: bool,
}

/// Result of scaffolding.
pub struct ScaffoldResult {
    pub id: String,
    pub project_path: String,
}

/// Run Stage 5: create project directory, write files, register.
pub fn run(paths: &DcalPaths, params: ScaffoldParams) -> Result<ScaffoldResult, ScaffoldError> {
    let id = generate_project_id();
    let project_dir = Path::new(&params.project_path);

    // Create project directory
    fs::create_dir_all(project_dir).map_err(|source| ScaffoldError::CreateDir {
        path: params.project_path.clone(),
        source,
    })?;

    // Write CLAUDE.md
    let claude_md_path = project_dir.join("CLAUDE.md");
    fs::write(&claude_md_path, &params.claude_md_content).map_err(|source| {
        ScaffoldError::WriteFile {
            path: claude_md_path.display().to_string(),
            source,
        }
    })?;

    // git init
    if params.git_init {
        let output = Command::new("git")
            .args(["init", &params.project_path])
            .output()
            .map_err(|e| ScaffoldError::GitInit(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ScaffoldError::GitInit(stderr.to_string()));
        }
    }

    // Write dcal project files
    let now = chrono::Utc::now();
    let meta = ProjectMeta {
        id: id.clone(),
        name: params.name.clone(),
        description: params.description,
        path: params.project_path.clone(),
        status: ProjectStatus::Active,
        phase: ProjectPhase::Ideation,
        created_at: now,
        last_active_at: now,
        blocked_reason: None,
        tags: vec![],
        priority: "medium".to_string(),
        cc_session_ids: vec![],
    };

    let dcal_project_dir = paths.project_dir(&id);
    project_files::create_project_dir(&dcal_project_dir, &meta, &params.idea_text)?;

    // Write initial snapshot
    let snapshot = "Project just created. Phase: ideation.\n".to_string();
    project_files::save_snapshot(&paths.project_snapshot(&id), &snapshot)?;

    // Register
    let entry = RegistryEntry::from(&meta);
    registry::add(&paths.registry(), entry)?;

    Ok(ScaffoldResult {
        id,
        project_path: params.project_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, DcalPaths, String) {
        let dir = TempDir::new().unwrap();
        let paths = DcalPaths::new(dir.path().join("dcal-home"));
        fs::create_dir_all(paths.projects_dir()).unwrap();
        fs::write(paths.registry(), "[]").unwrap();

        let project_path = dir.path().join("new-project");
        (dir, paths, project_path.display().to_string())
    }

    #[test]
    fn scaffold_creates_project_directory() {
        let (_dir, paths, project_path) = setup();

        let result = run(
            &paths,
            ScaffoldParams {
                name: "test-app".to_string(),
                description: "A test app".to_string(),
                project_path: project_path.clone(),
                claude_md_content: "# test-app\n".to_string(),
                idea_text: "Build a test app".to_string(),
                git_init: false,
            },
        )
        .unwrap();

        assert!(Path::new(&project_path).exists());
        assert!(Path::new(&project_path).join("CLAUDE.md").exists());
        assert!(result.id.starts_with("proj_"));
    }

    #[test]
    fn scaffold_writes_claude_md() {
        let (_dir, paths, project_path) = setup();

        run(
            &paths,
            ScaffoldParams {
                name: "test-app".to_string(),
                description: "A test app".to_string(),
                project_path: project_path.clone(),
                claude_md_content: "# Generated CLAUDE.md\n\nContent here.\n".to_string(),
                idea_text: "idea".to_string(),
                git_init: false,
            },
        )
        .unwrap();

        let content = fs::read_to_string(Path::new(&project_path).join("CLAUDE.md")).unwrap();
        assert!(content.contains("Generated CLAUDE.md"));
    }

    #[test]
    fn scaffold_creates_dcal_project_files() {
        let (_dir, paths, project_path) = setup();

        let result = run(
            &paths,
            ScaffoldParams {
                name: "test-app".to_string(),
                description: "A test app".to_string(),
                project_path,
                claude_md_content: "# test\n".to_string(),
                idea_text: "Build something cool".to_string(),
                git_init: false,
            },
        )
        .unwrap();

        let dcal_dir = paths.project_dir(&result.id);
        assert!(dcal_dir.join("meta.json").exists());
        assert!(dcal_dir.join("idea.md").exists());
        assert!(dcal_dir.join("snapshot.md").exists());
        assert!(dcal_dir.join("journal.md").exists());
        assert!(dcal_dir.join("sessions.json").exists());
    }

    #[test]
    fn scaffold_writes_correct_meta() {
        let (_dir, paths, project_path) = setup();

        let result = run(
            &paths,
            ScaffoldParams {
                name: "test-app".to_string(),
                description: "A test description".to_string(),
                project_path,
                claude_md_content: "# test\n".to_string(),
                idea_text: "idea".to_string(),
                git_init: false,
            },
        )
        .unwrap();

        let meta = project_files::load_meta(&paths.project_meta(&result.id)).unwrap();
        assert_eq!(meta.name, "test-app");
        assert_eq!(meta.description, "A test description");
        assert_eq!(meta.status, ProjectStatus::Active);
        assert_eq!(meta.phase, ProjectPhase::Ideation);
    }

    #[test]
    fn scaffold_registers_project() {
        let (_dir, paths, project_path) = setup();

        let result = run(
            &paths,
            ScaffoldParams {
                name: "test-app".to_string(),
                description: "desc".to_string(),
                project_path,
                claude_md_content: "# test\n".to_string(),
                idea_text: "idea".to_string(),
                git_init: false,
            },
        )
        .unwrap();

        let entries = registry::load(&paths.registry()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, result.id);
    }

    #[test]
    fn scaffold_with_git_init() {
        let (_dir, paths, project_path) = setup();

        run(
            &paths,
            ScaffoldParams {
                name: "test-app".to_string(),
                description: "desc".to_string(),
                project_path: project_path.clone(),
                claude_md_content: "# test\n".to_string(),
                idea_text: "idea".to_string(),
                git_init: true,
            },
        )
        .unwrap();

        assert!(Path::new(&project_path).join(".git").exists());
    }

    #[test]
    fn scaffold_stores_idea_text() {
        let (_dir, paths, project_path) = setup();

        let result = run(
            &paths,
            ScaffoldParams {
                name: "test-app".to_string(),
                description: "desc".to_string(),
                project_path,
                claude_md_content: "# test\n".to_string(),
                idea_text: "Build a CLI tool for invoices".to_string(),
                git_init: false,
            },
        )
        .unwrap();

        let idea = project_files::load_idea(&paths.project_idea(&result.id)).unwrap();
        assert_eq!(idea, "Build a CLI tool for invoices");
    }
}
