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
    dcal().arg("--version").assert().success().stdout(predicate::str::contains("0.1.0"));
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
                .and(predicate::str::contains("paused")),
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

// -- error log --

#[test]
fn hook_mode_bad_stdin_logs_error() {
    let home = setup_home();

    dcal()
        .args(["checkin", "--auto", "--project-from-cwd"])
        .env("DCAL_HOME", home.path())
        .write_stdin("not valid json")
        .assert()
        .success();

    let log = fs::read_to_string(home.path().join("errors.log")).unwrap();
    assert!(log.contains("failed to parse hook input"));
}
