mod helpers;

fn setup_app(roost: &helpers::TestRoost, app_name: &str) -> std::path::PathBuf {
    let rel = format!(".config/{}", app_name);
    let app_dir = roost.path(&rel);
    std::fs::create_dir_all(&app_dir).unwrap();
    std::fs::write(app_dir.join("config.toml"), "data").unwrap();
    roost.cmd().arg("add").arg(&app_dir).assert().success();
    app_dir
}

#[test]
fn test_remove_app() {
    let roost = helpers::TestRoost::new();
    roost.init_minimal();
    let app_dir = setup_app(&roost, "nvim");

    roost
        .cmd()
        .arg("remove")
        .arg("nvim")
        .write_stdin("y\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("Removed 'nvim'"));

    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(!config.apps.contains_key("nvim"));

    assert!(!app_dir.is_symlink());
    assert!(app_dir.join("config.toml").exists());
}

#[test]
fn test_remove_aborted() {
    let roost = helpers::TestRoost::new();
    roost.init_minimal();
    setup_app(&roost, "nvim");

    roost
        .cmd()
        .arg("remove")
        .arg("nvim")
        .write_stdin("n\n")
        .assert()
        .success()
        .stderr(predicates::str::contains("Aborted"));

    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(config.apps.contains_key("nvim"));
}

#[test]
fn test_remove_nonexistent_app_fails() {
    let roost = helpers::TestRoost::new();
    roost.init_minimal();

    roost
        .cmd()
        .arg("remove")
        .arg("nonexistent")
        .assert()
        .failure()
        .stderr(predicates::str::contains("not managed"));
}

#[test]
fn test_remove_no_args_fails() {
    let roost = helpers::TestRoost::new();
    roost.init_minimal();

    roost
        .cmd()
        .arg("remove")
        .assert()
        .failure()
        .stderr(predicates::str::contains("Usage"));
}

#[test]
fn test_remove_creates_git_commit() {
    let roost = helpers::TestRoost::new();
    roost.init_minimal();
    roost.init_git();
    setup_app(&roost, "nvim");

    roost
        .cmd()
        .arg("remove")
        .arg("nvim")
        .write_stdin("y\n")
        .assert()
        .success();

    let output = std::process::Command::new("git")
        .args(["log", "--oneline", "-1"])
        .current_dir(&roost.roost_dir)
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&output.stdout);
    assert!(
        log.to_lowercase().contains("removed"),
        "expected git log to mention 'removed', got: {}",
        log.trim()
    );
}
