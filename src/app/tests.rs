use super::*;
use std::collections::{BTreeMap, BTreeSet};
use tempfile::TempDir;

fn make_shared_config() -> SharedAppConfig {
    let mut apps = BTreeMap::new();
    apps.insert(
        "nvim".into(),
        Application {
            primary_config: Some(PathBuf::from("init.lua")),
            on_profiles: ["laptop".into()].into(),
            is_dir: true,
        },
    );
    apps.insert(
        "git".into(),
        Application {
            primary_config: None,
            on_profiles: ["laptop".into()].into(),
            is_dir: false,
        },
    );

    let mut profiles = BTreeMap::new();
    profiles.insert(
        "laptop".into(),
        Profile {
            apps: ["nvim".into(), "git".into()].into(),
            app_sources: BTreeMap::new(),
        },
    );

    SharedAppConfig {
        remote: Some("git@github.com:user/dotfiles".into()),
        profiles,
        apps,
        ignored: [".DS_Store".into(), "*.log".into()].into(),
    }
}

fn make_local_config() -> LocalAppConfig {
    LocalAppConfig {
        active_profile: "laptop".into(),
        os_info: OsInfo {
            os: "macos".into(),
            arch: "aarch64".into(),
        },
        link_paths: {
            let mut m = BTreeMap::new();
            m.insert("nvim".into(), PathBuf::from("/Users/test/.config/nvim"));
            m
        },
    }
}

#[test]
fn shared_config_round_trip() {
    let config = make_shared_config();
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("roost.toml");

    save_shared(&path, &config).unwrap();
    let loaded = load_shared(&path).unwrap();

    assert_eq!(loaded.remote, config.remote);
    assert_eq!(loaded.ignored, config.ignored);
    assert!(loaded.profiles.contains_key("laptop"));
    assert!(loaded.apps.contains_key("nvim"));
    assert_eq!(loaded.apps["nvim"].is_dir, true);
    assert_eq!(loaded.apps["git"].is_dir, false);
    assert_eq!(
        loaded.apps["nvim"].primary_config,
        Some(PathBuf::from("init.lua"))
    );
}

#[test]
fn local_config_round_trip() {
    let config = make_local_config();
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("local.toml");

    save_local(&path, &config).unwrap();
    let loaded = load_local(&path).unwrap();

    assert_eq!(loaded.active_profile, "laptop");
    assert_eq!(loaded.os_info.os, "macos");
    assert_eq!(loaded.os_info.arch, "aarch64");
    assert!(loaded.link_paths.contains_key("nvim"));
}

#[test]
fn empty_config_round_trip() {
    let config = SharedAppConfig {
        remote: None,
        profiles: BTreeMap::new(),
        apps: BTreeMap::new(),
        ignored: BTreeSet::new(),
    };
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("roost.toml");

    save_shared(&path, &config).unwrap();
    let loaded = load_shared(&path).unwrap();

    assert!(loaded.remote.is_none());
    assert!(loaded.profiles.is_empty());
    assert!(loaded.apps.is_empty());
    assert!(loaded.ignored.is_empty());
}

#[test]
fn validate_rejects_unknown_app_in_profile() {
    let mut config = make_shared_config();
    config
        .profiles
        .get_mut("laptop")
        .unwrap()
        .apps
        .insert("nonexistent".into());
    let result = validate_shared(&config);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("unknown app 'nonexistent'"));
}

#[test]
fn validate_rejects_unknown_profile_in_app() {
    let mut config = make_shared_config();
    config
        .apps
        .get_mut("nvim")
        .unwrap()
        .on_profiles
        .insert("nonexistent".into());
    let result = validate_shared(&config);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("unknown profile 'nonexistent'"));
}

#[test]
fn validate_rejects_cycle_in_app_sources() {
    let mut config = make_shared_config();
    config.profiles.insert(
        "shared".into(),
        Profile {
            apps: ["nvim".into()].into(),
            app_sources: BTreeMap::new(),
        },
    );
    config
        .profiles
        .get_mut("laptop")
        .unwrap()
        .app_sources
        .insert("nvim".into(), "shared".into());
    config
        .profiles
        .get_mut("shared")
        .unwrap()
        .app_sources
        .insert("nvim".into(), "laptop".into());

    let result = validate_shared(&config);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("cycle detected"));
}

#[test]
fn validate_rejects_unknown_source_profile() {
    let mut config = make_shared_config();
    config
        .profiles
        .get_mut("laptop")
        .unwrap()
        .app_sources
        .insert("nvim".into(), "ghost".into());
    let result = validate_shared(&config);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("unknown profile 'ghost'"));
}

#[test]
fn validate_accepts_valid_config() {
    let config = make_shared_config();
    assert!(validate_shared(&config).is_ok());
}

#[test]
fn roost_dir_respects_env_var() {
    unsafe {
        std::env::set_var("ROOST_DIR", "/tmp/test-roost");
    }
    let dir = roost_dir();
    unsafe {
        std::env::remove_var("ROOST_DIR");
    }
    assert_eq!(dir, PathBuf::from("/tmp/test-roost"));
}
