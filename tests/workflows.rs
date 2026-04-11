mod helpers;

use helpers::TestRoost;
use predicates::str::contains;
use std::fs;

fn setup() -> TestRoost {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost
}

fn add_app_dir(roost: &TestRoost, name: &str, filename: &str, content: &str) -> std::path::PathBuf {
    let dir = roost.path(&format!(".config/{}", name));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(filename), content).unwrap();
    roost
        .cmd()
        .args(["add", dir.to_str().unwrap()])
        .assert()
        .success();
    dir
}

#[test]
fn test_full_add_remove_cycle() {
    let roost = setup();

    let nvim_dir = add_app_dir(&roost, "nvim", "init.lua", "vim.opt.number = true");

    assert!(nvim_dir.is_symlink());
    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(config.apps.contains_key("nvim"));

    let profile_file = roost.roost_dir.join("default/nvim/init.lua");
    assert_eq!(
        fs::read_to_string(&profile_file).unwrap(),
        "vim.opt.number = true"
    );

    roost
        .cmd()
        .args(["remove", "nvim"])
        .write_stdin("y\n")
        .assert()
        .success()
        .stdout(contains("Removed 'nvim'"));

    assert!(!nvim_dir.is_symlink(), "should no longer be a symlink");
    assert!(
        nvim_dir.join("init.lua").exists(),
        "file should be restored"
    );
    assert_eq!(
        fs::read_to_string(nvim_dir.join("init.lua")).unwrap(),
        "vim.opt.number = true",
        "content preserved through cycle"
    );

    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(!config.apps.contains_key("nvim"), "app removed from config");
}

#[test]
fn test_profile_switch_with_apps() {
    let roost = setup();

    let nvim_dir = add_app_dir(&roost, "nvim", "init.lua", "print('hi')");

    assert!(nvim_dir.is_symlink());

    roost
        .cmd()
        .args(["profile", "add", "work", "--empty"])
        .assert()
        .success()
        .stdout(contains("Created empty profile 'work'"));

    roost
        .cmd()
        .args(["profile", "switch", "work"])
        .assert()
        .success()
        .stdout(contains("Switched to profile 'work'"));

    let local = roost::app::LocalAppConfig::load(&roost.local_config).unwrap();
    assert_eq!(local.active_profile, "work");

    assert!(
        !nvim_dir.exists(),
        "symlink should be removed when switching to profile that doesn't have the app"
    );

    let default_store = roost.roost_dir.join("default/nvim");
    assert!(
        default_store.join("init.lua").exists(),
        "files still in roost/default/"
    );

    roost
        .cmd()
        .args(["profile", "switch", "default"])
        .assert()
        .success()
        .stdout(contains("Switched to profile 'default'"));

    assert!(
        nvim_dir.is_symlink(),
        "symlink present after switching back to default"
    );
    let target = fs::read_link(&nvim_dir).unwrap();
    assert_eq!(target, roost.roost_dir.join("default").join("nvim"));
}

#[test]
fn test_add_multiple_apps_same_profile() {
    let roost = setup();

    for (name, file, content) in [
        ("nvim", "init.lua", "vim.opt.number = true"),
        ("ghostty", "config", "theme = dark"),
        ("zellij", "config.kdl", "pane"),
    ] {
        let dir = roost.path(&format!(".config/{}", name));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(file), content).unwrap();
        roost
            .cmd()
            .args(["add", dir.to_str().unwrap()])
            .assert()
            .success();
    }

    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert_eq!(config.apps.len(), 3);
    assert!(config.apps.contains_key("nvim"));
    assert!(config.apps.contains_key("ghostty"));
    assert!(config.apps.contains_key("zellij"));

    roost
        .cmd()
        .arg("status")
        .assert()
        .success()
        .stdout(contains("Apps managed: 3"));
}

#[test]
fn test_doctor_after_clean_operations() {
    let roost = setup();

    roost
        .cmd()
        .arg("doctor")
        .assert()
        .success()
        .stdout(contains("All checks passed"));

    let nvim_dir = add_app_dir(&roost, "nvim", "init.lua", "hello");

    roost
        .cmd()
        .arg("doctor")
        .assert()
        .success()
        .stdout(contains("All checks passed"));

    drop(nvim_dir);

    roost
        .cmd()
        .args(["remove", "nvim"])
        .write_stdin("y\n")
        .assert()
        .success();

    roost
        .cmd()
        .arg("doctor")
        .assert()
        .success()
        .stdout(contains("All checks passed"));
}

#[test]
fn test_nested_git_removed_on_ingest() {
    let roost = setup();

    let myapp = roost.path(".config/myapp");
    fs::create_dir_all(myapp.join(".git/objects")).unwrap();
    fs::write(myapp.join(".git/HEAD"), "ref: refs/heads/main").unwrap();
    fs::write(myapp.join("config.toml"), "key = value").unwrap();
    fs::write(myapp.join("data.txt"), "important").unwrap();

    roost
        .cmd()
        .args(["add", myapp.to_str().unwrap()])
        .assert()
        .success();

    let store = roost.roost_dir.join("default/myapp");
    assert!(
        !store.join(".git").exists(),
        ".git should be removed from roost store"
    );
    assert!(
        store.join("config.toml").exists(),
        "regular files preserved"
    );
    assert!(
        store.join("data.txt").exists(),
        "all regular files preserved"
    );
}

#[test]
fn test_clone_profile_copies_files() {
    let roost = setup();

    let nvim_dir = add_app_dir(&roost, "nvim", "init.lua", "vim.cmd('hi')");

    assert!(nvim_dir.is_symlink());

    roost
        .cmd()
        .args(["profile", "add", "laptop"])
        .assert()
        .success()
        .stdout(contains("Created profile 'laptop'"));

    let cloned = roost.roost_dir.join("laptop/nvim");
    assert!(
        cloned.join("init.lua").exists(),
        "files copied to cloned profile"
    );
    assert_eq!(
        fs::read_to_string(cloned.join("init.lua")).unwrap(),
        "vim.cmd('hi')",
        "content matches in cloned profile"
    );
}

#[test]
fn test_log_tracks_all_operations() {
    let roost = setup();
    roost.init_git();

    let nvim_dir = add_app_dir(&roost, "nvim", "init.lua", "set number");

    roost
        .cmd()
        .args(["remove", "nvim"])
        .write_stdin("y\n")
        .assert()
        .success();

    drop(nvim_dir);

    let entries = roost::git::log(&roost.roost_dir, 10).unwrap();
    let messages: Vec<&str> = entries.iter().map(|e| e.message.as_str()).collect();

    assert!(
        messages.iter().any(|m| m.contains("added app")),
        "expected 'added app' in log, got: {:?}",
        messages
    );
    assert!(
        messages.iter().any(|m| m.contains("removed app")),
        "expected 'removed app' in log, got: {:?}",
        messages
    );
}
