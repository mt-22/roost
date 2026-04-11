mod helpers;

use helpers::TestRoost;
use predicates::str::contains;

fn setup() -> TestRoost {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost
}

fn add_app(roost: &TestRoost, app_name: &str) -> std::path::PathBuf {
    let rel = format!(".config/{}", app_name);
    let app_dir = roost.path(&rel);
    std::fs::create_dir_all(&app_dir).unwrap();
    std::fs::write(app_dir.join("config.toml"), "data").unwrap();
    roost
        .cmd()
        .arg("add")
        .arg(app_dir.to_str().unwrap())
        .assert()
        .success();
    app_dir
}

#[test]
fn test_doctor_clean_state() {
    let roost = setup();

    roost
        .cmd()
        .arg("doctor")
        .assert()
        .success()
        .stdout(contains("All checks passed"));
}

#[test]
fn test_doctor_detects_broken_symlink() {
    let roost = setup();
    add_app(&roost, "nvim");

    std::fs::remove_dir_all(roost.roost_dir.join("default").join("nvim")).unwrap();

    roost
        .cmd()
        .arg("doctor")
        .assert()
        .failure()
        .stderr(contains("broken symlink"));
}

#[test]
fn test_doctor_detects_config_inconsistency() {
    let roost = setup();

    let mut config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    config
        .profiles
        .get_mut("default")
        .unwrap()
        .apps
        .insert("phantom".to_string());
    config.save(&roost.roost_config).unwrap();

    roost
        .cmd()
        .arg("doctor")
        .assert()
        .failure()
        .stderr(contains("phantom"));
}

#[test]
fn test_doctor_not_initialized_fails() {
    let roost = TestRoost::new();

    roost
        .cmd()
        .arg("doctor")
        .assert()
        .failure()
        .stderr(contains("not initialized"));
}

#[test]
fn test_doctor_detects_missing_profile_files() {
    let roost = setup();
    add_app(&roost, "nvim");

    roost
        .cmd()
        .arg("remove")
        .arg("nvim")
        .write_stdin("y\n")
        .assert()
        .success();

    let mut config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    config.apps.insert(
        "nvim".to_string(),
        roost::app::Application {
            name: "nvim".to_string(),
            primary_config: None,
            on_profiles: vec!["default".to_string()],
        },
    );
    config
        .profiles
        .get_mut("default")
        .unwrap()
        .apps
        .insert("nvim".to_string());
    config.save(&roost.roost_config).unwrap();

    let mut local = roost::app::LocalAppConfig::load(&roost.local_config).unwrap();
    local
        .link_paths
        .insert("nvim".to_string(), roost.path(".config/nvim"));
    local.save(&roost.local_config).unwrap();

    roost
        .cmd()
        .arg("doctor")
        .assert()
        .success()
        .stderr(contains("no files in profile"));
}
