use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn setup_roost(dir: &std::path::Path) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("roost.toml"), "[profiles.default]\napps = []\n").unwrap();
    std::fs::write(
        dir.join("local.toml"),
        "active_profile = \"default\"\n\n[os_info]\nos = \"test\"\narch = \"x86_64\"\n\n[link_paths]\n",
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

#[test]
fn status_errors_without_init() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .arg("status")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not initialized"));
}

#[test]
fn status_shows_profile_and_clean_state() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("Active profile:"))
        .stdout(predicate::str::contains("default"))
        .stdout(predicate::str::contains("App count:"))
        .stdout(predicate::str::contains("0"))
        .stdout(predicate::str::contains("Dirty state:"))
        .stdout(predicate::str::contains("clean"))
        .stdout(predicate::str::contains("Remote URL:"))
        .stdout(predicate::str::contains("none"))
        .stdout(predicate::str::contains("Last commit:"))
        .stdout(predicate::str::contains("init"));
}

#[test]
fn status_shows_app_count() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);
    std::fs::write(
        dir.join("roost.toml"),
        "[profiles.default]\napps = [\"myapp\"]\n\n[apps.myapp]\nis_dir = true\non_profiles = [\"default\"]\n",
    )
    .unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("App count:"))
        .stdout(predicate::str::contains("1"));
}

#[test]
fn status_shows_dirty_state() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);
    std::fs::write(
        dir.join("roost.toml"),
        "[profiles.default]\napps = []\n\n# modified\n",
    )
    .unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("Dirty state:"))
        .stdout(predicate::str::contains("dirty"));
}
