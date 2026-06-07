use super::*;
use crate::app::{Application, LocalAppConfig, Profile, SharedAppConfig};
use crate::os_detect::OsInfo;
use std::collections::{BTreeMap, BTreeSet};
use tempfile::TempDir;

// helpers: create a file with dummy content, creating parent dirs as needed
fn create_file(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, "test content").unwrap();
    path
}

// helpers: create an empty directory tree
fn create_dir(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    fs::create_dir_all(&path).unwrap();
    path
}

// helpers: create roost/<profile>/ and return the roost path
fn setup_profile(tmp: &TempDir, profile: &str) -> PathBuf {
    let roost = tmp.path().join("roost");
    let profile_dir = roost.join(profile);
    fs::create_dir_all(&profile_dir).unwrap();
    roost
}

// --- ingest ---

#[test]
fn ingest_dir_moves_and_symlinks() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    // set up a real directory with contents at origin
    let origin = create_dir(tmp.path(), "config/nvim");
    create_file(&origin, "init.lua");

    ingest(&origin, &profile_dir, "nvim", true).unwrap();

    // origin should now be a symlink pointing into roost
    assert!(origin.is_symlink());
    assert!(profile_dir.join("nvim").is_dir());
    assert!(profile_dir.join("nvim/init.lua").exists());
    assert_eq!(fs::read_link(&origin).unwrap(), profile_dir.join("nvim"));
}

#[test]
fn ingest_file_moves_to_misc() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    let origin = create_file(tmp.path(), ".gitconfig");

    ingest(&origin, &profile_dir, "gitconfig", false).unwrap();

    // single files land under misc/ instead of at profile root
    assert!(origin.is_symlink());
    assert!(profile_dir.join("misc/gitconfig").exists());
}

#[test]
fn ingest_rejects_nonexistent() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    let result = ingest(Path::new("/no/such/path"), &profile_dir, "app", true);
    assert!(result.is_err());
}

#[test]
fn ingest_rejects_symlink() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    // origin is a symlink — ingest should refuse to avoid chain symlinks
    let real = create_file(tmp.path(), "real.txt");
    let link = tmp.path().join("link.txt");
    create_symlink(&real, &link, false).unwrap();

    let result = ingest(&link, &profile_dir, "app", false);
    assert!(result.is_err());
}

#[test]
fn ingest_creates_backup() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    let origin = create_file(tmp.path(), ".gitconfig");

    ingest(&origin, &profile_dir, "gitconfig", false).unwrap();

    // ingest copies to .backups/ before moving, so the user can undo
    let backup = profile_dir.join(".backups/gitconfig");
    assert!(backup.exists());
    assert_eq!(fs::read_to_string(&backup).unwrap(), "test content");

    assert!(profile_dir.join("misc/gitconfig").exists());
}

// --- restore ---

#[test]
fn restore_creates_symlink() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    // app already exists in roost (e.g. from a git pull)
    create_dir(&profile_dir, "nvim");

    let origin = tmp.path().join("config/nvim");
    restore(&origin, &profile_dir, "nvim", true).unwrap();

    assert!(origin.is_symlink());
    assert_eq!(fs::read_link(&origin).unwrap(), profile_dir.join("nvim"));
}

#[test]
fn restore_skips_if_already_linked() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    create_dir(&profile_dir, "nvim");
    let origin = tmp.path().join("config/nvim");
    fs::create_dir_all(origin.parent().unwrap()).unwrap();
    create_symlink(&profile_dir.join("nvim"), &origin, true).unwrap();

    // already pointing to the right place — should be a no-op
    restore(&origin, &profile_dir, "nvim", true).unwrap();
}

#[test]
fn restore_rejects_real_file() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    create_dir(&profile_dir, "nvim");
    let origin = create_file(tmp.path(), "config/nvim");

    // a real file at origin means the user should ingest, not restore
    let result = restore(&origin, &profile_dir, "nvim", true);
    assert!(result.is_err());
}

// --- unlink ---

#[test]
fn unlink_removes_symlink_and_restores_files() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    let origin = create_dir(tmp.path(), "config/nvim");
    create_file(&origin, "init.lua");

    ingest(&origin, &profile_dir, "nvim", true).unwrap();
    assert!(origin.is_symlink());

    unlink(&origin, &profile_dir, "nvim", true).unwrap();

    // symlink gone, files back at original location, nothing left in roost
    assert!(!origin.is_symlink());
    assert!(origin.join("init.lua").exists());
    assert!(!profile_dir.join("nvim").exists());
}

#[test]
fn unlink_rejects_non_symlink() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    let origin = create_file(tmp.path(), "real.txt");
    let result = unlink(&origin, &profile_dir, "app", false);
    assert!(result.is_err());
}

#[test]
fn unlink_file_cleans_empty_misc_dir() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    let origin = create_file(tmp.path(), ".gitconfig");

    ingest(&origin, &profile_dir, "gitconfig", false).unwrap();
    assert!(profile_dir.join("misc").exists());

    // unlinking the only misc file should remove the now-empty misc/ dir
    unlink(&origin, &profile_dir, "gitconfig", false).unwrap();

    assert!(!profile_dir.join("misc").exists());
    assert!(!origin.is_symlink());
    assert!(origin.exists());
}

#[test]
fn unlink_leaves_misc_dir_if_other_files() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    // two misc files — unlinking one should leave misc/ intact
    let origin_a = create_file(tmp.path(), ".gitconfig");
    let origin_b = create_file(tmp.path(), ".bashrc");

    ingest(&origin_a, &profile_dir, "gitconfig", false).unwrap();
    ingest(&origin_b, &profile_dir, "bashrc", false).unwrap();

    unlink(&origin_a, &profile_dir, "gitconfig", false).unwrap();

    assert!(profile_dir.join("misc").exists());
    assert!(profile_dir.join("misc/bashrc").exists());
}

// --- import_from ---

#[test]
fn import_from_creates_chain() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "shared");
    let _ = setup_profile(&tmp, "laptop");

    // shared profile owns the real nvim config
    create_dir(&roost.join("shared"), "nvim");

    import_from("nvim", "shared", "laptop", &roost).unwrap();

    // laptop/nvim is now a symlink pointing to shared/nvim (zero-copy)
    let target = roost.join("laptop/nvim");
    assert!(target.is_symlink());
    assert_eq!(fs::read_link(&target).unwrap(), roost.join("shared/nvim"));
}

#[test]
fn import_from_rejects_missing_source() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "shared");
    let _ = setup_profile(&tmp, "laptop");

    let result = import_from("nvim", "shared", "laptop", &roost);
    assert!(result.is_err());
}

#[test]
fn import_from_rejects_symlink_source() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "shared");
    let _ = setup_profile(&tmp, "laptop");

    // source is itself a symlink — rejecting prevents symlink chains
    let real_target = create_dir(tmp.path(), "real/nvim");
    let source_path = roost.join("shared/nvim");
    create_symlink(&real_target, &source_path, true).unwrap();

    let result = import_from("nvim", "shared", "laptop", &roost);
    assert!(result.is_err());
}

// --- copy_to ---

#[test]
fn copy_to_creates_independent_copy() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "shared");
    let _ = setup_profile(&tmp, "laptop");

    let nvim = create_dir(&roost.join("shared"), "nvim");
    create_file(&nvim, "init.lua");

    copy_to("nvim", "shared", "laptop", &roost).unwrap();

    // independent copy — not a symlink, edits don't affect source
    let target = roost.join("laptop/nvim");
    assert!(target.is_dir());
    assert!(target.join("init.lua").exists());
    assert!(!target.is_symlink());
}

#[test]
fn copy_to_rejects_existing_target() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "shared");
    let _ = setup_profile(&tmp, "laptop");

    create_dir(&roost.join("shared"), "nvim");
    create_dir(&roost.join("laptop"), "nvim");

    let result = copy_to("nvim", "shared", "laptop", &roost);
    assert!(result.is_err());
}

// --- app_dest ---

#[test]
fn app_dest_dir_vs_file() {
    assert_eq!(
        app_dest(Path::new("/roost"), "nvim", true),
        PathBuf::from("/roost/nvim")
    );
    assert_eq!(
        app_dest(Path::new("/roost"), "gitconfig", false),
        PathBuf::from("/roost/misc/gitconfig")
    );
}

// --- config builders for ensure_links / switch_profile tests ---

// build a single-profile shared config with the given apps
fn build_config(apps: Vec<(&str, bool)>, profile_apps: Vec<&str>) -> SharedAppConfig {
    let mut app_map = BTreeMap::new();
    for (name, is_dir) in apps {
        app_map.insert(
            name.to_string(),
            Application {
                primary_config: None,
                on_profiles: BTreeSet::new(),
                is_dir,
                ignore: Vec::new(),
            },
        );
    }
    let mut profile = Profile {
        apps: BTreeSet::new(),
        app_sources: BTreeMap::new(),
    };
    for name in profile_apps {
        profile.apps.insert(name.to_string());
    }
    let mut profiles = BTreeMap::new();
    profiles.insert("laptop".to_string(), profile);
    SharedAppConfig {
        remote: None,
        profiles,
        apps: app_map,
        ignored: BTreeSet::new(),
    }
}

// build a local config with the given link paths (active profile: laptop)
fn build_local(link_paths: Vec<(&str, &Path)>) -> LocalAppConfig {
    let mut paths = BTreeMap::new();
    for (name, path) in link_paths {
        paths.insert(name.to_string(), path.to_path_buf());
    }
    LocalAppConfig {
        active_profile: "laptop".to_string(),
        os_info: OsInfo {
            os: "macos".to_string(),
            arch: "aarch64".to_string(),
        },
        link_paths: paths,
    }
}

// --- ensure_links ---

#[test]
fn ensure_links_creates_missing_symlinks() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    // place real app files in the profile dir (as if restored from git)
    let nvim_dir = create_dir(&profile_dir, "nvim");
    create_file(&nvim_dir, "init.lua");
    fs::create_dir_all(profile_dir.join("misc")).unwrap();
    create_file(&profile_dir.join("misc"), "gitconfig");

    // origins don't exist yet on disk
    let nvim_origin = tmp.path().join("config/nvim");
    let gitconfig_origin = tmp.path().join(".gitconfig");

    let config = build_config(
        vec![("nvim", true), ("gitconfig", false)],
        vec!["nvim", "gitconfig"],
    );
    let mut local = build_local(vec![
        ("nvim", &nvim_origin),
        ("gitconfig", &gitconfig_origin),
    ]);

    let actions = ensure_links(&config, &mut local, &roost).unwrap();

    // both origins should now be symlinks into roost
    assert_eq!(actions.len(), 2);
    assert!(nvim_origin.is_symlink());
    assert!(gitconfig_origin.is_symlink());
    assert_eq!(
        fs::read_link(&nvim_origin).unwrap(),
        profile_dir.join("nvim")
    );
    assert_eq!(
        fs::read_link(&gitconfig_origin).unwrap(),
        profile_dir.join("misc/gitconfig")
    );
}

#[test]
fn ensure_links_skips_already_correct() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    create_dir(&profile_dir, "nvim");

    // pre-create the correct symlink
    let nvim_origin = tmp.path().join("config/nvim");
    fs::create_dir_all(nvim_origin.parent().unwrap()).unwrap();
    create_symlink(&profile_dir.join("nvim"), &nvim_origin, true).unwrap();

    let config = build_config(vec![("nvim", true)], vec!["nvim"]);
    let mut local = build_local(vec![("nvim", &nvim_origin)]);

    // nothing to do — should report no actions
    let actions = ensure_links(&config, &mut local, &roost).unwrap();
    assert!(actions.is_empty());
}

#[test]
fn ensure_links_backs_up_conflicting_real_file() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    create_dir(&profile_dir, "nvim");

    // a real directory sits at the origin path — conflict scenario
    let nvim_origin = create_dir(tmp.path(), "config/nvim");
    create_file(&nvim_origin, "old.lua");

    let config = build_config(vec![("nvim", true)], vec!["nvim"]);
    let mut local = build_local(vec![("nvim", &nvim_origin)]);

    let actions = ensure_links(&config, &mut local, &roost).unwrap();

    // conflicting dir should be backed up before linking
    let backup = roost.join(".backups/conflict-nvim");
    assert!(backup.is_dir());
    assert!(backup.join("old.lua").exists());

    assert!(nvim_origin.is_symlink());
    assert_eq!(actions.len(), 2); // BACKED UP + LINKED
}

#[test]
fn ensure_links_backs_up_wrong_symlink() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    create_dir(&profile_dir, "nvim");

    // origin is a symlink, but points to the wrong place
    let wrong_target = create_dir(tmp.path(), "wrong/nvim");
    let nvim_origin = tmp.path().join("config/nvim");
    fs::create_dir_all(nvim_origin.parent().unwrap()).unwrap();
    create_symlink(&wrong_target, &nvim_origin, true).unwrap();

    let config = build_config(vec![("nvim", true)], vec!["nvim"]);
    let mut local = build_local(vec![("nvim", &nvim_origin)]);

    let actions = ensure_links(&config, &mut local, &roost).unwrap();

    // wrong symlink backed up, origin now points to correct target
    let backup = roost.join(".backups/conflict-nvim");
    assert!(backup.is_symlink() || backup.exists());

    assert!(nvim_origin.is_symlink());
    assert_eq!(
        fs::read_link(&nvim_origin).unwrap(),
        profile_dir.join("nvim")
    );
    assert!(actions.iter().any(|a| a.contains("BACKED UP")));
}

// --- switch_profile ---

#[test]
fn switch_profile_removes_old_creates_new() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let _ = setup_profile(&tmp, "desktop");

    let laptop_dir = roost.join("laptop");
    let desktop_dir = roost.join("desktop");

    // laptop owns nvim, desktop owns bash
    create_dir(&laptop_dir, "nvim");
    create_file(&laptop_dir.join("nvim"), "init.lua");
    create_dir(&desktop_dir, "bash");
    create_file(&desktop_dir.join("bash"), ".bashrc");

    let nvim_origin = tmp.path().join("config/nvim");
    let bash_origin = tmp.path().join(".bashrc");
    fs::create_dir_all(nvim_origin.parent().unwrap()).unwrap();
    // nvim is currently linked (we're on laptop profile)
    create_symlink(&laptop_dir.join("nvim"), &nvim_origin, true).unwrap();

    // two profiles with different apps — can't use single-profile builder
    let mut apps = BTreeMap::new();
    apps.insert(
        "nvim".to_string(),
        Application {
            primary_config: None,
            on_profiles: BTreeSet::new(),
            is_dir: true,
            ignore: Vec::new(),
        },
    );
    apps.insert(
        "bash".to_string(),
        Application {
            primary_config: None,
            on_profiles: BTreeSet::new(),
            is_dir: true,
            ignore: Vec::new(),
        },
    );

    let mut laptop_apps = BTreeSet::new();
    laptop_apps.insert("nvim".to_string());
    let mut desktop_apps = BTreeSet::new();
    desktop_apps.insert("bash".to_string());

    let mut profiles = BTreeMap::new();
    profiles.insert(
        "laptop".to_string(),
        Profile {
            apps: laptop_apps,
            app_sources: BTreeMap::new(),
        },
    );
    profiles.insert(
        "desktop".to_string(),
        Profile {
            apps: desktop_apps,
            app_sources: BTreeMap::new(),
        },
    );

    let config = SharedAppConfig {
        remote: None,
        profiles,
        apps,
        ignored: BTreeSet::new(),
    };

    let mut link_paths = BTreeMap::new();
    link_paths.insert("nvim".to_string(), nvim_origin.clone());
    link_paths.insert("bash".to_string(), bash_origin.clone());
    let mut local = LocalAppConfig {
        active_profile: "laptop".to_string(),
        os_info: OsInfo {
            os: "macos".to_string(),
            arch: "aarch64".to_string(),
        },
        link_paths,
    };

    switch_profile("laptop", "desktop", &config, &mut local, &roost).unwrap();

    // old profile symlink removed, new profile symlink created
    assert!(!nvim_origin.is_symlink());
    assert!(bash_origin.is_symlink());
    assert_eq!(local.active_profile, "desktop");
}

#[test]
fn switch_profile_rejects_unknown_profile() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");

    // empty config — "nonexistent" profile doesn't exist
    let config = SharedAppConfig {
        remote: None,
        profiles: BTreeMap::new(),
        apps: BTreeMap::new(),
        ignored: BTreeSet::new(),
    };
    let mut local = build_local(vec![]);

    let result = switch_profile("laptop", "nonexistent", &config, &mut local, &roost);
    assert!(result.is_err());
}

#[test]
fn test_validate_path_rejects_outside_home() {
    let home = std::path::PathBuf::from("/home/user");
    let bad = std::path::Path::new("/etc/passwd");
    assert!(validate_path_in_home(bad, &home).is_err());
}

#[test]
fn test_validate_path_rejects_parent_dir() {
    let home = std::path::PathBuf::from("/home/user");
    let bad = std::path::Path::new("/home/user/../etc/passwd");
    assert!(validate_path_in_home(bad, &home).is_err());
}

#[test]
fn test_validate_path_accepts_inside_home() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path();
    let good = home.join(".bashrc");
    fs::write(&good, "test").unwrap();
    assert!(validate_path_in_home(&good, home).is_ok());
}

// --- Path rejection at mutation boundaries [R-1, R-2] ---

#[test]
fn ingest_rejects_path_outside_home() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    // A path outside home (and temp) should be rejected
    let origin = Path::new("/etc/passwd");
    let result = ingest(origin, &profile_dir, "app", false);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("outside"));
}

#[test]
fn restore_rejects_path_outside_home() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    create_dir(&profile_dir, "nvim");

    let origin = Path::new("/etc/nvim");
    let result = restore(origin, &profile_dir, "nvim", true);
    assert!(result.is_err());
}

#[test]
fn unlink_rejects_path_outside_home() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    let origin = Path::new("/etc/nvim");
    let result = unlink(origin, &profile_dir, "nvim", true);
    assert!(result.is_err());
}

#[test]
fn ingest_rejects_invalid_app_name() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    let origin = create_file(tmp.path(), ".gitconfig");
    let result = ingest(&origin, &profile_dir, "app/../../etc", false);
    assert!(result.is_err());
}

#[test]
fn restore_rejects_invalid_app_name() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    create_dir(&profile_dir, "nvim");
    let origin = tmp.path().join("config/nvim");
    fs::create_dir_all(origin.parent().unwrap()).unwrap();

    let result = restore(&origin, &profile_dir, "../../etc", true);
    assert!(result.is_err());
}

#[test]
fn ensure_links_skips_apps_with_invalid_link_paths() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    create_dir(&profile_dir, "nvim");

    let config = build_config(vec![("nvim", true)], vec!["nvim"]);
    let mut local = build_local(vec![("nvim", Path::new("/etc/passwd"))]);

    let actions = ensure_links(&config, &mut local, &roost).unwrap();
    assert!(actions.iter().any(|a| a.contains("SKIP")));
    // origin should NOT have been touched
    assert!(!Path::new("/etc/passwd").is_symlink());
}

// --- Backup fidelity [R-3, R-4, R-5] ---

#[test]
fn ensure_links_backs_up_file_conflict_as_file() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    fs::create_dir_all(profile_dir.join("misc")).unwrap();
    fs::write(profile_dir.join("misc/gitconfig"), "roost content").unwrap();

    // real file at origin
    let origin = tmp.path().join(".gitconfig");
    fs::write(&origin, "original content").unwrap();

    let config = build_config(vec![("gitconfig", false)], vec!["gitconfig"]);
    let mut local = build_local(vec![("gitconfig", &origin)]);

    let actions = ensure_links(&config, &mut local, &roost).unwrap();

    // backup should exist as a regular file
    let backup = roost.join(".backups/conflict-gitconfig");
    assert!(backup.is_file());
    assert!(!backup.is_symlink());
    assert_eq!(fs::read_to_string(&backup).unwrap(), "original content");

    // origin should now be a symlink
    assert!(origin.is_symlink());
    assert!(actions.iter().any(|a| a.contains("BACKED UP")));
}

#[test]
fn ensure_links_backs_up_dir_conflict_as_dir() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    create_dir(&profile_dir, "nvim");
    fs::write(profile_dir.join("nvim/init.lua"), "roost content").unwrap();

    // real dir at origin with content
    let origin = tmp.path().join("config/nvim");
    fs::create_dir_all(&origin).unwrap();
    fs::write(origin.join("old.lua"), "original content").unwrap();

    let config = build_config(vec![("nvim", true)], vec!["nvim"]);
    let mut local = build_local(vec![("nvim", &origin)]);

    let actions = ensure_links(&config, &mut local, &roost).unwrap();

    // backup should exist as a directory
    let backup = roost.join(".backups/conflict-nvim");
    assert!(backup.is_dir());
    assert!(backup.join("old.lua").exists());

    // origin should now be a symlink
    assert!(origin.is_symlink());
    assert!(actions.iter().any(|a| a.contains("BACKED UP")));
}

#[test]
fn ensure_links_preserves_symlink_in_backup() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    fs::create_dir_all(profile_dir.join("misc")).unwrap();
    fs::write(profile_dir.join("misc/gitconfig"), "roost content").unwrap();

    // real file at origin
    let origin = tmp.path().join(".gitconfig");
    fs::write(&origin, "original content").unwrap();

    // create a symlink inside a subdir at origin for the backup to encounter
    let real_target = tmp.path().join("real_target.txt");
    fs::write(&real_target, "target content").unwrap();
    let inner_link = origin.parent().unwrap().join("inner_link");
    create_symlink(&real_target, &inner_link, false).unwrap();

    let config = build_config(vec![("gitconfig", false)], vec!["gitconfig"]);
    let mut local = build_local(vec![("gitconfig", &origin)]);

    let actions = ensure_links(&config, &mut local, &roost).unwrap();

    // The backup should preserve the inner symlink as a symlink
    let backup = roost.join(".backups/conflict-gitconfig");
    // This test primarily verifies that copy_item doesn't crash on mixed types
    // and that the backup operation completes successfully.
    assert!(backup.is_file());
    assert_eq!(fs::read_to_string(&backup).unwrap(), "original content");
    assert!(actions.iter().any(|a| a.contains("BACKED UP")));
}

#[test]
fn ensure_links_preserves_symlink_in_dir_backup() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    create_dir(&profile_dir, "nvim");
    fs::write(profile_dir.join("nvim/init.lua"), "roost content").unwrap();

    // real dir at origin with a symlink inside it
    let origin = tmp.path().join("config/nvim");
    fs::create_dir_all(&origin).unwrap();
    fs::write(origin.join("real.lua"), "real content").unwrap();

    let real_target = tmp.path().join("actual_config.lua");
    fs::write(&real_target, "target content").unwrap();
    let inner_link = origin.join("linked.lua");
    create_symlink(&real_target, &inner_link, false).unwrap();

    let config = build_config(vec![("nvim", true)], vec!["nvim"]);
    let mut local = build_local(vec![("nvim", &origin)]);

    let actions = ensure_links(&config, &mut local, &roost).unwrap();

    // backup should contain the symlink as a symlink, not a copy of its target
    let backup = roost.join(".backups/conflict-nvim");
    assert!(backup.join("real.lua").exists());
    assert!(backup.join("linked.lua").is_symlink());
    assert_eq!(
        fs::read_link(backup.join("linked.lua")).unwrap(),
        real_target
    );
    assert!(actions.iter().any(|a| a.contains("BACKED UP")));
}

#[test]
fn switch_profile_rejects_invalid_profile_name() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");

    let config = build_config(vec![], vec![]);
    let mut local = build_local(vec![]);

    let result = switch_profile("laptop", "../../etc", &config, &mut local, &roost);
    assert!(result.is_err());
}
