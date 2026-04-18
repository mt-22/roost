use super::*;
use std::collections::{HashMap, HashSet};
use tempfile::TempDir;

fn make_shared() -> SharedAppConfig {
    SharedAppConfig {
        remote: None,
        profiles: HashMap::new(),
        apps: HashMap::new(),
        ignored: HashSet::new(),
    }
}

fn make_local() -> LocalAppConfig {
    LocalAppConfig {
        active_profile: "default".to_string(),
        os_info: OsInfo {
            family: "unix".to_string(),
            name: "macos".to_string(),
            version: Some("15.0".to_string()),
            arch: "aarch64".to_string(),
        },
        link_paths: HashMap::new(),
    }
}

fn make_profile() -> Profile {
    Profile {
        apps: HashSet::new(),
        app_sources: HashMap::new(),
    }
}

// ── 1. apps_set dual-format deserialization ────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize)]
struct AppsWrapper {
    #[serde(with = "apps_set")]
    apps: HashSet<String>,
}

#[test]
fn apps_set_deserialize_array_format() {
    let toml = r#"apps = ["nvim", "ghostty"]"#;
    let w: AppsWrapper = toml::from_str(toml).unwrap();
    assert!(w.apps.contains("nvim"));
    assert!(w.apps.contains("ghostty"));
    assert_eq!(w.apps.len(), 2);
}

#[test]
fn apps_set_deserialize_legacy_table_format() {
    let toml = r#"apps = { nvim = "~/.config/nvim", ghostty = "~/.config/ghostty" }"#;
    let w: AppsWrapper = toml::from_str(toml).unwrap();
    assert!(w.apps.contains("nvim"));
    assert!(w.apps.contains("ghostty"));
    assert_eq!(w.apps.len(), 2);
}

#[test]
fn apps_set_deserialize_empty_array() {
    let toml = r#"apps = []"#;
    let w: AppsWrapper = toml::from_str(toml).unwrap();
    assert!(w.apps.is_empty());
}

#[test]
fn apps_set_roundtrip_array_format() {
    let original: HashSet<String> = ["nvim", "ghostty", "bash"]
        .into_iter()
        .map(String::from)
        .collect();
    let serialized = toml::to_string(&AppsWrapper {
        apps: original.clone(),
    })
    .unwrap();
    let deserialized: AppsWrapper = toml::from_str(&serialized).unwrap();
    assert_eq!(original, deserialized.apps);
}

// ── 2. SharedAppConfig serialization roundtrip ─────────────────────────────

#[test]
fn shared_config_roundtrip_full() {
    let mut cfg = make_shared();
    cfg.remote = Some("https://github.com/example/dots".to_string());
    cfg.profiles.insert("default".to_string(), make_profile());
    cfg.ignored.insert("secret".to_string());
    cfg.apps.insert(
        "nvim".to_string(),
        Application {
            name: "nvim".to_string(),
            primary_config: None,
            on_profiles: vec!["default".to_string()],
        },
    );

    let s = toml::to_string(&cfg).unwrap();
    let back: SharedAppConfig = toml::from_str(&s).unwrap();
    assert_eq!(back.remote, cfg.remote);
    assert_eq!(back.profiles.len(), 1);
    assert!(back.profiles.contains_key("default"));
    assert!(back.ignored.contains("secret"));
    assert_eq!(back.apps.len(), 1);
    assert!(back.apps.contains_key("nvim"));
}

#[test]
fn shared_config_roundtrip_empty() {
    let cfg = make_shared();
    let s = toml::to_string(&cfg).unwrap();
    let back: SharedAppConfig = toml::from_str(&s).unwrap();
    assert!(back.remote.is_none());
    assert!(back.profiles.is_empty());
    assert!(back.apps.is_empty());
    assert!(back.ignored.is_empty());
}

// ── 3. LocalAppConfig serialization roundtrip ──────────────────────────────

#[test]
fn local_config_roundtrip_full() {
    let mut cfg = make_local();
    cfg.link_paths.insert(
        "nvim".to_string(),
        dirs::home_dir().unwrap().join(".config/nvim"),
    );

    let s = toml::to_string(&cfg).unwrap();
    let back: LocalAppConfig = toml::from_str(&s).unwrap();
    assert_eq!(back.active_profile, "default");
    assert_eq!(back.os_info.family, "unix");
    assert_eq!(back.link_paths.len(), 1);
    assert!(back.link_paths.contains_key("nvim"));
}

#[test]
fn local_config_empty_link_paths_omitted() {
    let cfg = make_local();
    let s = toml::to_string(&cfg).unwrap();
    assert!(!s.contains("link_paths"));
}

// ── 4. File load/save ──────────────────────────────────────────────────────

#[test]
fn shared_config_save_and_load() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("roost.toml");

    let mut cfg = make_shared();
    cfg.remote = Some("git@host:dots".to_string());
    cfg.profiles.insert("work".to_string(), make_profile());

    cfg.save(&path).unwrap();
    let loaded = SharedAppConfig::load(&path).unwrap();
    assert_eq!(loaded.remote, Some("git@host:dots".to_string()));
    assert!(loaded.profiles.contains_key("work"));
}

#[test]
fn local_config_save_and_load() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("local.toml");

    let mut cfg = make_local();
    cfg.link_paths.insert(
        "bash".to_string(),
        dirs::home_dir().unwrap().join(".bashrc"),
    );

    cfg.save(&path).unwrap();
    let loaded = LocalAppConfig::load(&path).unwrap();
    assert_eq!(loaded.active_profile, "default");
    assert!(loaded.link_paths.contains_key("bash"));
}

#[test]
fn load_nonexistent_file_fails() {
    let result = SharedAppConfig::load(Path::new("/no/such/file.toml"));
    assert!(result.is_err());
}

// ── 5. add_profile tests ───────────────────────────────────────────────────

#[test]
fn add_profile_succeeds() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let shared_path = tmp.path().join("roost.toml");
    let local_path = tmp.path().join("local.toml");

    let mut shared = make_shared();
    shared
        .profiles
        .insert("default".to_string(), make_profile());
    let mut local = make_local();

    shared.save(&shared_path).unwrap();
    local.save(&local_path).unwrap();

    let count = add_profile(
        "work",
        &roost_dir,
        &mut shared,
        &shared_path,
        &mut local,
        &local_path,
        None,
    )
    .unwrap();

    assert_eq!(count, 0);
    assert!(shared.profiles.contains_key("work"));
    assert!(roost_dir.join("work").exists());
}

#[test]
fn add_profile_empty_name_fails() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let shared_path = tmp.path().join("roost.toml");
    let local_path = tmp.path().join("local.toml");

    let mut shared = make_shared();
    let mut local = make_local();
    shared.save(&shared_path).unwrap();
    local.save(&local_path).unwrap();

    let result = add_profile(
        "",
        &roost_dir,
        &mut shared,
        &shared_path,
        &mut local,
        &local_path,
        None,
    );
    assert!(result.is_err());
}

#[test]
fn add_profile_duplicate_name_fails() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let shared_path = tmp.path().join("roost.toml");
    let local_path = tmp.path().join("local.toml");

    let mut shared = make_shared();
    shared
        .profiles
        .insert("default".to_string(), make_profile());
    let mut local = make_local();
    shared.save(&shared_path).unwrap();
    local.save(&local_path).unwrap();

    let result = add_profile(
        "default",
        &roost_dir,
        &mut shared,
        &shared_path,
        &mut local,
        &local_path,
        None,
    );
    assert!(result.is_err());
}

// ── 6. delete_profile tests ────────────────────────────────────────────────

fn setup_two_profiles() -> (
    TempDir,
    SharedAppConfig,
    LocalAppConfig,
    PathBuf,
    PathBuf,
    PathBuf,
) {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let shared_path = tmp.path().join("roost.toml");
    let local_path = tmp.path().join("local.toml");

    fs::create_dir_all(roost_dir.join("default")).unwrap();
    fs::create_dir_all(roost_dir.join("other")).unwrap();

    let mut shared = make_shared();
    shared
        .profiles
        .insert("default".to_string(), make_profile());
    shared.profiles.insert("other".to_string(), make_profile());

    let mut local = make_local();
    local.active_profile = "default".to_string();

    shared.save(&shared_path).unwrap();
    local.save(&local_path).unwrap();

    (tmp, shared, local, roost_dir, shared_path, local_path)
}

#[test]
fn delete_profile_cannot_delete_active() {
    let (_tmp, mut shared, mut local, roost_dir, shared_path, local_path) = setup_two_profiles();
    let result = delete_profile(
        "default",
        &roost_dir,
        &mut shared,
        &shared_path,
        &mut local,
        &local_path,
    );
    assert!(result.is_err());
}

#[test]
fn delete_profile_cannot_delete_only_profile() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let shared_path = tmp.path().join("roost.toml");
    let local_path = tmp.path().join("local.toml");

    fs::create_dir_all(roost_dir.join("only")).unwrap();

    let mut shared = make_shared();
    shared.profiles.insert("only".to_string(), make_profile());
    let mut local = make_local();
    local.active_profile = "only".to_string();

    shared.save(&shared_path).unwrap();
    local.save(&local_path).unwrap();

    let result = delete_profile(
        "only",
        &roost_dir,
        &mut shared,
        &shared_path,
        &mut local,
        &local_path,
    );
    assert!(result.is_err());
}

#[test]
fn delete_profile_nonexistent_fails() {
    let (_tmp, mut shared, mut local, roost_dir, shared_path, local_path) = setup_two_profiles();
    let result = delete_profile(
        "nope",
        &roost_dir,
        &mut shared,
        &shared_path,
        &mut local,
        &local_path,
    );
    assert!(result.is_err());
}

#[test]
fn delete_profile_succeeds() {
    let (_tmp, mut shared, mut local, roost_dir, shared_path, local_path) = setup_two_profiles();
    delete_profile(
        "other",
        &roost_dir,
        &mut shared,
        &shared_path,
        &mut local,
        &local_path,
    )
    .unwrap();
    assert!(!shared.profiles.contains_key("other"));
    assert!(!roost_dir.join("other").exists());
    assert!(shared.profiles.contains_key("default"));
}

// ── 7. Legacy migration tests ──────────────────────────────────────────────

#[test]
fn migrate_link_paths_from_legacy() {
    let tmp = TempDir::new().unwrap();
    let shared_path = tmp.path().join("roost.toml");
    let local_path = tmp.path().join("local.toml");

    let legacy_toml = r#"
[apps.nvim]
link_path = "~/.config/nvim"

[apps.bash]
link_path = "~/.bashrc"
"#;
    fs::write(&shared_path, legacy_toml).unwrap();

    let mut local = make_local();
    local.save(&local_path).unwrap();

    migrate_link_paths_if_needed(&shared_path, &mut local, &local_path).unwrap();

    assert_eq!(local.link_paths.len(), 2);
    assert!(local.link_paths.contains_key("nvim"));
    assert!(local.link_paths.contains_key("bash"));
}

#[test]
fn migrate_idempotent() {
    let tmp = TempDir::new().unwrap();
    let shared_path = tmp.path().join("roost.toml");
    let local_path = tmp.path().join("local.toml");

    let legacy_toml = r#"
[apps.nvim]
link_path = "~/.config/nvim"
"#;
    fs::write(&shared_path, legacy_toml).unwrap();

    let mut local = make_local();
    local.save(&local_path).unwrap();

    migrate_link_paths_if_needed(&shared_path, &mut local, &local_path).unwrap();
    let count_after_first = local.link_paths.len();

    migrate_link_paths_if_needed(&shared_path, &mut local, &local_path).unwrap();
    assert_eq!(local.link_paths.len(), count_after_first);
}

#[test]
fn test_load_corrupted_toml_fails() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("roost.toml");
    fs::write(&path, "[profile\ninvalid toml {{{").unwrap();
    let result = SharedAppConfig::load(&path);
    assert!(result.is_err());
}

#[test]
fn test_load_empty_file_fails() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("roost.toml");
    fs::write(&path, "").unwrap();
    let result = SharedAppConfig::load(&path);
    assert!(result.is_err());
}

#[test]
fn test_load_missing_file_fails_with_clear_error() {
    let path = PathBuf::from("/tmp/roost-nonexistent-12345/roost.toml");
    let result = SharedAppConfig::load(&path);
    match result {
        Err(e) => assert!(!e.to_string().is_empty()),
        Ok(_) => panic!("expected error for missing file"),
    }
}

#[test]
fn test_save_creates_parent_directories() {
    let dir = tempfile::TempDir::new().unwrap();
    let nested = dir.path().join("nested/deep");
    fs::create_dir_all(&nested).unwrap();
    let path = nested.join("roost.toml");
    let config = SharedAppConfig {
        remote: None,
        profiles: HashMap::new(),
        apps: HashMap::new(),
        ignored: HashSet::new(),
    };
    config.save(&path).unwrap();
    assert!(path.exists());
}

#[test]
fn test_shared_config_with_many_apps_roundtrip() {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let mut profiles = HashMap::new();
    let mut apps = HashMap::new();
    let mut ignored = HashSet::new();

    for i in 0..20 {
        let name = format!("app{}", i);
        let prof_name = if i % 2 == 0 { "even" } else { "odd" };
        profiles
            .entry(prof_name.to_string())
            .or_insert_with(Profile::empty)
            .apps
            .insert(name.clone());
        apps.insert(
            name.clone(),
            Application {
                name: name.clone(),
                primary_config: Some(home.join(format!(".config/{}/config.toml", name))),
                on_profiles: vec![prof_name.to_string()],
            },
        );
        ignored.insert(format!("{}.tmp", name));
    }

    let config = SharedAppConfig {
        remote: Some("git@github.com:test/dotfiles.git".to_string()),
        profiles,
        apps,
        ignored,
    };

    let toml_str = toml::to_string(&config).unwrap();
    let deserialized: SharedAppConfig = toml::from_str(&toml_str).unwrap();

    assert_eq!(deserialized.apps.len(), 20);
    assert_eq!(deserialized.profiles.len(), 2);
    assert_eq!(deserialized.ignored.len(), 20);
    assert_eq!(
        deserialized.remote.as_deref(),
        Some("git@github.com:test/dotfiles.git")
    );
}

#[test]
fn test_local_config_with_special_chars_in_path() {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let config = LocalAppConfig {
        active_profile: "default".to_string(),
        os_info: OsInfo::default(),
        link_paths: HashMap::from([
            (
                "spaces app".to_string(),
                home.join("path with spaces/config"),
            ),
            ("unicode_app".to_string(), home.join(".config/myapp")),
        ]),
    };

    let toml_str = toml::to_string(&config).unwrap();
    let deserialized: LocalAppConfig = toml::from_str(&toml_str).unwrap();

    assert_eq!(deserialized.link_paths.len(), 2);
    assert!(deserialized.link_paths.contains_key("spaces app"));
}

#[test]
fn test_migrate_link_paths_no_legacy_field_is_nop() {
    let dir = tempfile::TempDir::new().unwrap();
    let shared_path = dir.path().join("roost.toml");
    let local_path = dir.path().join("local.toml");

    let modern_toml = r#"
[apps.nvim]
name = "nvim"
on_profiles = ["laptop"]
"#;
    fs::write(&shared_path, modern_toml).unwrap();

    let mut local = LocalAppConfig {
        active_profile: "laptop".to_string(),
        os_info: OsInfo::default(),
        link_paths: HashMap::from([("nvim".to_string(), PathBuf::from("/home/.config/nvim"))]),
    };

    let link_count_before = local.link_paths.len();
    migrate_link_paths_if_needed(&shared_path, &mut local, &local_path).unwrap();
    assert_eq!(local.link_paths.len(), link_count_before);
}

#[test]
fn test_profile_empty_has_no_apps() {
    let p = Profile::empty();
    assert!(p.apps.is_empty());
    assert!(p.app_sources.is_empty());
}
