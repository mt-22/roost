use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn setup_roost(dir: &std::path::Path) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
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
    fs::write(
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
    fs::write(dir.join(".gitignore"), "local.toml\n").unwrap();
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

#[test]
fn ignore_errors_without_init() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .arg("ignore")
        .arg("*.log")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not initialized"));
}

#[test]
fn ignore_list_empty() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["ignore", "--list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Global ignore patterns:"))
        .stdout(predicate::str::contains("(none)"));
}

#[test]
fn ignore_list_shows_patterns() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);
    fs::write(
        dir.join("roost.toml"),
        r#"
ignored = ["*.tmp"]

[profiles]
[profiles.default]
apps = ["myapp"]
app_sources = {}

[apps.myapp]
is_dir = true
on_profiles = ["default"]
ignore = ["*.cache"]
"#,
    )
    .unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["ignore", "--list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("*.tmp"))
        .stdout(predicate::str::contains("*.cache"))
        .stdout(predicate::str::contains("Per-app ignore patterns:"));
}

#[test]
fn ignore_add_global_pattern() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["ignore", "*.log"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Added global ignore"));

    let config = fs::read_to_string(dir.join("roost.toml")).unwrap();
    assert!(config.contains("*.log"));
}

#[test]
fn ignore_add_per_app_pattern() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);
    fs::write(
        dir.join("roost.toml"),
        r#"
ignored = []

[profiles]
[profiles.default]
apps = ["myapp"]
app_sources = {}

[apps.myapp]
is_dir = true
on_profiles = ["default"]
"#,
    )
    .unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["ignore", "--app", "myapp", "*.cache"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Added per-app ignore"));

    let config = fs::read_to_string(dir.join("roost.toml")).unwrap();
    assert!(config.contains("*.cache"));
}

#[test]
fn ignore_add_duplicate_no_op() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["ignore", "*.log"])
        .assert()
        .success();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["ignore", "*.log"])
        .assert()
        .success()
        .stdout(predicate::str::contains("already exists"));
}

#[test]
fn ignore_regenerates_gitignore() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["ignore", "secrets.txt"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated .gitignore"));

    let gitignore = fs::read_to_string(dir.join(".gitignore")).unwrap();
    assert!(gitignore.contains("secrets.txt"));
}
