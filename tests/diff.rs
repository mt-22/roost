mod helpers;

#[test]
fn test_diff_clean() {
    let roost = helpers::TestRoost::new();
    roost.init_minimal();
    roost.init_git();

    roost
        .cmd()
        .arg("diff")
        .assert()
        .success()
        .stdout(predicates::str::contains("No uncommitted changes"));
}

#[test]
fn test_diff_dirty_shows_changes() {
    let roost = helpers::TestRoost::new();
    roost.init_minimal();
    roost.init_git();

    std::fs::write(&roost.roost_config, "modified content\n").unwrap();

    let output = roost
        .cmd()
        .arg("diff")
        .env("PAGER", "cat")
        .assert()
        .success()
        .get_output()
        .clone();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("modified content") || stdout.contains("roost.toml"),
        "diff output should reference the changed file"
    );
}

#[test]
fn test_diff_not_initialized_fails() {
    let roost = helpers::TestRoost::new();

    roost.cmd().arg("diff").assert().failure();
}
