mod helpers;

use helpers::TestRoost;
use std::fs;

fn setup() -> TestRoost {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost
}

#[test]
fn test_adopt_creates_missing_app_entries() {
    let roost = setup();

    // Add nvim to default profile normally
    let nvim = roost.path(".config/nvim");
    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "config").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    // Remove nvim from config.apps (simulates bad merge)
    let mut config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    config.apps.remove("nvim");
    config.save(&roost.roost_config).unwrap();

    // Files still exist in profile dir
    assert!(
        roost.roost_dir.join("default/nvim").exists(),
        "nvim files should still be in profile dir"
    );

    // Run adopt
    roost.cmd().arg("adopt").assert().success();

    // Verify nvim is back in config.apps
    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(
        config.apps.contains_key("nvim"),
        "adopt should recreate nvim entry"
    );
    assert!(
        config.profiles["default"].apps.contains("nvim"),
        "profile should still reference nvim"
    );
}

#[test]
fn test_adopt_creates_entries_for_orphaned_files() {
    let roost = setup();

    // Create a file in the profile dir that has no config entry at all
    fs::create_dir_all(roost.roost_dir.join("default/ghostty")).unwrap();
    fs::write(roost.roost_dir.join("default/ghostty/config"), "test").unwrap();

    // Add it to the profile's apps set (simulating a partial sync)
    let mut config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    config
        .profiles
        .get_mut("default")
        .unwrap()
        .apps
        .insert("ghostty".to_string());
    config.save(&roost.roost_config).unwrap();

    // Run adopt
    roost.cmd().arg("adopt").assert().success();

    // Verify ghostty entry was created
    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(
        config.apps.contains_key("ghostty"),
        "adopt should create ghostty entry"
    );
    assert!(
        config.apps["ghostty"]
            .on_profiles
            .contains(&"default".to_string()),
        "ghostty should list default in on_profiles"
    );
}
