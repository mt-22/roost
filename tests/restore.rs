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
fn restore_errors_without_init() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["restore", "myapp"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not initialized"));
}

#[test]
fn restore_errors_for_unknown_app() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["restore", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn restore_creates_symlink() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);

    let app_dir = dir.join("default").join("myapp");
    fs::create_dir_all(&app_dir).unwrap();
    fs::write(app_dir.join("config.yml"), "data").unwrap();

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
    let origin = tmp.path().join("origin_myapp");
    fs::write(
        dir.join("local.toml"),
        format!(
            r#"
active_profile = "default"

[os_info]
os = "test"
arch = "x86_64"

[link_paths]
myapp = "{}"
"#,
            origin.display()
        ),
    )
    .unwrap();

    assert!(!origin.exists());

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["restore", "myapp"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Restored"));

    assert!(origin.is_symlink());
    assert!(origin.join("config.yml").exists());
}
