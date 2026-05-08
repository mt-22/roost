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
    std::process::Command::new("git")
        .args(["commit", "--allow-empty", "-m", message])
        .current_dir(dir)
        .output()
        .unwrap();
}

#[test]
fn log_shows_commits() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("log")
        .assert()
        .stdout(predicate::str::contains("init"));
}

#[test]
fn log_shows_multiple_commits() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);
    git_commit(&roost_dir, "second");
    git_commit(&roost_dir, "third");

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("log")
        .assert()
        .stdout(predicate::str::contains("init"))
        .stdout(predicate::str::contains("second"))
        .stdout(predicate::str::contains("third"));
}

#[test]
fn undo_removes_last_commit() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);
    git_commit(&roost_dir, "to-remove");

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("undo")
        .assert()
        .success();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("log")
        .assert()
        .stdout(predicate::str::contains("to-remove").not());
}

#[test]
fn undo_defaults_to_one() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);
    git_commit(&roost_dir, "first-extra");
    git_commit(&roost_dir, "second-extra");
    git_commit(&roost_dir, "third-extra");

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("undo")
        .assert()
        .success();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("log")
        .assert()
        .stdout(predicate::str::contains("third-extra").not())
        .stdout(predicate::str::contains("second-extra"))
        .stdout(predicate::str::contains("first-extra"));
}

#[test]
fn remote_shows_no_remote() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("remote")
        .assert()
        .stdout(predicate::str::contains("No remote configured"));
}

#[test]
fn remote_sets_url() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("remote")
        .arg("https://example.com/repo.git")
        .assert()
        .success();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("remote")
        .assert()
        .stdout(predicate::str::contains("https://example.com/repo.git"));
}
