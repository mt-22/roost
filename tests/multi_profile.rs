mod helpers;
use helpers::*;
use std::fs;

#[test]
fn test_add_app_to_second_profile() {
    let roost = TestRoost::new();
    roost.init_minimal();

    let nvim = roost.path(".config/nvim");
    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "config").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    roost
        .cmd()
        .args(["profile", "add", "work", "--empty"])
        .assert()
        .success();
    roost
        .cmd()
        .args(["profile", "switch", "work"])
        .assert()
        .success();

    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "work config").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert_eq!(config.apps["nvim"].on_profiles.len(), 2);
    assert!(config.apps["nvim"]
        .on_profiles
        .contains(&"default".to_string()));
    assert!(config.apps["nvim"]
        .on_profiles
        .contains(&"work".to_string()));
    assert!(config.profiles["default"].apps.contains("nvim"));
    assert!(config.profiles["work"].apps.contains("nvim"));
}

#[test]
fn test_remove_app_from_multiple_profiles_removes_all() {
    let roost = TestRoost::new();
    roost.init_minimal();

    let nvim = roost.path(".config/nvim");
    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "original").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    roost
        .cmd()
        .args(["profile", "add", "work", "--empty"])
        .assert()
        .success();
    roost
        .cmd()
        .args(["profile", "switch", "work"])
        .assert()
        .success();
    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "original").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    roost
        .cmd()
        .args(["profile", "switch", "default"])
        .assert()
        .success();

    roost
        .cmd()
        .args(["remove", "nvim"])
        .write_stdin("y\n")
        .assert()
        .success();

    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(
        !config.apps.contains_key("nvim"),
        "app should be fully removed"
    );
    assert!(!config.profiles["default"].apps.contains("nvim"));
    assert!(!config.profiles["work"].apps.contains("nvim"));
}

#[test]
fn test_profile_rename_updates_on_profiles() {
    let roost = TestRoost::new();
    roost.init_minimal();

    let nvim = roost.path(".config/nvim");
    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    roost
        .cmd()
        .args(["profile", "rename", "default", "personal"])
        .assert()
        .success();

    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(
        config.apps["nvim"]
            .on_profiles
            .contains(&"personal".to_string()),
        "app.on_profiles should reference new name"
    );
    assert!(
        !config.apps["nvim"]
            .on_profiles
            .contains(&"default".to_string()),
        "app.on_profiles should not reference old name"
    );
}

#[test]
fn test_profile_rename_updates_active_profile() {
    let roost = TestRoost::new();
    roost.init_minimal();

    roost
        .cmd()
        .args(["profile", "rename", "default", "main"])
        .assert()
        .success();

    let local = roost::app::LocalAppConfig::load(&roost.local_config).unwrap();
    assert_eq!(local.active_profile, "main");
}

#[test]
fn test_profile_rename_updates_app_sources() {
    let roost = TestRoost::new();
    roost.init_minimal();

    let nvim = roost.path(".config/nvim");
    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    roost
        .cmd()
        .args(["profile", "add", "laptop", "--empty"])
        .assert()
        .success();

    let mut config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    config
        .profiles
        .get_mut("laptop")
        .unwrap()
        .app_sources
        .insert("nvim".to_string(), "default".to_string());
    config
        .profiles
        .get_mut("laptop")
        .unwrap()
        .apps
        .insert("nvim".to_string());
    config
        .apps
        .get_mut("nvim")
        .unwrap()
        .on_profiles
        .push("laptop".to_string());
    config.save(&roost.roost_config).unwrap();

    roost
        .cmd()
        .args(["profile", "rename", "default", "personal"])
        .assert()
        .success();

    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert_eq!(
        config.profiles["laptop"].app_sources.get("nvim").unwrap(),
        "personal",
        "app_sources should reference new profile name"
    );
}
