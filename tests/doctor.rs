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
fn doctor_passes_on_healthy_setup() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("doctor")
        .assert()
        .stdout(predicate::str::contains("All checks passed"))
        .success();
}

#[test]
fn doctor_detects_orphaned_app() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);

    let profile_dir = roost_dir.join("default");
    fs::create_dir_all(profile_dir.join("someorphan")).unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("doctor")
        .assert()
        .stdout(predicate::str::contains("someorphan"));
}

#[test]
fn doctor_detects_missing_app_in_config() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);

    fs::write(
        roost_dir.join("roost.toml"),
        r#"
ignored = []

[profiles]
[profiles.default]
apps = ["ghost"]
app_sources = {}

[apps]
"#,
    )
    .unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("doctor")
        .assert()
        .failure()
        .stderr(predicate::str::contains("ghost"));
}
