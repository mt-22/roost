mod helpers;
use helpers::*;
use predicates::str::contains;
use std::fs;

#[test]
fn test_rollback_to_commit() {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost.init_git();

    let nvim = roost.path(".config/nvim");
    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "v1").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    let ghostty = roost.path(".config/ghostty");
    fs::create_dir_all(&ghostty).unwrap();
    fs::write(ghostty.join("config"), "dark").unwrap();
    roost.cmd().arg("add").arg(&ghostty).assert().success();

    let entries = roost::git::log(&roost.roost_dir, 10).unwrap();
    let nvim_commit = entries.iter().find(|e| e.message.contains("nvim")).unwrap();
    let hash = nvim_commit.hash.clone();

    roost
        .cmd()
        .args(["rollback", &hash])
        .write_stdin("y\n")
        .assert()
        .success()
        .stdout(contains("Rolled back"));

    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(
        config.apps.contains_key("nvim"),
        "nvim should exist at rollback point"
    );
    assert!(
        !config.apps.contains_key("ghostty"),
        "ghostty should not exist at rollback point"
    );
}

#[test]
fn test_rollback_aborted() {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost.init_git();

    let app_dir = roost.path(".config/nvim");
    fs::create_dir_all(&app_dir).unwrap();
    fs::write(app_dir.join("init.lua"), "config").unwrap();
    roost.cmd().arg("add").arg(&app_dir).assert().success();

    let entries = roost::git::log(&roost.roost_dir, 10).unwrap();
    let hash = entries.last().unwrap().hash.clone();

    roost
        .cmd()
        .args(["rollback", &hash])
        .write_stdin("n\n")
        .assert()
        .success()
        .stderr(contains("Aborted"));

    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(config.apps.contains_key("nvim"), "no change after abort");
}

#[test]
fn test_rollback_no_hash_fails() {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost.cmd().arg("rollback").assert().failure();
}

#[test]
fn test_rollback_not_initialized_fails() {
    let roost = TestRoost::new();
    roost.cmd().args(["rollback", "abc123"]).assert().failure();
}
