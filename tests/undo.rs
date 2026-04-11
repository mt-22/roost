mod helpers;
use helpers::*;
use predicates::str::contains;
use std::fs;

#[test]
fn test_undo_reverts_last_commit() {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost.init_git();

    let app_dir = roost.path(".config/nvim");
    fs::create_dir_all(&app_dir).unwrap();
    fs::write(app_dir.join("init.lua"), "config").unwrap();
    roost.cmd().arg("add").arg(&app_dir).assert().success();

    roost
        .cmd()
        .arg("undo")
        .write_stdin("y\n")
        .assert()
        .success()
        .stdout(contains("Undone"));

    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(
        !config.apps.contains_key("nvim"),
        "app should be gone after undo"
    );
}

#[test]
fn test_undo_multiple_commits() {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost.init_git();

    for name in ["nvim", "ghostty"] {
        let dir = roost.path(&format!(".config/{}", name));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("config.toml"), name).unwrap();
        roost.cmd().arg("add").arg(&dir).assert().success();
    }

    roost
        .cmd()
        .args(["undo", "2"])
        .write_stdin("y\n")
        .assert()
        .success();

    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(!config.apps.contains_key("nvim"));
    assert!(!config.apps.contains_key("ghostty"));
}

#[test]
fn test_undo_aborted() {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost.init_git();

    let app_dir = roost.path(".config/nvim");
    fs::create_dir_all(&app_dir).unwrap();
    fs::write(app_dir.join("init.lua"), "config").unwrap();
    roost.cmd().arg("add").arg(&app_dir).assert().success();

    roost
        .cmd()
        .arg("undo")
        .write_stdin("n\n")
        .assert()
        .success()
        .stderr(contains("Aborted"));

    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(
        config.apps.contains_key("nvim"),
        "app should still exist after abort"
    );
}

#[test]
fn test_undo_not_enough_commits_fails() {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost.init_git();

    roost
        .cmd()
        .args(["undo", "5"])
        .write_stdin("y\n")
        .assert()
        .failure()
        .stderr(contains("Not enough commits"));
}

#[test]
fn test_undo_not_initialized_fails() {
    let roost = TestRoost::new();
    roost
        .cmd()
        .arg("undo")
        .write_stdin("y\n")
        .assert()
        .failure();
}

#[test]
fn test_undo_restores_config_to_previous_state() {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost.init_git();

    let app_dir = roost.path(".config/nvim");
    fs::create_dir_all(&app_dir).unwrap();
    fs::write(app_dir.join("init.lua"), "config").unwrap();
    roost.cmd().arg("add").arg(&app_dir).assert().success();

    roost
        .cmd()
        .arg("undo")
        .write_stdin("y\n")
        .assert()
        .success();

    let _ = fs::remove_file(&app_dir);
    let _ = fs::remove_dir_all(&app_dir);
    fs::create_dir_all(&app_dir).unwrap();
    fs::write(app_dir.join("init.lua"), "config").unwrap();
    roost.cmd().arg("add").arg(&app_dir).assert().success();
}
