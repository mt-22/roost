mod helpers;
use helpers::*;
use std::fs;

#[test]
fn test_add_same_app_twice_to_same_profile() {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost.init_git();

    let app_dir = roost.path(".config/nvim");
    fs::create_dir_all(&app_dir).unwrap();
    fs::write(app_dir.join("init.lua"), "config").unwrap();

    roost.cmd().arg("add").arg(&app_dir).assert().success();

    fs::create_dir_all(&app_dir).unwrap();
    fs::write(app_dir.join("init.lua"), "new config").unwrap();

    roost.cmd().arg("add").arg(&app_dir).assert().success();

    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(config.apps.contains_key("nvim"));
    assert_eq!(config.apps.len(), 1);
}

#[test]
fn test_add_file_that_is_already_symlink_ingests_target() {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost.init_git();

    let target = roost.path("actual_config");
    fs::write(&target, "real data").unwrap();

    let link = roost.path(".config/nvim");
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();

    roost.cmd()
        .arg("add")
        .arg(&link)
        .assert()
        .success();

    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(config.apps.contains_key("actual_config"));
    assert!(link.is_symlink());
}

#[test]
fn test_remove_from_active_profile_removes_from_all() {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost.init_git();

    let nvim = roost.path(".config/nvim");
    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "config").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    roost.cmd().args(["profile", "add", "work", "--empty"]).assert().success();
    roost.cmd().args(["profile", "switch", "work"]).assert().success();

    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "work config").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    let config_before = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(config_before.apps["nvim"].on_profiles.contains(&"default".to_string()));
    assert!(config_before.apps["nvim"].on_profiles.contains(&"work".to_string()));

    roost.cmd().args(["profile", "switch", "default"]).assert().success();

    roost.cmd().arg("remove").arg("nvim").write_stdin("y\n").assert().success();

    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(!config.apps.contains_key("nvim"), "remove deletes app from all profiles");
    assert!(!config.profiles["default"].apps.contains("nvim"));
    assert!(!config.profiles["work"].apps.contains("nvim"));
    assert!(!nvim.is_symlink());
}

#[test]
fn test_doctor_detects_app_in_config_not_in_profile_dir() {
    let roost = TestRoost::new();
    roost.init_minimal();

    let mut config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    config.apps.insert(
        "ghost".to_string(),
        roost::app::Application {
            name: "ghost".to_string(),
            primary_config: None,
            on_profiles: vec!["default".to_string()],
        },
    );
    config.profiles.get_mut("default").unwrap().apps.insert("ghost".to_string());
    config.save(&roost.roost_config).unwrap();

    let local = roost::app::LocalAppConfig::load(&roost.local_config).unwrap();
    let mut local = local;
    local.link_paths.insert("ghost".to_string(), roost.path(".config/ghost"));
    local.save(&roost.local_config).unwrap();

    roost.cmd()
        .arg("doctor")
        .assert()
        .failure();
}

#[test]
fn test_restore_after_manual_config_edit() {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost.init_git();

    let nvim = roost.path(".config/nvim");
    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "v1").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    fs::remove_file(&nvim).unwrap();
    assert!(!nvim.exists());

    roost.cmd().arg("restore").assert().success();
    assert!(nvim.is_symlink());
    assert_eq!(fs::read_to_string(nvim.join("init.lua")).unwrap(), "v1");
}

#[test]
fn test_status_shows_correct_count_after_add_remove() {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost.init_git();

    for name in &["nvim", "ghostty", "zellij"] {
        let dir = roost.path(&format!(".config/{}", name));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("config.toml"), name).unwrap();
        roost.cmd().arg("add").arg(&dir).assert().success();
    }

    roost.cmd()
        .arg("status")
        .assert()
        .stdout(predicates::str::contains("Apps managed: 3"));

    roost.cmd().arg("remove").arg("ghostty").write_stdin("y\n").assert().success();

    roost.cmd()
        .arg("status")
        .assert()
        .stdout(predicates::str::contains("Apps managed: 2"));
}

#[test]
fn test_where_shows_multiple_paths_for_multi_profile_app() {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost.init_git();

    let nvim = roost.path(".config/nvim");
    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "config").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    roost.cmd().args(["profile", "add", "work", "--empty"]).assert().success();
    roost.cmd().args(["profile", "switch", "work"]).assert().success();
    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "work").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    roost.cmd()
        .arg("where")
        .arg("nvim")
        .assert()
        .success();
}

#[test]
fn test_profile_rename_moves_files() {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost.init_git();

    let nvim = roost.path(".config/nvim");
    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    roost.cmd()
        .args(["profile", "rename", "default", "main"])
        .assert()
        .success();

    assert!(roost.roost_dir.join("main/nvim/init.lua").exists());
    assert!(!roost.roost_dir.join("default").exists());
}

#[test]
fn test_full_cycle_add_remove_readd() {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost.init_git();

    let nvim = roost.path(".config/nvim");
    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "v1").unwrap();

    roost.cmd().arg("add").arg(&nvim).assert().success();
    assert!(nvim.is_symlink());

    roost.cmd().arg("remove").arg("nvim").write_stdin("y\n").assert().success();
    assert!(!nvim.is_symlink());
    assert_eq!(fs::read_to_string(nvim.join("init.lua")).unwrap(), "v1");

    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "v2").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();
    assert!(nvim.is_symlink());

    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(config.apps.contains_key("nvim"));
    assert_eq!(
        fs::read_to_string(nvim.join("init.lua")).unwrap(),
        "v2"
    );
}
