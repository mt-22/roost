mod helpers;

use helpers::TestRoost;
use predicates::str::contains;

fn setup() -> TestRoost {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost
}

#[test]
fn test_remote_show_none() {
    let roost = setup();

    roost
        .cmd()
        .arg("remote")
        .assert()
        .success()
        .stdout(contains("No remote configured"));
}

#[test]
fn test_remote_set() {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost.init_git();

    roost
        .cmd()
        .args(["remote", "set", "https://github.com/example/dots"])
        .assert()
        .success()
        .stdout(contains("Remote set to"));

    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert_eq!(
        config.remote,
        Some("https://github.com/example/dots".to_string())
    );

    let git_output = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(&roost.roost_dir)
        .output()
        .expect("git remote get-url failed");
    let url = String::from_utf8_lossy(&git_output.stdout);
    assert_eq!(url.trim(), "https://github.com/example/dots");
}

#[test]
fn test_remote_unknown_subcommand_fails() {
    let roost = setup();

    roost
        .cmd()
        .args(["remote", "unknown"])
        .assert()
        .failure()
        .stderr(contains("Unknown remote command"));
}

#[test]
fn test_remote_set_no_url_fails() {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost.init_git();

    roost
        .cmd()
        .args(["remote", "set"])
        .assert()
        .failure()
        .stderr(contains("Usage"));
}
