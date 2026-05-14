use std::fs;

use assert_cmd::Command;
use chrono::Utc;
use predicates::prelude::*;
use tempfile::TempDir;

fn dcal() -> Command {
    Command::cargo_bin("dcal").unwrap()
}

fn setup_home() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("projects")).unwrap();
    fs::write(dir.path().join("registry.json"), "[]").unwrap();
    dir
}

fn seed_project(home: &TempDir, id: &str, name: &str, status: &str, phase: &str) {
    let now = Utc::now();
    let meta = serde_json::json!({
        "id": id,
        "name": name,
        "description": format!("Test project {name}"),
        "path": format!("~/projects/{name}"),
        "status": status,
        "phase": phase,
        "created_at": now,
        "last_active_at": now,
        "tags": [],
        "priority": "medium",
        "cc_session_ids": []
    });

    let project_dir = home.path().join("projects").join(id);
    fs::create_dir_all(&project_dir).unwrap();
    fs::write(project_dir.join("meta.json"), serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    fs::write(project_dir.join("idea.md"), "test idea").unwrap();
    fs::write(project_dir.join("snapshot.md"), "").unwrap();
    fs::write(project_dir.join("journal.md"), "").unwrap();
    fs::write(project_dir.join("sessions.json"), "[]").unwrap();

    let entry = serde_json::json!({
        "id": id,
        "name": name,
        "path": format!("~/projects/{name}"),
        "status": status,
        "created_at": now,
        "last_active_at": now,
    });

    let registry_path = home.path().join("registry.json");
    let mut entries: Vec<serde_json::Value> =
        serde_json::from_str(&fs::read_to_string(&registry_path).unwrap()).unwrap();
    entries.push(entry);
    fs::write(&registry_path, serde_json::to_string_pretty(&entries).unwrap()).unwrap();
}

// -- help / version --

#[test]
fn help_flag() {
    dcal().arg("--help").assert().success().stdout(predicate::str::contains("dcal"));
}

#[test]
fn version_flag() {
    dcal().arg("--version").assert().success().stdout(predicate::str::contains("0.2.0"));
}

// -- list --

#[test]
fn list_empty_registry() {
    let home = setup_home();
    dcal()
        .arg("list")
        .env("DCAL_HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No projects found"));
}

#[test]
fn list_shows_projects() {
    let home = setup_home();
    seed_project(&home, "proj_aaa111", "my-app", "active", "design");
    seed_project(&home, "proj_bbb222", "my-lib", "paused", "testing");

    dcal()
        .arg("list")
        .env("DCAL_HOME", home.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("my-app")
                .and(predicate::str::contains("my-lib"))
                .and(predicate::str::contains("active"))
                .and(predicate::str::contains("paused"))
                .and(predicate::str::contains("proj_aaa111"))
                .and(predicate::str::contains("proj_bbb222")),
        );
}

#[test]
fn list_filter_by_status() {
    let home = setup_home();
    seed_project(&home, "proj_aaa111", "my-app", "active", "design");
    seed_project(&home, "proj_bbb222", "my-lib", "paused", "testing");

    dcal()
        .arg("list")
        .args(["--status", "paused"])
        .env("DCAL_HOME", home.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("my-lib")
                .and(predicate::str::contains("my-app").not()),
        );
}

#[test]
fn list_invalid_status() {
    let home = setup_home();
    dcal()
        .arg("list")
        .args(["--status", "bogus"])
        .env("DCAL_HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown status"));
}

// -- info --

#[test]
fn info_shows_dashboard() {
    let home = setup_home();
    seed_project(&home, "proj_aaa111", "my-app", "active", "design");

    dcal()
        .args(["info", "my-app"])
        .env("DCAL_HOME", home.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("my-app")
                .and(predicate::str::contains("proj_aaa111"))
                .and(predicate::str::contains("design")),
        );
}

#[test]
fn info_by_id() {
    let home = setup_home();
    seed_project(&home, "proj_aaa111", "my-app", "active", "design");

    dcal()
        .args(["info", "proj_aaa111"])
        .env("DCAL_HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("my-app"));
}

#[test]
fn info_unknown_project() {
    let home = setup_home();

    dcal()
        .args(["info", "nonexistent"])
        .env("DCAL_HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("no project found"));
}

// -- journal --

#[test]
fn journal_empty() {
    let home = setup_home();
    seed_project(&home, "proj_aaa111", "my-app", "active", "design");

    dcal()
        .args(["journal", "my-app"])
        .env("DCAL_HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No journal entries"));
}

#[test]
fn journal_with_content() {
    let home = setup_home();
    seed_project(&home, "proj_aaa111", "my-app", "active", "design");
    fs::write(
        home.path().join("projects/proj_aaa111/journal.md"),
        "## Session — 2026-05-14\n\nDid some work.\n",
    )
    .unwrap();

    dcal()
        .args(["journal", "my-app"])
        .env("DCAL_HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Did some work"));
}

// -- snapshot --

#[test]
fn snapshot_empty() {
    let home = setup_home();
    seed_project(&home, "proj_aaa111", "my-app", "active", "design");

    dcal()
        .args(["snapshot", "my-app"])
        .env("DCAL_HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No snapshot"));
}

// -- sessions --

#[test]
fn sessions_empty() {
    let home = setup_home();
    seed_project(&home, "proj_aaa111", "my-app", "active", "design");

    dcal()
        .args(["sessions", "my-app"])
        .env("DCAL_HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No sessions"));
}

// -- pause --

#[test]
fn pause_active_project() {
    let home = setup_home();
    seed_project(&home, "proj_aaa111", "my-app", "active", "design");

    dcal()
        .args(["pause", "my-app", "--note", "taking a break"])
        .env("DCAL_HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Paused 'my-app'"));

    let meta: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(home.path().join("projects/proj_aaa111/meta.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(meta["status"], "paused");

    let journal =
        fs::read_to_string(home.path().join("projects/proj_aaa111/journal.md")).unwrap();
    assert!(journal.contains("taking a break"));
}

#[test]
fn pause_already_paused_fails() {
    let home = setup_home();
    seed_project(&home, "proj_aaa111", "my-app", "paused", "design");

    dcal()
        .args(["pause", "my-app"])
        .env("DCAL_HOME", home.path())
        .assert()
        .failure();
}

#[test]
fn pause_unknown_project_fails() {
    let home = setup_home();

    dcal()
        .args(["pause", "nonexistent"])
        .env("DCAL_HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("no project found"));
}

// -- phase --

#[test]
fn phase_update() {
    let home = setup_home();
    seed_project(&home, "proj_aaa111", "my-app", "active", "design");

    dcal()
        .args(["phase", "my-app", "testing"])
        .env("DCAL_HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("testing"));

    let meta: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(home.path().join("projects/proj_aaa111/meta.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(meta["phase"], "testing");
}

#[test]
fn phase_invalid_value() {
    let home = setup_home();
    seed_project(&home, "proj_aaa111", "my-app", "active", "design");

    dcal()
        .args(["phase", "my-app", "bogus"])
        .env("DCAL_HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown phase"));
}

#[test]
fn phase_same_value_noop() {
    let home = setup_home();
    seed_project(&home, "proj_aaa111", "my-app", "active", "design");

    dcal()
        .args(["phase", "my-app", "design"])
        .env("DCAL_HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("already"));
}

// -- checkin --

#[test]
fn checkin_hook_mode_no_matching_project() {
    let home = setup_home();
    let stdin_json = serde_json::json!({
        "session_id": "test-session",
        "transcript_path": "/tmp/nonexistent-transcript.jsonl",
        "cwd": "/some/random/path"
    });

    dcal()
        .args(["checkin", "--auto", "--project-from-cwd"])
        .env("DCAL_HOME", home.path())
        .write_stdin(serde_json::to_string(&stdin_json).unwrap())
        .assert()
        .success();
}

#[test]
fn checkin_no_args_shows_usage() {
    let home = setup_home();
    dcal()
        .arg("checkin")
        .env("DCAL_HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("usage"));
}

// -- new --

#[test]
fn new_invalid_path_fails_before_api() {
    let home = setup_home();
    fs::write(
        home.path().join("config.yml"),
        "version: \"1.0\"\npersonal:\n  name: test\n",
    )
    .unwrap();

    dcal()
        .args(["new", "--path", "/nonexistent/deep/path/project"])
        .env("DCAL_HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
}

// -- resolve by ID --

#[test]
fn pause_by_project_id() {
    let home = setup_home();
    seed_project(&home, "proj_aaa111", "my-app", "active", "design");

    dcal()
        .args(["pause", "proj_aaa111"])
        .env("DCAL_HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Paused 'my-app'"));
}

// -- search --

#[test]
fn search_finds_matching() {
    let home = setup_home();
    seed_project(&home, "proj_aaa111", "my-app", "active", "design");
    seed_project(&home, "proj_bbb222", "my-lib", "paused", "testing");
    seed_project(&home, "proj_ccc333", "other-tool", "active", "implementation");

    dcal()
        .args(["search", "my"])
        .env("DCAL_HOME", home.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("my-app")
                .and(predicate::str::contains("my-lib"))
                .and(predicate::str::contains("other-tool").not()),
        );
}

#[test]
fn search_case_insensitive() {
    let home = setup_home();
    seed_project(&home, "proj_aaa111", "MyApp", "active", "design");

    dcal()
        .args(["search", "myapp"])
        .env("DCAL_HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("MyApp"));
}

#[test]
fn search_no_matches() {
    let home = setup_home();
    seed_project(&home, "proj_aaa111", "my-app", "active", "design");

    dcal()
        .args(["search", "zzz"])
        .env("DCAL_HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No projects matching"));
}

// -- sync --

#[test]
fn sync_no_projects() {
    let home = setup_home();

    dcal()
        .arg("sync")
        .env("DCAL_HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No projects to sync"));
}

#[test]
fn sync_single_project() {
    let home = setup_home();
    seed_project(&home, "proj_aaa111", "my-app", "active", "design");

    dcal()
        .args(["sync", "my-app"])
        .env("DCAL_HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
}

// -- default routing (dcal <name|id> → info) --

#[test]
fn default_routing_by_name() {
    let home = setup_home();
    seed_project(&home, "proj_aaa111", "my-app", "active", "design");

    dcal()
        .arg("my-app")
        .env("DCAL_HOME", home.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("my-app")
                .and(predicate::str::contains("proj_aaa111")),
        );
}

#[test]
fn default_routing_by_id() {
    let home = setup_home();
    seed_project(&home, "proj_aaa111", "my-app", "active", "design");

    dcal()
        .arg("proj_aaa111")
        .env("DCAL_HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("my-app"));
}

#[test]
fn default_routing_unknown() {
    let home = setup_home();

    dcal()
        .arg("nonexistent")
        .env("DCAL_HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("no project found"));
}

// -- error log --

#[test]
fn hook_mode_bad_stdin_logs_error_and_exits_nonzero() {
    let home = setup_home();

    dcal()
        .args(["checkin", "--auto", "--project-from-cwd"])
        .env("DCAL_HOME", home.path())
        .write_stdin("not valid json")
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to parse hook input"));

    let log = fs::read_to_string(home.path().join("errors.log")).unwrap();
    assert!(log.contains("failed to parse hook input"));
}
