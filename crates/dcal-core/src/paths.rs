use std::env;
use std::path::{Path, PathBuf};

const DEFAULT_DCAL_DIR: &str = ".dcal";
const ENV_DCAL_HOME: &str = "DCAL_HOME";

/// Resolved paths for all dcal data files.
///
/// Construct via `DcalPaths::from_env()` for production use, or
/// `DcalPaths::new(path)` with an explicit root for testing.
#[derive(Debug, Clone)]
pub struct DcalPaths {
    root: PathBuf,
}

impl DcalPaths {
    /// Create paths rooted at an explicit directory.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Resolve paths from the environment.
    ///
    /// Uses `DCAL_HOME` if set, otherwise defaults to `~/.dcal/`.
    pub fn from_env() -> Self {
        let root = if let Ok(custom) = env::var(ENV_DCAL_HOME) {
            PathBuf::from(custom)
        } else {
            home_dir().join(DEFAULT_DCAL_DIR)
        };
        Self { root }
    }

    /// The dcal root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Path to `config.yml`.
    pub fn config(&self) -> PathBuf {
        self.root.join("config.yml")
    }

    /// Path to `registry.json`.
    pub fn registry(&self) -> PathBuf {
        self.root.join("registry.json")
    }

    /// Path to `errors.log`.
    pub fn errors_log(&self) -> PathBuf {
        self.root.join("errors.log")
    }

    /// Path to the `projects/` directory.
    pub fn projects_dir(&self) -> PathBuf {
        self.root.join("projects")
    }

    /// Path to a specific project's data directory.
    pub fn project_dir(&self, project_id: &str) -> PathBuf {
        self.projects_dir().join(project_id)
    }

    /// Path to a project's `meta.json`.
    pub fn project_meta(&self, project_id: &str) -> PathBuf {
        self.project_dir(project_id).join("meta.json")
    }

    /// Path to a project's `idea.md`.
    pub fn project_idea(&self, project_id: &str) -> PathBuf {
        self.project_dir(project_id).join("idea.md")
    }

    /// Path to a project's `snapshot.md`.
    pub fn project_snapshot(&self, project_id: &str) -> PathBuf {
        self.project_dir(project_id).join("snapshot.md")
    }

    /// Path to a project's `journal.md`.
    pub fn project_journal(&self, project_id: &str) -> PathBuf {
        self.project_dir(project_id).join("journal.md")
    }

    /// Path to a project's `sessions.json`.
    pub fn project_sessions(&self, project_id: &str) -> PathBuf {
        self.project_dir(project_id).join("sessions.json")
    }
}

fn home_dir() -> PathBuf {
    env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .map(PathBuf::from)
        .expect("HOME (or USERPROFILE on Windows) environment variable not set")
}

/// Expand a leading `~` to the user's home directory.
///
/// Handles both `~/path` (Unix) and `~\path` (Windows) prefixes.
pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        home_dir().join(rest)
    } else if path == "~" {
        home_dir()
    } else {
        PathBuf::from(path)
    }
}

/// Collapse a leading home directory back to `~`.
///
/// Always uses forward slashes for portability across platforms.
pub fn collapse_to_tilde(path: &Path) -> String {
    let home = home_dir();
    if let Ok(rest) = path.strip_prefix(&home) {
        let rest_str = rest.to_string_lossy().replace('\\', "/");
        format!("~/{rest_str}")
    } else {
        path.display().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_paths() -> DcalPaths {
        DcalPaths::new(PathBuf::from("/tmp/test-dcal"))
    }

    #[test]
    fn root_matches_constructor() {
        let paths = test_paths();
        assert_eq!(paths.root(), Path::new("/tmp/test-dcal"));
    }

    #[test]
    fn config_path() {
        let paths = test_paths();
        assert_eq!(paths.config(), PathBuf::from("/tmp/test-dcal/config.yml"));
    }

    #[test]
    fn registry_path() {
        let paths = test_paths();
        assert_eq!(paths.registry(), PathBuf::from("/tmp/test-dcal/registry.json"));
    }

    #[test]
    fn errors_log_path() {
        let paths = test_paths();
        assert_eq!(paths.errors_log(), PathBuf::from("/tmp/test-dcal/errors.log"));
    }

    #[test]
    fn projects_dir_path() {
        let paths = test_paths();
        assert_eq!(paths.projects_dir(), PathBuf::from("/tmp/test-dcal/projects"));
    }

    #[test]
    fn project_dir_path() {
        let paths = test_paths();
        assert_eq!(
            paths.project_dir("proj_abc123"),
            PathBuf::from("/tmp/test-dcal/projects/proj_abc123")
        );
    }

    #[test]
    fn project_file_paths() {
        let paths = test_paths();
        let id = "proj_abc123";
        assert_eq!(
            paths.project_meta(id),
            PathBuf::from("/tmp/test-dcal/projects/proj_abc123/meta.json")
        );
        assert_eq!(
            paths.project_idea(id),
            PathBuf::from("/tmp/test-dcal/projects/proj_abc123/idea.md")
        );
        assert_eq!(
            paths.project_snapshot(id),
            PathBuf::from("/tmp/test-dcal/projects/proj_abc123/snapshot.md")
        );
        assert_eq!(
            paths.project_journal(id),
            PathBuf::from("/tmp/test-dcal/projects/proj_abc123/journal.md")
        );
        assert_eq!(
            paths.project_sessions(id),
            PathBuf::from("/tmp/test-dcal/projects/proj_abc123/sessions.json")
        );
    }

    #[test]
    fn from_env_with_dcal_home() {
        env::set_var("DCAL_HOME", "/tmp/custom-dcal");
        let paths = DcalPaths::from_env();
        assert_eq!(paths.root(), Path::new("/tmp/custom-dcal"));
        env::remove_var("DCAL_HOME");
    }

    #[test]
    fn from_env_defaults_to_dot_dcal() {
        env::remove_var("DCAL_HOME");
        let paths = DcalPaths::from_env();
        assert!(paths.root().ends_with(".dcal"));
    }

    #[test]
    fn expand_tilde_with_path() {
        let expanded = expand_tilde("~/projects/my-app");
        assert!(!expanded.starts_with("~"));
        assert!(expanded.ends_with("projects/my-app"));
    }

    #[test]
    fn expand_tilde_bare() {
        let expanded = expand_tilde("~");
        assert_eq!(expanded, home_dir());
    }

    #[test]
    fn expand_tilde_with_backslash() {
        let expanded = expand_tilde("~\\projects\\my-app");
        assert!(!expanded.starts_with("~"));
    }

    #[test]
    fn expand_tilde_no_tilde() {
        let expanded = expand_tilde("/absolute/path");
        assert_eq!(expanded, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn collapse_roundtrip() {
        let original = "~/projects/my-app";
        let expanded = expand_tilde(original);
        let collapsed = collapse_to_tilde(&expanded);
        assert_eq!(collapsed, original);
    }
}
