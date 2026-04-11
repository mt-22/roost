mod helpers;

#[test]
fn test_help() {
    let roost = helpers::TestRoost::new();
    let output = roost
        .cmd()
        .arg("help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8_lossy(&output);
    for kw in ["roost", "init", "sync", "add", "remove", "doctor"] {
        assert!(stdout.contains(kw), "expected '{}' in help output", kw);
    }
}

#[test]
fn test_help_flag() {
    let roost = helpers::TestRoost::new();

    roost
        .cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Usage"));
}

#[test]
fn test_help_short_flag() {
    let roost = helpers::TestRoost::new();

    roost
        .cmd()
        .arg("-h")
        .assert()
        .success()
        .stdout(predicates::str::contains("Usage"));
}

#[test]
fn test_unknown_command_fails() {
    let roost = helpers::TestRoost::new();

    roost.cmd().arg("nonexistent").assert().failure();
}

#[test]
fn test_unknown_option_fails() {
    let roost = helpers::TestRoost::new();

    roost.cmd().arg("--nonexistent").assert().failure();
}
