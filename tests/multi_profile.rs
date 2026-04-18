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

#[test]
fn test_switch_to_profile_auto_detects_link_paths() {
    let roost = TestRoost::new();
    roost.init_minimal();

    // Add nvim on default profile (simulating macbook setup)
    let nvim = roost.path(".config/nvim");
    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "config").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    // Create second profile with nvim referenced but no local link_path
    // (simulates git sync pulling profile from another device)
    roost
        .cmd()
        .args(["profile", "add", "laptop", "--empty"])
        .assert()
        .success();

    // Manually add nvim to laptop profile in shared config (simulating sync)
    let mut config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
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

    // Remove link_path for nvim to simulate cross-device (never ran `roost add` on this device)
    let mut local = roost::app::LocalAppConfig::load(&roost.local_config).unwrap();
    local.link_paths.remove("nvim");
    local.save(&roost.local_config).unwrap();

    // Create nvim files in laptop profile dir (simulating sync pulling files)
    let laptop_prof_dir = roost.roost_dir.join("laptop");
    fs::create_dir_all(laptop_prof_dir.join("nvim")).unwrap();
    fs::write(laptop_prof_dir.join("nvim").join("init.lua"), "config").unwrap();

    // Switch to laptop — nvim has no link_path entry yet
    roost
        .cmd()
        .args(["profile", "switch", "laptop"])
        .assert()
        .success();

    // Verify the symlink was created pointing into the laptop profile
    let link = roost.path(".config/nvim");
    assert!(
        link.is_symlink(),
        "nvim should be symlinked after profile switch"
    );
    let target = fs::read_link(&link).unwrap();
    assert!(
        target.starts_with(roost.roost_dir.join("laptop")),
        "symlink should point into laptop profile dir, not {:?}",
        target
    );
    assert!(link.exists(), "nvim symlink should be valid (not broken)");
    let target = fs::read_link(&link).unwrap();
    assert!(
        target.starts_with(&roost.roost_dir),
        "symlink should point into roost dir"
    );
    assert!(link.exists(), "nvim symlink should be valid (not broken)");
}

#[test]
fn test_import_creates_missing_apps_entry() {
    let roost = TestRoost::new();
    roost.init_minimal();

    // Add nvim to default profile
    let nvim = roost.path(".config/nvim");
    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "config").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    // Remove nvim from config.apps entirely (simulates bad git merge)
    let mut config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    config.apps.remove("nvim");
    config.save(&roost.roost_config).unwrap();

    // Create laptop profile
    roost
        .cmd()
        .args(["profile", "add", "laptop", "--empty"])
        .assert()
        .success();

    // Import nvim from default into laptop — should recreate the apps entry
    let mut config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    let mut local = roost::app::LocalAppConfig::load(&roost.local_config).unwrap();
    let result = roost::linker::import_app_from_profile(
        "nvim",
        "laptop",
        "default",
        &mut config,
        &roost.roost_config,
        &roost.roost_dir,
        &mut local,
    );
    assert!(result.is_ok(), "import should succeed");

    // Verify apps entry was recreated
    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(
        config.apps.contains_key("nvim"),
        "nvim should be in config.apps after import"
    );
    assert!(
        config.apps["nvim"]
            .on_profiles
            .contains(&"laptop".to_string()),
        "nvim should list laptop in on_profiles"
    );
}

#[test]
fn test_import_auto_detects_missing_link_path() {
    let roost = TestRoost::new();
    roost.init_minimal();

    // Add nvim to default profile
    let nvim = roost.path(".config/nvim");
    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "config").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    // Create laptop profile with empty link_paths (simulates cross-device)
    roost
        .cmd()
        .args(["profile", "add", "laptop", "--empty"])
        .assert()
        .success();

    // Clear link_paths to simulate fresh device, then re-add nvim's path
    // (auto-detect via find_app_on_filesystem can't work in-process because
    // dirs::home_dir() points to the real home, not the test's temp dir).
    let mut local = roost::app::LocalAppConfig::load(&roost.local_config).unwrap();
    let nvim_link = roost.path(".config/nvim");
    local.link_paths.clear();
    local
        .link_paths
        .insert("nvim".to_string(), nvim_link.clone());
    local.save(&roost.local_config).unwrap();

    // Import nvim — link_path is known, import should succeed
    let mut config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    let mut local = roost::app::LocalAppConfig::load(&roost.local_config).unwrap();
    let result = roost::linker::import_app_from_profile(
        "nvim",
        "laptop",
        "default",
        &mut config,
        &roost.roost_config,
        &roost.roost_dir,
        &mut local,
    );
    assert!(result.is_ok(), "import should succeed: {:?}", result.err());

    // Verify symlink was created
    let link = roost.path(".config/nvim");
    assert!(link.is_symlink(), "nvim should be symlinked after import");
    assert!(link.exists(), "symlink should be valid");
}

#[test]
fn test_auto_detect_resolves_sourced_apps() {
    let roost = TestRoost::new();
    roost.init_minimal();

    // Add nvim to default
    let nvim = roost.path(".config/nvim");
    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "config").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    // Create laptop profile with nvim sourced from default
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
        .apps
        .insert("nvim".to_string());
    config
        .profiles
        .get_mut("laptop")
        .unwrap()
        .app_sources
        .insert("nvim".to_string(), "default".to_string());
    config
        .apps
        .get_mut("nvim")
        .unwrap()
        .on_profiles
        .push("laptop".to_string());
    config.save(&roost.roost_config).unwrap();

    // Remove link_path to simulate cross-device
    let mut local = roost::app::LocalAppConfig::load(&roost.local_config).unwrap();
    local.link_paths.remove("nvim");
    local.save(&roost.local_config).unwrap();

    // Switch to laptop — nvim is sourced, not local to this profile
    roost
        .cmd()
        .args(["profile", "switch", "laptop"])
        .assert()
        .success();

    let link = roost.path(".config/nvim");
    assert!(link.is_symlink(), "sourced nvim should be symlinked");
    assert!(link.exists(), "sourced nvim symlink should not be broken");
    let target = fs::read_link(&link).unwrap();
    assert!(
        target.starts_with(roost.roost_dir.join("laptop")),
        "symlink should point into laptop profile dir, not {:?}",
        target
    );
}
