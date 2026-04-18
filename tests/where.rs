mod helpers;

use helpers::TestRoost;
use predicates::str::contains;

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
fn test_where_existing_app() {
    let roost = setup_with_app();

    roost
        .cmd()
        .args(["where", "nvim"])
        .assert()
        .success()
        .stdout(contains("link path"));

    let _config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    let local = roost::app::LocalAppConfig::load(&roost.local_config).unwrap();
    assert!(
        local.link_paths.contains_key("nvim"),
        "nvim should have a link_path entry"
    );
    let link = roost.path(".config/nvim");
    assert!(link.is_symlink(), "nvim should be a symlink");
    assert!(link.exists(), "symlink should be valid");
}

#[test]
fn test_where_nonexistent_app_fails() {
    let roost = TestRoost::new();
    roost.init_minimal();

    roost
        .cmd()
        .args(["where", "nonexistent"])
        .assert()
        .failure()
        .stderr(contains("not managed"));
}

#[test]
fn test_where_no_args_fails() {
    let roost = TestRoost::new();
    roost.init_minimal();

    roost
        .cmd()
        .arg("where")
        .assert()
        .failure()
        .stderr(contains("Usage"));
}
