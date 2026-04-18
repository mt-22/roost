use super::*;
use std::collections::{HashMap, HashSet};
use tempfile::TempDir;

fn make_profile(sources: &[(&str, &str)]) -> crate::app::Profile {
    crate::app::Profile {
        apps: HashSet::new(),
        app_sources: sources
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

fn make_config(profiles: Vec<(&str, &[(&str, &str)])>) -> crate::app::SharedAppConfig {
    crate::app::SharedAppConfig {
        remote: None,
        apps: HashMap::new(),
        ignored: HashSet::new(),
        profiles: profiles
            .into_iter()
            .map(|(name, sources)| (name.to_string(), make_profile(sources)))
            .collect(),
    }
}

// ── roost_dest ──

#[test]
fn roost_dest_existing_dir_in_profile() {
    let tmp = TempDir::new().unwrap();
    let profile_dir = tmp.path().join("profile");
    let dir_candidate = profile_dir.join("nvim");
    fs::create_dir_all(&dir_candidate).unwrap();

    let original = tmp.path().join("config").join("nvim");
    let result = roost_dest(&profile_dir, &original).unwrap();
    assert_eq!(result, dir_candidate);
}

#[test]
fn roost_dest_existing_file_in_misc() {
    let tmp = TempDir::new().unwrap();
    let profile_dir = tmp.path().join("profile");
    let misc = profile_dir.join("misc");
    fs::create_dir_all(&misc).unwrap();
    let file_candidate = misc.join("config.toml");
    fs::write(&file_candidate, "data").unwrap();

    let original = tmp.path().join("config").join("config.toml");
    let result = roost_dest(&profile_dir, &original).unwrap();
    assert_eq!(result, file_candidate);
}

#[test]
fn roost_dest_fallback_original_is_dir() {
    let tmp = TempDir::new().unwrap();
    let profile_dir = tmp.path().join("profile");
    fs::create_dir_all(&profile_dir).unwrap();

    let original = tmp.path().join("config").join("nvim");
    fs::create_dir_all(&original).unwrap();

    let result = roost_dest(&profile_dir, &original).unwrap();
    assert_eq!(result, profile_dir.join("nvim"));
}

#[test]
fn roost_dest_fallback_original_is_file() {
    let tmp = TempDir::new().unwrap();
    let profile_dir = tmp.path().join("profile");
    fs::create_dir_all(&profile_dir).unwrap();

    let original = tmp.path().join("config").join("config.toml");
    fs::create_dir_all(original.parent().unwrap()).unwrap();
    fs::write(&original, "data").unwrap();

    let result = roost_dest(&profile_dir, &original).unwrap();
    assert_eq!(result, profile_dir.join("misc").join("config.toml"));
}

#[test]
fn roost_dest_no_filename_error() {
    let tmp = TempDir::new().unwrap();
    let profile_dir = tmp.path().join("profile");
    assert!(roost_dest(&profile_dir, Path::new("/")).is_err());
}

// ── is_roost_symlink ──

#[test]
fn is_roost_symlink_into_roost() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let target = roost_dir.join("profile/nvim");
    fs::create_dir_all(&target).unwrap();

    let link = tmp.path().join("config").join("nvim");
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();

    assert!(is_roost_symlink(&link, &roost_dir));
}

#[test]
fn is_roost_symlink_outside_roost() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let target = tmp.path().join("other").join("nvim");
    fs::create_dir_all(&target).unwrap();

    let link = tmp.path().join("config").join("nvim");
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();

    assert!(!is_roost_symlink(&link, &roost_dir));
}

#[test]
fn is_roost_symlink_not_a_symlink() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let path = tmp.path().join("config").join("nvim");
    fs::create_dir_all(&path).unwrap();

    assert!(!is_roost_symlink(&path, &roost_dir));
}

// ── ingest ──

#[test]
fn ingest_directory() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let profile_dir = roost_dir.join("profile");
    fs::create_dir_all(&profile_dir).unwrap();

    let original = tmp.path().join("config").join("nvim");
    fs::create_dir_all(&original).unwrap();
    fs::write(original.join("init.lua"), "content").unwrap();

    ingest(&original, &profile_dir, &roost_dir).unwrap();

    assert!(original.is_symlink());
    let dest = profile_dir.join("nvim");
    assert!(dest.is_dir());
    assert_eq!(
        fs::read_to_string(dest.join("init.lua")).unwrap(),
        "content"
    );
}

#[test]
fn ingest_file() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let profile_dir = roost_dir.join("profile");
    fs::create_dir_all(&profile_dir).unwrap();

    let original = tmp.path().join("config").join("settings.toml");
    fs::create_dir_all(original.parent().unwrap()).unwrap();
    fs::write(&original, "data").unwrap();

    ingest(&original, &profile_dir, &roost_dir).unwrap();

    assert!(original.is_symlink());
    let dest = profile_dir.join("misc").join("settings.toml");
    assert!(dest.is_file());
    assert_eq!(fs::read_to_string(&dest).unwrap(), "data");
}

#[test]
fn ingest_already_roost_symlink_noop() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let profile_dir = roost_dir.join("profile");
    fs::create_dir_all(&profile_dir).unwrap();

    let dest = profile_dir.join("nvim");
    fs::create_dir_all(&dest).unwrap();

    let original = tmp.path().join("config").join("nvim");
    fs::create_dir_all(original.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&dest, &original).unwrap();

    ingest(&original, &profile_dir, &roost_dir).unwrap();
}

#[test]
fn ingest_nonexistent_source_error() {
    let tmp = TempDir::new().unwrap();
    let profile_dir = tmp.path().join("profile");
    fs::create_dir_all(&profile_dir).unwrap();

    let original = tmp.path().join("nonexistent");
    assert!(ingest(&original, &profile_dir, tmp.path()).is_err());
}

#[test]
fn ingest_destination_exists_error() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let profile_dir = roost_dir.join("profile");
    fs::create_dir_all(&profile_dir).unwrap();

    let dest = profile_dir.join("nvim");
    fs::create_dir_all(&dest).unwrap();

    let original = tmp.path().join("config").join("nvim");
    fs::create_dir_all(&original).unwrap();
    fs::write(original.join("init.lua"), "content").unwrap();

    assert!(ingest(&original, &profile_dir, &roost_dir).is_err());
}

#[test]
fn ingest_removes_nested_git() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let profile_dir = roost_dir.join("profile");
    fs::create_dir_all(&profile_dir).unwrap();

    let original = tmp.path().join("config").join("myapp");
    fs::create_dir_all(original.join(".git/objects")).unwrap();
    fs::write(original.join(".git/HEAD"), "ref: refs/heads/main").unwrap();
    fs::write(original.join("config.toml"), "settings").unwrap();

    ingest(&original, &profile_dir, &roost_dir).unwrap();

    let dest = profile_dir.join("myapp");
    assert!(dest.join("config.toml").exists());
    assert!(!dest.join(".git").exists());
}

// ── restore ──

#[test]
fn restore_creates_symlink() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");

    let dest = roost_dir.join("profile/nvim");
    fs::create_dir_all(&dest).unwrap();

    let original = tmp.path().join("config").join("nvim");
    restore(&original, &dest, &roost_dir).unwrap();

    assert!(original.is_symlink());
    assert_eq!(fs::read_link(&original).unwrap(), dest);
}

#[test]
fn restore_already_roost_symlink_noop() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");

    let dest = roost_dir.join("profile/nvim");
    fs::create_dir_all(&dest).unwrap();

    let original = tmp.path().join("config").join("nvim");
    fs::create_dir_all(original.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&dest, &original).unwrap();

    restore(&original, &dest, &roost_dir).unwrap();
}

#[test]
fn restore_dest_not_exists_error() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");

    let dest = roost_dir.join("profile/nvim");
    let original = tmp.path().join("config").join("nvim");

    assert!(restore(&original, &dest, &roost_dir).is_err());
}

#[test]
fn restore_original_exists_not_symlink_error() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");

    let dest = roost_dir.join("profile/nvim");
    fs::create_dir_all(&dest).unwrap();

    let original = tmp.path().join("config").join("nvim");
    fs::create_dir_all(&original).unwrap();

    assert!(restore(&original, &dest, &roost_dir).is_err());
}

// ── unlink ──

#[test]
fn unlink_owned_app_restores_files() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let profile_dir = roost_dir.join("profile");
    fs::create_dir_all(&profile_dir).unwrap();

    let original = tmp.path().join("config").join("nvim");
    fs::create_dir_all(&original).unwrap();
    fs::write(original.join("init.lua"), "hello").unwrap();

    ingest(&original, &profile_dir, &roost_dir).unwrap();
    assert!(original.is_symlink());

    unlink(&original, &profile_dir, &roost_dir).unwrap();

    assert!(!original.is_symlink());
    assert!(original.join("init.lua").exists());
    assert_eq!(
        fs::read_to_string(original.join("init.lua")).unwrap(),
        "hello"
    );
}

#[test]
fn unlink_sourced_app_removes_both_symlinks() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let profile_dir = roost_dir.join("profile");
    let source_profile_dir = roost_dir.join("shared");
    fs::create_dir_all(&profile_dir).unwrap();
    fs::create_dir_all(&source_profile_dir).unwrap();

    let source_app = source_profile_dir.join("nvim");
    fs::create_dir_all(&source_app).unwrap();
    fs::write(source_app.join("init.lua"), "source_content").unwrap();

    let dest = profile_dir.join("nvim");
    std::os::unix::fs::symlink(&source_app, &dest).unwrap();

    let original = tmp.path().join("config").join("nvim");
    fs::create_dir_all(original.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&dest, &original).unwrap();

    unlink(&original, &profile_dir, &roost_dir).unwrap();

    assert!(!original.exists());
    assert!(!dest.exists());
    assert!(source_app.join("init.lua").exists());
}

#[test]
fn unlink_not_symlink_error() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let profile_dir = roost_dir.join("profile");
    fs::create_dir_all(&profile_dir).unwrap();

    let original = tmp.path().join("config").join("nvim");
    fs::create_dir_all(&original).unwrap();

    assert!(unlink(&original, &profile_dir, &roost_dir).is_err());
}

// ── detect_source_cycle ──

#[test]
fn cycle_no_cycle() {
    let config = make_config(vec![("A", &[]), ("B", &[])]);
    assert!(!detect_source_cycle("A", "nvim", "B", &config));
}

#[test]
fn cycle_direct() {
    let config = make_config(vec![("A", &[]), ("B", &[("nvim", "A")])]);
    assert!(detect_source_cycle("A", "nvim", "B", &config));
}

#[test]
fn cycle_indirect() {
    let config = make_config(vec![
        ("A", &[]),
        ("B", &[("nvim", "C")]),
        ("C", &[("nvim", "A")]),
    ]);
    assert!(detect_source_cycle("A", "nvim", "B", &config));
}

#[test]
fn cycle_chain_terminates() {
    let config = make_config(vec![("A", &[]), ("B", &[("nvim", "C")]), ("C", &[])]);
    assert!(!detect_source_cycle("A", "nvim", "B", &config));
}

#[test]
fn cycle_missing_profile() {
    let config = make_config(vec![("A", &[])]);
    assert!(!detect_source_cycle("A", "nvim", "nonexistent", &config));
}

#[test]
fn cycle_self_target() {
    let config = make_config(vec![("A", &[])]);
    assert!(detect_source_cycle("A", "nvim", "A", &config));
}

// ── relocate ──

#[test]
fn relocate_same_device() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src.txt");
    let dest = tmp.path().join("dest.txt");
    fs::write(&src, "content").unwrap();

    relocate(&src, &dest).unwrap();

    assert!(!src.exists());
    assert_eq!(fs::read_to_string(&dest).unwrap(), "content");
}

#[test]
fn relocate_creates_parent_dirs() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src.txt");
    let dest = tmp.path().join("a").join("b").join("dest.txt");
    fs::write(&src, "content").unwrap();

    relocate(&src, &dest).unwrap();

    assert_eq!(fs::read_to_string(&dest).unwrap(), "content");
}

// ── copy_dir_recursive ──

#[test]
fn copy_dir_nested() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(src.join("sub")).unwrap();
    fs::write(src.join("top.txt"), "top").unwrap();
    fs::write(src.join("sub/nested.txt"), "nested").unwrap();

    let dest = tmp.path().join("dest");
    copy_dir_recursive(&src, &dest).unwrap();

    assert_eq!(fs::read_to_string(dest.join("top.txt")).unwrap(), "top");
    assert_eq!(
        fs::read_to_string(dest.join("sub/nested.txt")).unwrap(),
        "nested"
    );
}

#[test]
fn copy_dir_preserves_content() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("file.txt"), "hello world").unwrap();

    let dest = tmp.path().join("dest");
    copy_dir_recursive(&src, &dest).unwrap();

    assert_eq!(
        fs::read_to_string(dest.join("file.txt")).unwrap(),
        "hello world"
    );
}

// ── switch_links ──

fn make_profile_with_apps(apps: &[&str], sources: &[(&str, &str)]) -> crate::app::Profile {
    crate::app::Profile {
        apps: apps.iter().map(|s| s.to_string()).collect(),
        app_sources: sources
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

fn make_switch_config(
    profiles: Vec<(&str, &[&str], &[(&str, &str)])>,
    apps: Vec<(&str, &[&str])>,
) -> crate::app::SharedAppConfig {
    crate::app::SharedAppConfig {
        remote: None,
        apps: apps
            .into_iter()
            .map(|(name, profiles)| {
                (
                    name.to_string(),
                    crate::app::Application {
                        name: name.to_string(),
                        primary_config: None,
                        on_profiles: profiles.iter().map(|s| s.to_string()).collect(),
                    },
                )
            })
            .collect(),
        ignored: HashSet::new(),
        profiles: profiles
            .into_iter()
            .map(|(name, app_names, sources)| {
                (name.to_string(), make_profile_with_apps(app_names, sources))
            })
            .collect(),
    }
}

fn make_local(active: &str, link_paths: Vec<(&str, &Path)>) -> crate::app::LocalAppConfig {
    crate::app::LocalAppConfig {
        active_profile: active.to_string(),
        os_info: Default::default(),
        link_paths: link_paths
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_path_buf()))
            .collect(),
    }
}

#[test]
fn test_switch_links_removes_old_symlinks() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let link_path = tmp.path().join("config").join("nvim");

    let old_prof_dir = roost_dir.join("old");
    let new_prof_dir = roost_dir.join("new");
    fs::create_dir_all(old_prof_dir.join("nvim")).unwrap();
    fs::create_dir_all(new_prof_dir.join("nvim")).unwrap();
    fs::write(old_prof_dir.join("nvim/init.lua"), "old content").unwrap();
    fs::write(new_prof_dir.join("nvim/init.lua"), "new content").unwrap();

    fs::create_dir_all(link_path.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&old_prof_dir.join("nvim"), &link_path).unwrap();

    let config = make_switch_config(
        vec![("old", &["nvim"], &[]), ("new", &["nvim"], &[])],
        vec![("nvim", &["old", "new"])],
    );
    let local = make_local("old", vec![("nvim", &link_path)]);

    super::switch_links("old", "new", &config, &local, &roost_dir);

    assert!(link_path.is_symlink());
    assert_eq!(
        fs::canonicalize(&link_path).unwrap(),
        fs::canonicalize(new_prof_dir.join("nvim")).unwrap()
    );
    assert_eq!(
        fs::read_to_string(link_path.join("init.lua")).unwrap(),
        "new content"
    );
}

#[test]
fn test_switch_links_to_empty_profile_removes_symlinks() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let link_path = tmp.path().join("config").join("nvim");

    let old_prof_dir = roost_dir.join("old");
    fs::create_dir_all(old_prof_dir.join("nvim")).unwrap();
    fs::write(old_prof_dir.join("nvim/init.lua"), "old content").unwrap();

    fs::create_dir_all(link_path.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&old_prof_dir.join("nvim"), &link_path).unwrap();

    let config = make_switch_config(
        vec![("old", &["nvim"], &[]), ("empty", &[], &[])],
        vec![("nvim", &["old", "empty"])],
    );
    let local = make_local("old", vec![("nvim", &link_path)]);

    super::switch_links("old", "empty", &config, &local, &roost_dir);

    assert!(
        !link_path.exists(),
        "symlink should be removed when new profile has no apps"
    );
}

#[test]
fn test_switch_links_nonexistent_new_profile_noop() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let link_path = tmp.path().join("config").join("nvim");

    let old_prof_dir = roost_dir.join("old");
    fs::create_dir_all(old_prof_dir.join("nvim")).unwrap();

    fs::create_dir_all(link_path.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&old_prof_dir.join("nvim"), &link_path).unwrap();

    let config = make_switch_config(vec![("old", &["nvim"], &[])], vec![("nvim", &["old"])]);
    let local = make_local("old", vec![("nvim", &link_path)]);

    super::switch_links("old", "nonexistent", &config, &local, &roost_dir);
}

#[test]
fn test_switch_links_with_source_symlinks() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let link_path = tmp.path().join("config").join("nvim");

    let source_prof_dir = roost_dir.join("source");
    let old_prof_dir = roost_dir.join("old");
    let new_prof_dir = roost_dir.join("new");
    fs::create_dir_all(source_prof_dir.join("nvim")).unwrap();
    fs::write(source_prof_dir.join("nvim/init.lua"), "source content").unwrap();

    fs::create_dir_all(&old_prof_dir).unwrap();
    let old_nvim = old_prof_dir.join("nvim");
    std::os::unix::fs::symlink(&source_prof_dir.join("nvim"), &old_nvim).unwrap();

    fs::create_dir_all(&new_prof_dir).unwrap();
    let new_nvim_pre = new_prof_dir.join("nvim");
    std::os::unix::fs::symlink(&source_prof_dir.join("nvim"), &new_nvim_pre).unwrap();
    fs::create_dir_all(link_path.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&old_nvim, &link_path).unwrap();

    let config = make_switch_config(
        vec![
            ("source", &["nvim"], &[]),
            ("old", &["nvim"], &[("nvim", "source")]),
            ("new", &["nvim"], &[("nvim", "source")]),
        ],
        vec![("nvim", &["source", "old", "new"])],
    );
    let local = make_local("old", vec![("nvim", &link_path)]);

    super::switch_links("old", "new", &config, &local, &roost_dir);

    let new_nvim = new_prof_dir.join("nvim");
    assert!(new_nvim.is_symlink());
    assert_eq!(
        fs::canonicalize(&new_nvim).unwrap(),
        fs::canonicalize(source_prof_dir.join("nvim")).unwrap()
    );

    assert!(link_path.is_symlink());
    assert_eq!(
        fs::canonicalize(&link_path).unwrap(),
        fs::canonicalize(source_prof_dir.join("nvim")).unwrap()
    );
    assert_eq!(
        fs::read_to_string(link_path.join("init.lua")).unwrap(),
        "source content"
    );
}

// ── ensure_links ──

fn make_ensure_config(
    profiles: Vec<(&str, &[&str], &[(&str, &str)])>,
    apps: Vec<(&str, &[&str])>,
) -> crate::app::SharedAppConfig {
    crate::app::SharedAppConfig {
        remote: None,
        apps: apps
            .into_iter()
            .map(|(name, profiles)| {
                (
                    name.to_string(),
                    crate::app::Application {
                        name: name.to_string(),
                        primary_config: None,
                        on_profiles: profiles.iter().map(|s| s.to_string()).collect(),
                    },
                )
            })
            .collect(),
        ignored: HashSet::new(),
        profiles: profiles
            .into_iter()
            .map(|(name, app_names, sources)| {
                (name.to_string(), make_profile_with_apps(app_names, sources))
            })
            .collect(),
    }
}

#[test]
fn test_ensure_links_creates_external_symlink() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let profile_dir = roost_dir.join("default");
    fs::create_dir_all(profile_dir.join("nvim")).unwrap();
    fs::write(profile_dir.join("nvim/init.lua"), "config").unwrap();

    let link_path = tmp.path().join("home/.config/nvim");
    let roost_slot = profile_dir.join("nvim");

    let config = make_ensure_config(
        vec![("default", &["nvim"], &[])],
        vec![("nvim", &["default"])],
    );
    let local = make_local("default", vec![("nvim", &link_path)]);

    ensure_links(&config, &local, &roost_dir);

    assert!(link_path.is_symlink());
    assert!(link_path.join("init.lua").exists());
    assert_eq!(fs::read_link(&link_path).unwrap(), roost_slot);
}

#[test]
fn test_ensure_links_skips_existing_symlink() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let profile_dir = roost_dir.join("default");
    fs::create_dir_all(profile_dir.join("nvim")).unwrap();
    fs::write(profile_dir.join("nvim/init.lua"), "config").unwrap();

    let link_path = tmp.path().join("home/.config/nvim");
    let roost_slot = profile_dir.join("nvim");
    fs::create_dir_all(link_path.parent().unwrap()).unwrap();
    symlink(&roost_slot, &link_path).unwrap();

    let config = make_ensure_config(
        vec![("default", &["nvim"], &[])],
        vec![("nvim", &["default"])],
    );
    let local = make_local("default", vec![("nvim", &link_path)]);

    ensure_links(&config, &local, &roost_dir);

    assert!(link_path.is_symlink());
}

#[test]
fn test_ensure_links_backs_up_conflicting_file() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let profile_dir = roost_dir.join("default");
    fs::create_dir_all(profile_dir.join("nvim")).unwrap();
    fs::write(profile_dir.join("nvim/init.lua"), "config").unwrap();

    let link_path = tmp.path().join("home/.config/nvim");
    fs::create_dir_all(&link_path).unwrap();
    fs::write(link_path.join("existing.txt"), "original content").unwrap();

    let backup = tmp_backup_path(&link_path);

    let config = make_ensure_config(
        vec![("default", &["nvim"], &[])],
        vec![("nvim", &["default"])],
    );
    let local = make_local("default", vec![("nvim", &link_path)]);

    ensure_links(&config, &local, &roost_dir);

    assert!(link_path.is_symlink());
    assert!(backup.exists());
    assert_eq!(
        fs::read_to_string(backup.join("existing.txt")).unwrap(),
        "original content"
    );
}

#[test]
fn test_ensure_links_pass1_creates_source_symlink() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let source_dir = roost_dir.join("source");
    let target_dir = roost_dir.join("target");
    fs::create_dir_all(source_dir.join("nvim")).unwrap();
    fs::write(source_dir.join("nvim/init.lua"), "source config").unwrap();

    let link_path = tmp.path().join("home/.config/nvim");
    fs::create_dir_all(&link_path).unwrap();

    let target_slot = target_dir.join("nvim");

    let config = make_ensure_config(
        vec![
            ("source", &["nvim"], &[]),
            ("target", &["nvim"], &[("nvim", "source")]),
        ],
        vec![("nvim", &["source", "target"])],
    );
    let local = make_local("target", vec![("nvim", &link_path)]);

    ensure_links(&config, &local, &roost_dir);

    assert!(target_slot.is_symlink());
    assert_eq!(
        fs::read_link(&target_slot).unwrap(),
        source_dir.join("nvim")
    );
}

#[test]
fn test_ensure_links_skips_app_not_on_device() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let profile_dir = roost_dir.join("default");
    fs::create_dir_all(profile_dir.join("nvim")).unwrap();
    fs::write(profile_dir.join("nvim/init.lua"), "config").unwrap();

    let config = make_ensure_config(
        vec![("default", &["nvim"], &[])],
        vec![("nvim", &["default"])],
    );
    let local = make_local("default", vec![]);

    ensure_links(&config, &local, &roost_dir);
}

#[test]
fn test_ensure_links_skips_real_files_in_roost_slot() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let source_dir = roost_dir.join("source");
    let target_dir = roost_dir.join("target");

    fs::create_dir_all(source_dir.join("nvim")).unwrap();
    fs::write(source_dir.join("nvim/init.lua"), "source config").unwrap();

    fs::create_dir_all(target_dir.join("nvim")).unwrap();
    fs::write(target_dir.join("nvim/init.lua"), "target owns this").unwrap();

    let link_path = tmp.path().join("home/.config/nvim");
    let target_slot = target_dir.join("nvim");

    let config = make_ensure_config(
        vec![
            ("source", &["nvim"], &[]),
            ("target", &["nvim"], &[("nvim", "source")]),
        ],
        vec![("nvim", &["source", "target"])],
    );
    let local = make_local("target", vec![("nvim", &link_path)]);

    ensure_links(&config, &local, &roost_dir);

    assert!(!target_slot.is_symlink());
    assert_eq!(
        fs::read_to_string(target_slot.join("init.lua")).unwrap(),
        "target owns this"
    );
}

// ── import_app_from_profile ──

fn make_import_setup(
    _roost_dir: &Path,
    config_path: &Path,
    profiles: Vec<(&str, &[&str], &[(&str, &str)])>,
    apps: Vec<(&str, &[&str])>,
    link_path: &Path,
) -> (crate::app::SharedAppConfig, crate::app::LocalAppConfig) {
    let config = make_switch_config(profiles, apps);
    config.save(config_path).unwrap();
    fs::create_dir_all(link_path).unwrap();
    let mut local = crate::app::LocalAppConfig {
        active_profile: "source".to_string(),
        os_info: crate::os_detect::OsInfo {
            family: "unix".to_string(),
            name: "test".to_string(),
            version: Some("1.0.0".to_string()),
            arch: std::env::consts::ARCH.to_string(),
        },
        link_paths: HashMap::new(),
    };
    local
        .link_paths
        .insert("nvim".to_string(), link_path.to_path_buf());
    (config, local)
}

#[test]
fn test_import_app_from_profile_success() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let config_path = roost_dir.join("roost.toml");
    let link_path = tmp.path().join("config").join("nvim");

    let source_nvim = roost_dir.join("source").join("nvim");
    fs::create_dir_all(source_nvim.join("lua")).unwrap();
    fs::write(source_nvim.join("init.lua"), "source init").unwrap();
    fs::write(source_nvim.join("lua/plugins.lua"), "plugins").unwrap();
    fs::create_dir_all(roost_dir.join("target")).unwrap();

    let (mut config, mut local) = make_import_setup(
        &roost_dir,
        &config_path,
        vec![("source", &["nvim"], &[]), ("target", &[], &[])],
        vec![("nvim", &["source"])],
        &link_path,
    );

    import_app_from_profile(
        "nvim",
        "target",
        "source",
        &mut config,
        &config_path,
        &roost_dir,
        &mut local,
    )
    .unwrap();

    assert!(config.profiles["target"].apps.contains("nvim"));
    assert_eq!(
        config.profiles["target"].app_sources.get("nvim").unwrap(),
        "source"
    );
    let target_nvim = roost_dir.join("target").join("nvim");
    assert!(target_nvim.is_symlink());
    assert_eq!(fs::read_link(&target_nvim).unwrap(), source_nvim);
    assert!(link_path.is_symlink());
    assert_eq!(fs::read_link(&link_path).unwrap(), target_nvim);
    assert!(config.apps["nvim"]
        .on_profiles
        .contains(&"target".to_string()));
}

#[test]
fn test_import_app_from_same_profile_fails() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let config_path = roost_dir.join("roost.toml");
    let link_path = tmp.path().join("config").join("nvim");

    fs::create_dir_all(roost_dir.join("source").join("nvim")).unwrap();

    let (mut config, mut local) = make_import_setup(
        &roost_dir,
        &config_path,
        vec![("source", &["nvim"], &[])],
        vec![],
        &link_path,
    );

    let result = import_app_from_profile(
        "nvim",
        "source",
        "source",
        &mut config,
        &config_path,
        &roost_dir,
        &mut local,
    );
    assert!(result.is_err());
}

#[test]
fn test_import_app_already_in_target_fails() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let config_path = roost_dir.join("roost.toml");
    let link_path = tmp.path().join("config").join("nvim");

    fs::create_dir_all(roost_dir.join("source").join("nvim")).unwrap();
    fs::create_dir_all(roost_dir.join("target")).unwrap();

    let (mut config, mut local) = make_import_setup(
        &roost_dir,
        &config_path,
        vec![("source", &["nvim"], &[]), ("target", &["nvim"], &[])],
        vec![],
        &link_path,
    );

    let result = import_app_from_profile(
        "nvim",
        "target",
        "source",
        &mut config,
        &config_path,
        &roost_dir,
        &mut local,
    );
    assert!(result.is_err());
}

#[test]
fn test_import_app_detects_cycle() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let config_path = roost_dir.join("roost.toml");
    let link_path = tmp.path().join("config").join("nvim");

    fs::create_dir_all(roost_dir.join("a").join("nvim")).unwrap();
    fs::create_dir_all(roost_dir.join("b")).unwrap();

    let (mut config, mut local) = make_import_setup(
        &roost_dir,
        &config_path,
        vec![("a", &["nvim"], &[]), ("b", &["nvim"], &[("nvim", "a")])],
        vec![("nvim", &["a", "b"])],
        &link_path,
    );

    let result = import_app_from_profile(
        "nvim",
        "a",
        "b",
        &mut config,
        &config_path,
        &roost_dir,
        &mut local,
    );
    assert!(result.is_err());
}

// ── copy_to_profile ──

fn make_copy_setup(
    _roost_dir: &Path,
    config_path: &Path,
    profiles: Vec<(&str, &[&str], &[(&str, &str)])>,
    apps: Vec<(&str, &[&str])>,
    link_path: &Path,
) -> (
    crate::app::SharedAppConfig,
    HashMap<String, std::path::PathBuf>,
) {
    let config = make_switch_config(profiles, apps);
    config.save(config_path).unwrap();
    fs::create_dir_all(link_path).unwrap();
    let mut link_paths = HashMap::new();
    link_paths.insert("nvim".to_string(), link_path.to_path_buf());
    (config, link_paths)
}

#[test]
fn test_copy_to_profile_success() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let config_path = roost_dir.join("roost.toml");
    let link_path = tmp.path().join("config").join("nvim");

    let source_nvim = roost_dir.join("source").join("nvim");
    fs::create_dir_all(source_nvim.join("lua")).unwrap();
    fs::write(source_nvim.join("init.lua"), "source init").unwrap();
    fs::write(source_nvim.join("lua/plugins.lua"), "plugins").unwrap();
    fs::create_dir_all(roost_dir.join("dest")).unwrap();

    let (mut config, link_paths) = make_copy_setup(
        &roost_dir,
        &config_path,
        vec![("source", &["nvim"], &[]), ("dest", &[], &[])],
        vec![("nvim", &["source"])],
        &link_path,
    );

    copy_to_profile(
        "nvim",
        "source",
        "dest",
        &mut config,
        &config_path,
        &roost_dir,
        &link_paths,
    )
    .unwrap();

    let dest_nvim = roost_dir.join("dest").join("nvim");
    assert!(dest_nvim.is_dir());
    assert!(!dest_nvim.is_symlink());
    assert_eq!(
        fs::read_to_string(dest_nvim.join("init.lua")).unwrap(),
        "source init"
    );
    assert_eq!(
        fs::read_to_string(dest_nvim.join("lua/plugins.lua")).unwrap(),
        "plugins"
    );
    assert!(config.profiles["dest"].apps.contains("nvim"));
    assert!(!config.profiles["dest"].app_sources.contains_key("nvim"));
    assert!(config.apps["nvim"]
        .on_profiles
        .contains(&"dest".to_string()));
}

#[test]
fn test_copy_to_profile_same_profile_fails() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let config_path = roost_dir.join("roost.toml");
    let link_path = tmp.path().join("config").join("nvim");

    fs::create_dir_all(roost_dir.join("source").join("nvim")).unwrap();

    let (mut config, link_paths) = make_copy_setup(
        &roost_dir,
        &config_path,
        vec![("source", &["nvim"], &[])],
        vec![],
        &link_path,
    );

    let result = copy_to_profile(
        "nvim",
        "source",
        "source",
        &mut config,
        &config_path,
        &roost_dir,
        &link_paths,
    );
    assert!(result.is_err());
}

#[test]
fn test_copy_to_profile_already_exists_fails() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let config_path = roost_dir.join("roost.toml");
    let link_path = tmp.path().join("config").join("nvim");

    fs::create_dir_all(roost_dir.join("source").join("nvim")).unwrap();
    fs::create_dir_all(roost_dir.join("dest").join("nvim")).unwrap();

    let (mut config, link_paths) = make_copy_setup(
        &roost_dir,
        &config_path,
        vec![("source", &["nvim"], &[]), ("dest", &[], &[])],
        vec![],
        &link_path,
    );

    let result = copy_to_profile(
        "nvim",
        "source",
        "dest",
        &mut config,
        &config_path,
        &roost_dir,
        &link_paths,
    );
    assert!(result.is_err());
}

#[test]
fn test_copy_to_profile_no_source_files_fails() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    let config_path = roost_dir.join("roost.toml");
    let link_path = tmp.path().join("config").join("nvim");

    fs::create_dir_all(&roost_dir).unwrap();

    let (mut config, link_paths) = make_copy_setup(
        &roost_dir,
        &config_path,
        vec![("source", &["nvim"], &[]), ("dest", &[], &[])],
        vec![],
        &link_path,
    );

    let result = copy_to_profile(
        "nvim",
        "source",
        "dest",
        &mut config,
        &config_path,
        &roost_dir,
        &link_paths,
    );
    assert!(result.is_err());
}

// ── ingest edge cases ──

#[test]
fn test_ingest_already_roost_symlink_is_nop() {
    let dir = tempfile::TempDir::new().unwrap();
    let roost_dir = dir.path().join("roost");
    let profile_dir = roost_dir.join("default");
    fs::create_dir_all(&profile_dir).unwrap();

    let target = profile_dir.join("nvim");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("init.lua"), "existing").unwrap();

    let original = dir.path().join("nvim");
    symlink(&target, &original).unwrap();

    ingest(&original, &profile_dir, &roost_dir).unwrap();

    let content = fs::read_to_string(target.join("init.lua")).unwrap();
    assert_eq!(content, "existing");
}

#[test]
fn test_ingest_preserves_nested_directory_structure() {
    let dir = tempfile::TempDir::new().unwrap();
    let roost_dir = dir.path().join("roost");
    let profile_dir = roost_dir.join("default");
    fs::create_dir_all(&profile_dir).unwrap();

    let original = dir.path().join("nvim");
    fs::create_dir_all(original.join("lua/plugins")).unwrap();
    fs::write(original.join("init.lua"), "init").unwrap();
    fs::write(original.join("lua/plugins/lsp.lua"), "lsp config").unwrap();
    fs::write(original.join("lua/utils.lua"), "utils").unwrap();

    ingest(&original, &profile_dir, &roost_dir).unwrap();

    assert!(profile_dir.join("nvim/init.lua").exists());
    assert!(profile_dir.join("nvim/lua/plugins/lsp.lua").exists());
    assert!(profile_dir.join("nvim/lua/utils.lua").exists());
    assert_eq!(
        fs::read_to_string(profile_dir.join("nvim/lua/plugins/lsp.lua")).unwrap(),
        "lsp config"
    );
}

#[test]
fn test_ingest_file_with_no_parent_dir_creates_misc() {
    let dir = tempfile::TempDir::new().unwrap();
    let roost_dir = dir.path().join("roost");
    let profile_dir = roost_dir.join("default");
    fs::create_dir_all(&profile_dir).unwrap();

    let original = dir.path().join(".bashrc");
    fs::write(&original, "export PATH=$HOME/bin").unwrap();

    ingest(&original, &profile_dir, &roost_dir).unwrap();

    assert!(profile_dir.join("misc").is_dir());
    assert!(profile_dir.join("misc/.bashrc").exists());
    assert_eq!(
        fs::read_to_string(profile_dir.join("misc/.bashrc")).unwrap(),
        "export PATH=$HOME/bin"
    );
}

#[test]
fn test_ingest_strips_nested_git_but_keeps_gitignore() {
    let dir = tempfile::TempDir::new().unwrap();
    let roost_dir = dir.path().join("roost");
    let profile_dir = roost_dir.join("default");
    fs::create_dir_all(&profile_dir).unwrap();

    let original = dir.path().join("myconfig");
    fs::create_dir_all(original.join(".git/refs")).unwrap();
    fs::write(original.join(".git/HEAD"), "ref: refs/heads/main").unwrap();
    fs::write(original.join(".gitignore"), "*.log\n").unwrap();
    fs::write(original.join("config.toml"), "data").unwrap();

    ingest(&original, &profile_dir, &roost_dir).unwrap();

    assert!(!profile_dir.join("myconfig/.git").exists());
    assert!(profile_dir.join("myconfig/.gitignore").exists());
    assert!(profile_dir.join("myconfig/config.toml").exists());
}

#[test]
fn test_unlink_external_symlink_not_owned_fails() {
    let dir = tempfile::TempDir::new().unwrap();
    let roost_dir = dir.path().join("roost");
    let profile_dir = roost_dir.join("default");
    fs::create_dir_all(&profile_dir).unwrap();

    let outside_target = dir.path().join("somewhere_else");
    fs::create_dir_all(&outside_target).unwrap();
    fs::write(outside_target.join("file.txt"), "data").unwrap();

    let link = dir.path().join("nvim");
    symlink(&outside_target, &link).unwrap();

    let result = unlink(&link, &profile_dir, &roost_dir);
    assert!(result.is_err());
    assert!(link.is_symlink());
}

#[test]
fn test_unlink_sourced_app_preserves_source() {
    let dir = tempfile::TempDir::new().unwrap();
    let roost_dir = dir.path().join("roost");
    let source = roost_dir.join("source");
    let target = roost_dir.join("target");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&target).unwrap();

    let source_file = source.join("nvim");
    fs::create_dir_all(&source_file).unwrap();
    fs::write(source_file.join("init.lua"), "shared").unwrap();

    let intermediate = target.join("nvim");
    symlink(&source_file, &intermediate).unwrap();

    let external = dir.path().join("nvim");
    symlink(&intermediate, &external).unwrap();

    unlink(&external, &target, &roost_dir).unwrap();

    assert!(!external.exists());
    assert!(!intermediate.exists());
    assert!(source_file.join("init.lua").exists());
    assert_eq!(
        fs::read_to_string(source_file.join("init.lua")).unwrap(),
        "shared"
    );
}

#[test]
fn test_restore_real_file_at_target_fails() {
    let dir = tempfile::TempDir::new().unwrap();
    let roost_dir = dir.path().join("roost");
    fs::create_dir_all(&roost_dir).unwrap();

    let dest = roost_dir.join("nvim");
    fs::create_dir_all(&dest).unwrap();
    fs::write(dest.join("config.toml"), "data").unwrap();

    let original = dir.path().join("nvim");
    fs::create_dir_all(&original).unwrap();
    fs::write(original.join("existing.txt"), "I exist").unwrap();

    let result = restore(&original, &dest, &roost_dir);
    assert!(result.is_err());
    assert!(original.join("existing.txt").exists());
}

#[test]
fn test_restore_already_linked_to_same_target_is_nop() {
    let dir = tempfile::TempDir::new().unwrap();
    let roost_dir = dir.path().join("roost");
    fs::create_dir_all(&roost_dir).unwrap();

    let dest = roost_dir.join("nvim");
    fs::create_dir_all(&dest).unwrap();
    fs::write(dest.join("init.lua"), "config").unwrap();

    let original = dir.path().join("nvim");
    symlink(&dest, &original).unwrap();

    restore(&original, &dest, &roost_dir).unwrap();

    assert!(original.is_symlink());
    assert_eq!(fs::read_link(&original).unwrap(), dest);
}
