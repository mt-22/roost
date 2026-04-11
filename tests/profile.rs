use predicates::prelude::*;
use std::fs;

mod helpers;

use helpers::TestRoost;

fn add_test_app(tr: &TestRoost, app_name: &str) {
    let app_dir = tr.home_dir.join(app_name);
    fs::create_dir_all(&app_dir).unwrap();
    fs::write(app_dir.join("config.toml"), "test = true").unwrap();
    tr.cmd()
        .args(["add", app_dir.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn test_profile_add_empty() {
    let tr = TestRoost::new();
    tr.init_minimal();

    tr.cmd()
        .args(["profile", "add", "work", "--empty"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created empty profile 'work'"));

    let config = roost::app::SharedAppConfig::load(&tr.roost_config).unwrap();
    assert!(config.profiles.contains_key("work"));
    assert!(tr.roost_dir.join("work").exists());
}

#[test]
fn test_profile_add_duplicate_fails() {
    let tr = TestRoost::new();
    tr.init_minimal();

    tr.cmd()
        .args(["profile", "add", "default", "--empty"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn test_profile_add_clones_template() {
    let tr = TestRoost::new();
    tr.init_minimal();

    add_test_app(&tr, "myapp");

    tr.cmd()
        .args(["profile", "add", "laptop"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created profile 'laptop'"));

    let config = roost::app::SharedAppConfig::load(&tr.roost_config).unwrap();
    let laptop = config.profiles.get("laptop").unwrap();
    assert!(laptop.apps.contains("myapp"));
}

#[test]
fn test_profile_list() {
    let tr = TestRoost::new();
    tr.init_minimal();

    tr.cmd()
        .args(["profile", "add", "work", "--empty"])
        .assert()
        .success();

    let mut cmd = tr.cmd();
    cmd.args(["profile", "list"]);
    let output = cmd.assert().success().get_output().clone();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("default"));
    assert!(stdout.contains("work"));
    assert!(stdout.contains("* default"));
}

#[test]
fn test_profile_switch() {
    let tr = TestRoost::new();
    tr.init_minimal();

    tr.cmd()
        .args(["profile", "add", "work", "--empty"])
        .assert()
        .success();

    tr.cmd()
        .args(["profile", "switch", "work"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Switched to profile 'work'"));

    let local = roost::app::LocalAppConfig::load(&tr.local_config).unwrap();
    assert_eq!(local.active_profile, "work");
}

#[test]
fn test_profile_switch_nonexistent_fails() {
    let tr = TestRoost::new();
    tr.init_minimal();

    tr.cmd()
        .args(["profile", "switch", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
}

#[test]
fn test_profile_delete() {
    let tr = TestRoost::new();
    tr.init_minimal();

    tr.cmd()
        .args(["profile", "add", "work", "--empty"])
        .assert()
        .success();

    tr.cmd()
        .args(["profile", "switch", "work"])
        .assert()
        .success();

    tr.cmd()
        .args(["profile", "delete", "default"])
        .write_stdin("y\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Deleted profile 'default'"));

    let config = roost::app::SharedAppConfig::load(&tr.roost_config).unwrap();
    assert!(!config.profiles.contains_key("default"));
    assert!(!tr.roost_dir.join("default").exists());
}

#[test]
fn test_profile_delete_active_fails() {
    let tr = TestRoost::new();
    tr.init_minimal();

    tr.cmd()
        .args(["profile", "add", "work", "--empty"])
        .assert()
        .success();

    tr.cmd()
        .args(["profile", "delete", "default"])
        .write_stdin("y\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("active"));
}

#[test]
fn test_profile_delete_only_profile_fails() {
    let tr = TestRoost::new();
    tr.init_minimal();

    tr.cmd()
        .args(["profile", "delete", "default"])
        .write_stdin("y\n")
        .assert()
        .failure();
}

#[test]
fn test_profile_delete_with_apps() {
    let tr = TestRoost::new();
    tr.init_minimal();

    add_test_app(&tr, "myapp");

    tr.cmd()
        .args(["profile", "add", "work", "--empty"])
        .assert()
        .success();

    tr.cmd()
        .args(["profile", "switch", "work"])
        .assert()
        .success();

    tr.cmd()
        .args(["profile", "delete", "default"])
        .write_stdin("y\n")
        .assert()
        .success();

    let config = roost::app::SharedAppConfig::load(&tr.roost_config).unwrap();
    assert!(!config.apps.contains_key("myapp"));
}

#[test]
fn test_profile_rename() {
    let tr = TestRoost::new();
    tr.init_minimal();

    tr.cmd()
        .args(["profile", "add", "work", "--empty"])
        .assert()
        .success();

    tr.cmd()
        .args(["profile", "rename", "work", "personal"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Renamed profile 'work' to 'personal'",
        ));

    let config = roost::app::SharedAppConfig::load(&tr.roost_config).unwrap();
    assert!(!config.profiles.contains_key("work"));
    assert!(config.profiles.contains_key("personal"));
    assert!(!tr.roost_dir.join("work").exists());
    assert!(tr.roost_dir.join("personal").exists());
}

#[test]
fn test_profile_rename_nonexistent_fails() {
    let tr = TestRoost::new();
    tr.init_minimal();

    tr.cmd()
        .args(["profile", "rename", "nonexistent", "other"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
}

#[test]
fn test_profile_rename_same_name_fails() {
    let tr = TestRoost::new();
    tr.init_minimal();

    tr.cmd()
        .args(["profile", "rename", "default", "default"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}
