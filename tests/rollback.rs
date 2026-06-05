use assert_cmd::Command;
use predicates::prelude::*;
use std::process::Command as SysCommand;
use tempfile::TempDir;

fn setup_roost(dir: &std::path::Path) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        dir.join("roost.toml"),
        r#"
ignored = []

[profiles]
[profiles.default]
apps = []
app_sources = {}

[apps]
"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("local.toml"),
        r#"
active_profile = "default"

[os_info]
os = "test"
arch = "x86_64"

[link_paths]
"#,
    )
    .unwrap();
    std::fs::write(dir.join(".gitignore"), "local.toml\n").unwrap();
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(dir)
        .output()
        .unwrap();
}

fn git_commit(dir: &std::path::Path, message: &str) {
    SysCommand::new("git")
        .args(["commit", "--allow-empty", "-m", message])
        .current_dir(dir)
        .output()
        .unwrap();
}

#[test]
fn rollback_errors_without_init() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["rollback", "abc123"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not initialized"));
}

#[test]
fn rollback_to_specific_hash() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);
    git_commit(dir, "second");

    let output = SysCommand::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .unwrap();
    let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

    git_commit(dir, "third");

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["rollback", &hash])
        .assert()
        .success()
        .stdout(predicate::str::contains("Rolled back to"));

    // safe_rollback creates a new forward commit rather than moving HEAD
    let log = SysCommand::new("git")
        .args(["log", "-1", "--pretty=%s"])
        .current_dir(dir)
        .output()
        .unwrap();
    let msg = String::from_utf8_lossy(&log.stdout);
    assert!(msg.contains("rollback to"), "expected rollback commit, got: {}", msg);
}

#[test]
fn rollback_rejects_invalid_hash() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["rollback", "notavalidhash"])
        .assert()
        .failure();
}
