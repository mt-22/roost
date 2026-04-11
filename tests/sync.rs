mod helpers;

use helpers::TestRoost;
use predicates::str::contains;
use std::fs;

fn setup() -> TestRoost {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost.init_git();
    std::process::Command::new("git")
        .args(["config", "user.name", "test"])
        .current_dir(&roost.roost_dir)
        .output()
        .expect("failed to set git user.name");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&roost.roost_dir)
        .output()
        .expect("failed to set git user.email");
    roost
}

#[test]
fn test_sync_with_local_remote() {
    let roost = setup();

    let remote_dir = tempfile::TempDir::new().unwrap();
    std::process::Command::new("git")
        .args(["init", "--bare"])
        .current_dir(remote_dir.path())
        .output()
        .expect("failed to init bare repo");

    let remote_path = remote_dir.path().to_str().unwrap();

    roost
        .cmd()
        .args(["remote", "set", remote_path])
        .assert()
        .success();

    let nvim_dir = roost.path(".config/nvim");
    fs::create_dir_all(&nvim_dir).unwrap();
    fs::write(nvim_dir.join("init.lua"), "vim.opt.number = true").unwrap();

    roost
        .cmd()
        .args(["add", nvim_dir.to_str().unwrap()])
        .assert()
        .success();

    roost
        .cmd()
        .arg("sync")
        .assert()
        .success()
        .stdout(contains("Sync complete"));
}

#[test]
fn test_sync_not_initialized_fails() {
    let roost = TestRoost::new();
    roost.cmd().arg("sync").assert().failure();
}

#[test]
fn test_sync_no_remote_fails() {
    let roost = setup();
    roost.cmd().arg("sync").assert().failure();
}
