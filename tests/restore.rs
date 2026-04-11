mod helpers;

use helpers::TestRoost;
use predicates::str::contains;
use std::fs;

fn setup_with_app() -> TestRoost {
    let roost = TestRoost::new();
    roost.init_minimal();

    let nvim_dir = roost.path(".config/nvim");
    std::fs::create_dir_all(&nvim_dir).unwrap();
    std::fs::write(nvim_dir.join("init.lua"), "vim.cmd('echo hi')").unwrap();
    roost
        .cmd()
        .args(["add", nvim_dir.to_str().unwrap()])
        .assert()
        .success();

    roost
}

#[test]
fn test_restore_repair_broken_symlink() {
    let roost = setup_with_app();

    let nvim_dir = roost.path(".config/nvim");
    assert!(nvim_dir.is_symlink(), "should be a symlink after add");

    std::fs::remove_file(&nvim_dir).unwrap();
    assert!(!nvim_dir.exists(), "symlink should be gone");

    roost
        .cmd()
        .arg("restore")
        .assert()
        .success()
        .stdout(contains("Links restored"));

    assert!(nvim_dir.is_symlink(), "symlink should be recreated");

    let target = fs::read_link(&nvim_dir).unwrap();
    assert_eq!(target, roost.roost_dir.join("default").join("nvim"));

    assert!(
        nvim_dir.join("init.lua").exists(),
        "files should be accessible through symlink"
    );
}

#[test]
fn test_restore_not_initialized_fails() {
    let roost = TestRoost::new();

    roost.cmd().arg("restore").assert().failure();
}

#[test]
fn test_restore_idempotent() {
    let roost = TestRoost::new();
    roost.init_minimal();

    let app_dir = roost.path(".config/nvim");
    fs::create_dir_all(&app_dir).unwrap();
    fs::write(app_dir.join("init.lua"), "config").unwrap();
    roost.cmd().arg("add").arg(&app_dir).assert().success();

    roost.cmd().arg("restore").assert().success();
    assert!(app_dir.is_symlink());

    roost.cmd().arg("restore").assert().success();
    assert!(app_dir.is_symlink());
    let target = fs::read_link(&app_dir).unwrap();
    assert_eq!(target, roost.roost_dir.join("default").join("nvim"));
}

#[test]
fn test_restore_multiple_apps() {
    let roost = TestRoost::new();
    roost.init_minimal();

    for name in &["nvim", "ghostty"] {
        let dir = roost.path(&format!(".config/{}", name));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("config.toml"), name).unwrap();
        roost.cmd().arg("add").arg(&dir).assert().success();
    }

    let nvim = roost.path(".config/nvim");
    let ghostty = roost.path(".config/ghostty");
    fs::remove_file(&nvim).unwrap();
    fs::remove_file(&ghostty).unwrap();

    roost.cmd().arg("restore").assert().success();

    assert!(nvim.is_symlink());
    assert!(ghostty.is_symlink());
}
