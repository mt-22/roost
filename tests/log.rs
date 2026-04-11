mod helpers;

#[test]
fn test_log_empty_repo() {
    let roost = helpers::TestRoost::new();
    roost.init_minimal();
    roost.init_git();

    roost
        .cmd()
        .arg("log")
        .assert()
        .success()
        .stdout(predicates::str::contains("init"));
}

#[test]
fn test_log_with_commits() {
    let roost = helpers::TestRoost::new();
    roost.init_minimal();
    roost.init_git();

    let app_dir = roost.path(".config/nvim");
    std::fs::create_dir_all(&app_dir).unwrap();
    std::fs::write(app_dir.join("config.toml"), "data").unwrap();
    roost.cmd().arg("add").arg(&app_dir).assert().success();

    roost
        .cmd()
        .arg("log")
        .assert()
        .success()
        .stdout(predicates::str::contains("added app"));
}

#[test]
fn test_log_not_initialized_fails() {
    let roost = helpers::TestRoost::new();

    roost.cmd().arg("log").assert().failure();
}
