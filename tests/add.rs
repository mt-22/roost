mod helpers;

use helpers::TestRoost;
use predicates::str::contains;
use std::fs;

fn setup() -> TestRoost {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost
}

#[test]
fn test_add_directory() {
    let roost = setup();

    let nvim_dir = roost.path(".config/nvim");
    std::fs::create_dir_all(&nvim_dir).unwrap();
    std::fs::write(nvim_dir.join("init.lua"), "vim.cmd('echo hi')").unwrap();

    roost
        .cmd()
        .args(["add", nvim_dir.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("Added 'nvim'"));

    assert!(nvim_dir.is_symlink(), "original should now be a symlink");

    let target = fs::read_link(&nvim_dir).unwrap();
    assert_eq!(target, roost.roost_dir.join("default").join("nvim"));

    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(config.apps.contains_key("nvim"));
    assert!(config
        .profiles
        .get("default")
        .unwrap()
        .apps
        .contains("nvim"));

    let local = roost::app::LocalAppConfig::load(&roost.local_config).unwrap();
    assert!(local.link_paths.contains_key("nvim"));
}

#[test]
fn test_add_file() {
    let roost = setup();

    let bashrc = roost.path(".bashrc");
    std::fs::write(&bashrc, "export PATH=$HOME/bin:$PATH").unwrap();

    roost
        .cmd()
        .args(["add", bashrc.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("Added '.bashrc'"));

    assert!(bashrc.is_symlink(), "original file should now be a symlink");

    let target = fs::read_link(&bashrc).unwrap();
    assert!(
        target.to_string_lossy().contains("misc"),
        "single file should be in misc/"
    );

    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(config.apps.contains_key(".bashrc"));

    let local = roost::app::LocalAppConfig::load(&roost.local_config).unwrap();
    assert!(local.link_paths.contains_key(".bashrc"));
}

#[test]
fn test_add_nonexistent_path_fails() {
    let roost = setup();

    roost
        .cmd()
        .args(["add", "/nonexistent/path/that/does/not/exist"])
        .assert()
        .failure();
}

#[test]
fn test_add_no_args_fails() {
    let roost = setup();

    roost.cmd().arg("add").assert().failure();
}

#[test]
fn test_add_not_initialized_fails() {
    let roost = TestRoost::new();

    let some_dir = roost.path(".config/someapp");
    std::fs::create_dir_all(&some_dir).unwrap();

    roost
        .cmd()
        .args(["add", some_dir.to_str().unwrap()])
        .assert()
        .failure();
}

#[test]
fn test_add_directory_with_nested_git() {
    let roost = setup();

    let myapp = roost.path(".config/myapp");
    std::fs::create_dir_all(myapp.join(".git/objects")).unwrap();
    std::fs::write(myapp.join(".git/HEAD"), "ref: refs/heads/main").unwrap();
    std::fs::write(myapp.join("config.toml"), "key = value").unwrap();

    roost
        .cmd()
        .args(["add", myapp.to_str().unwrap()])
        .assert()
        .success();

    let profile_dir = roost.roost_dir.join("default").join("myapp");
    assert!(profile_dir.join("config.toml").exists());
    assert!(
        !profile_dir.join(".git").exists(),
        "nested .git should be removed from roost store"
    );
}

#[test]
fn test_add_creates_git_commit() {
    let roost = setup();
    roost.init_git();

    let nvim_dir = roost.path(".config/nvim");
    std::fs::create_dir_all(&nvim_dir).unwrap();
    std::fs::write(nvim_dir.join("init.lua"), "vim.opt.number = true").unwrap();

    roost
        .cmd()
        .args(["add", nvim_dir.to_str().unwrap()])
        .assert()
        .success();

    let entries = roost::git::log(&roost.roost_dir, 10).unwrap();
    assert!(
        entries.iter().any(|e| e.message.contains("added app")),
        "expected a commit message containing 'added app', got: {:?}",
        entries.iter().map(|e| &e.message).collect::<Vec<_>>()
    );
}
