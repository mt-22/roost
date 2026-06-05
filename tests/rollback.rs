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

#[test]
fn rollback_preserves_new_apps() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;

    let tmp = tempfile::tempdir()?;
    let roost_dir = tmp.path().join(".roost");
    fs::create_dir_all(&roost_dir)?;

    // Set up a roost directory with initial state (appA only)
    let initial_config = r#"
[apps.appA]
is_dir = true
on_profiles = ["default"]

[profiles.default]
apps = ["appA"]
"#;
    fs::write(roost_dir.join("roost.toml"), initial_config)?;
    let local_config = r#"
active_profile = "default"

[os_info]
os = "test"
arch = "test"

[link_paths]
appA = "/home/user/.config/appA"
"#;
    fs::write(roost_dir.join("local.toml"), local_config)?;

    // Init git
    let output = std::process::Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(&roost_dir)
        .output()?;
    assert!(output.status.success());

    // Set git identity
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&roost_dir)
        .output()?;
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&roost_dir)
        .output()?;

    // Create appA directory and file
    fs::create_dir_all(roost_dir.join("default").join("appA"))?;
    fs::write(roost_dir.join("default/appA/file1.txt"), "original")?;

    // Commit initial state
    std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(&roost_dir)
        .output()?;
    std::process::Command::new("git")
        .args(["commit", "-m", "initial: add appA"])
        .current_dir(&roost_dir)
        .output()?;

    // Save the hash of the initial commit
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&roost_dir)
        .output()?;
    let initial_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Second commit: add appB, modify appA
    let config2 = r#"
[apps.appA]
is_dir = true
on_profiles = ["default"]

[apps.appB]
is_dir = true
on_profiles = ["default"]

[profiles.default]
apps = ["appA", "appB"]
"#;
    fs::write(roost_dir.join("roost.toml"), config2)?;
    let local_config2 = r#"
active_profile = "default"

[os_info]
os = "test"
arch = "test"

[link_paths]
appA = "/home/user/.config/appA"
appB = "/home/user/.config/appB"
"#;
    fs::write(roost_dir.join("local.toml"), local_config2)?;

    // Create appB directory and file
    fs::create_dir_all(roost_dir.join("default").join("appB"))?;
    fs::write(roost_dir.join("default/appB/file2.txt"), "new app")?;
    // Modify appA's file
    fs::write(roost_dir.join("default/appA/file1.txt"), "modified")?;

    std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(&roost_dir)
        .output()?;
    std::process::Command::new("git")
        .args(["commit", "-m", "add appB, modify appA"])
        .current_dir(&roost_dir)
        .output()?;

    // Run roost rollback to the initial commit
    Command::cargo_bin("roost")?
        .env("ROOST_DIR", &roost_dir)
        .args(["rollback", &initial_hash])
        .assert()
        .success()
        .stdout(predicate::str::contains("Rolled back to"));

    // Verify: appB is still in config (preserved)
    let new_config = fs::read_to_string(roost_dir.join("roost.toml"))?;
    assert!(new_config.contains("appB"), "appB should be preserved in config");

    // Verify: appB files still exist on disk
    assert!(
        roost_dir.join("default/appB/file2.txt").exists(),
        "appB files should exist"
    );

    // Verify: appA file is rolled back to original
    let app_a_content = fs::read_to_string(roost_dir.join("default/appA/file1.txt"))?;
    assert_eq!(app_a_content.trim(), "original", "appA should be rolled back");

    Ok(())
}
