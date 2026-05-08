use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn setup_roost(dir: &std::path::Path) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        dir.join("roost.toml"),
        "[profiles.default]\napps = []\n",
    )
    .unwrap();
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

fn setup_roost_with_app(dir: &std::path::Path) {
    setup_roost(dir);
    std::fs::write(
        dir.join("roost.toml"),
        "[profiles.default]\napps = [\"myapp\"]\n\n[apps.myapp]\nis_dir = true\non_profiles = [\"default\"]\n",
    )
    .unwrap();
}

#[test]
fn profile_list_shows_profiles() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["profile", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("default"));
}

#[test]
fn profile_list_marks_active() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["profile", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("* default"));
}

#[test]
fn profile_add_creates_new() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["profile", "add", "testprofile"])
        .assert()
        .success();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["profile", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("testprofile"));
}

#[test]
fn profile_add_rejects_duplicate() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["profile", "add", "default"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn profile_add_from_copies_apps() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost_with_app(dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["profile", "add", "testprofile", "--from", "default"])
        .assert()
        .success();

    let shared_path = dir.join("roost.toml");
    let shared = roost::app::load_shared(&shared_path).unwrap();
    assert!(shared.profiles.contains_key("testprofile"));
    assert!(shared.profiles["testprofile"].apps.contains("myapp"));
}

#[test]
fn profile_delete_removes_profile() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["profile", "add", "testprofile"])
        .assert()
        .success();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["profile", "delete", "testprofile"])
        .assert()
        .success();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["profile", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("testprofile").not());
}

#[test]
fn profile_rename_works() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["profile", "add", "testprofile"])
        .assert()
        .success();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["profile", "rename", "testprofile", "renamed"])
        .assert()
        .success();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["profile", "list"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("renamed")
                .and(predicate::str::contains("testprofile").not()),
        );
}

#[test]
fn profile_switch_changes_active() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["profile", "add", "testprofile"])
        .assert()
        .success();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["profile", "switch", "testprofile"])
        .assert()
        .success();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["profile", "list"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("* testprofile")
                .and(predicate::str::contains("* default").not()),
        );
}
