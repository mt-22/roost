mod helpers;

use std::fs;

use helpers::TestRoost;
use predicates::str::contains;

fn setup() -> TestRoost {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost
}

fn add_app(roost: &TestRoost, app_name: &str) {
    let rel = format!(".config/{}", app_name);
    let app_dir = roost.path(&rel);
    std::fs::create_dir_all(&app_dir).unwrap();
    std::fs::write(app_dir.join("config.toml"), "data").unwrap();
    roost
        .cmd()
        .args(["add", app_dir.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn test_status_after_init() {
    let roost = setup();

    roost
        .cmd()
        .arg("status")
        .assert()
        .success()
        .stdout(contains("Profile: default"))
        .stdout(contains("Apps managed: 0"));
}

#[test]
fn test_status_with_apps() {
    let roost = setup();
    add_app(&roost, "nvim");

    roost
        .cmd()
        .arg("status")
        .assert()
        .success()
        .stdout(contains("Apps managed: 1"))
        .stdout(contains("nvim"))
        .stdout(contains("[linked]"));
}

#[test]
fn test_status_not_initialized_fails() {
    let roost = TestRoost::new();

    roost.cmd().arg("status").assert().failure();
}

#[test]
fn test_status_shows_broken_symlink() {
    let roost = TestRoost::new();
    roost.init_minimal();

    let app_dir = roost.path(".config/nvim");
    std::fs::create_dir_all(&app_dir).unwrap();
    std::fs::write(app_dir.join("init.lua"), "config").unwrap();
    roost.cmd().arg("add").arg(&app_dir).assert().success();

    fs::remove_dir_all(roost.roost_dir.join("default").join("nvim")).unwrap();

    let output = roost
        .cmd()
        .arg("status")
        .assert()
        .success()
        .get_output()
        .clone();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("broken symlink"),
        "status should warn about broken symlinks"
    );
}
