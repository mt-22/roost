use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn setup_roost(dir: &std::path::Path) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join("roost.toml"),
        r#"
ignored = []

[profiles]
[profiles.default]
apps = []
app_sources = {}

[apps]
"#,
    )
    .unwrap();
    fs::write(
        dir.join("local.toml"),
        r#"
active_profile = "default"

[os_info]
os = "test"
arch = "x86_64"

[link_paths]
"#,
    )
    .unwrap();
    fs::write(dir.join(".gitignore"), "local.toml\n").unwrap();
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(dir)
        .output()
        .unwrap();
}

#[test]
fn add_file_moves_into_roost() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);
    let test_file = tmp.path().join("testconfig.toml");
    fs::write(&test_file, "content").unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("add")
        .arg(&test_file)
        .assert()
        .success();

    assert!(test_file.is_symlink());
    assert!(
        roost_dir
            .join("default")
            .join("misc")
            .join("testconfig.toml")
            .exists()
    );
}

#[test]
fn add_dir_moves_into_roost() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);
    let test_dir = tmp.path().join("myapp");
    fs::create_dir_all(&test_dir).unwrap();
    fs::write(test_dir.join("config.yml"), "data").unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("add")
        .arg(&test_dir)
        .assert()
        .success();

    assert!(test_dir.is_symlink());
    assert!(roost_dir.join("default").join("myapp").exists());
}

#[test]
fn add_updates_config() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);
    let test_file = tmp.path().join("testconfig.toml");
    fs::write(&test_file, "content").unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("add")
        .arg(&test_file)
        .assert()
        .success();

    let config = fs::read_to_string(roost_dir.join("roost.toml")).unwrap();
    assert!(config.contains("testconfig"));
}

#[test]
fn remove_restores_files() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);
    let test_file = tmp.path().join("testconfig.toml");
    fs::write(&test_file, "content").unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("add")
        .arg(&test_file)
        .assert()
        .success();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("remove")
        .arg("testconfig.toml")
        .assert()
        .success();

    assert!(!test_file.is_symlink());
    assert!(test_file.exists());
    assert_eq!(fs::read_to_string(&test_file).unwrap(), "content");
}

#[test]
fn remove_cleans_config() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);
    let test_file = tmp.path().join("testconfig.toml");
    fs::write(&test_file, "content").unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("add")
        .arg(&test_file)
        .assert()
        .success();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("remove")
        .arg("testconfig.toml")
        .assert()
        .success();

    let config = fs::read_to_string(roost_dir.join("roost.toml")).unwrap();
    let parsed: toml::Value = toml::from_str(&config).unwrap();
    let apps = parsed.get("apps").unwrap().as_table().unwrap();
    assert!(!apps.contains_key("testconfig.toml"));
}

#[test]
fn add_nonexistent_path_fails() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("add")
        .arg("/nonexistent/path/that/does/not/exist")
        .assert()
        .failure();
}

#[test]
fn remove_all_restores_files() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);
    let test_file = tmp.path().join("testconfig.toml");
    fs::write(&test_file, "content").unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("add")
        .arg(&test_file)
        .assert()
        .success();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("remove")
        .arg("--all")
        .assert()
        .success();

    assert!(!test_file.is_symlink());
    assert!(test_file.exists());
    assert_eq!(fs::read_to_string(&test_file).unwrap(), "content");
}

#[test]
fn remove_all_restores_directories() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);
    let test_dir = tmp.path().join("myapp");
    fs::create_dir_all(&test_dir).unwrap();
    fs::write(test_dir.join("config.yml"), "data").unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("add")
        .arg(&test_dir)
        .assert()
        .success();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("remove")
        .arg("--all")
        .assert()
        .success();

    assert!(!test_dir.is_symlink());
    assert!(test_dir.is_dir());
    assert!(test_dir.join("config.yml").exists());
    assert_eq!(
        fs::read_to_string(test_dir.join("config.yml")).unwrap(),
        "data"
    );
}

#[test]
fn add_existing_unlinked_app_to_new_active_profile_sets_local_path() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);
    fs::write(
        roost_dir.join("roost.toml"),
        r#"
ignored = []

[profiles]
[profiles.default]
apps = ["nvim"]
app_sources = {}

[profiles.work]
apps = []
app_sources = {}

[apps.nvim]
is_dir = true
on_profiles = ["default"]
ignore = []
"#,
    )
    .unwrap();
    fs::write(
        roost_dir.join("local.toml"),
        r#"
active_profile = "work"

[os_info]
os = "test"
arch = "x86_64"

[link_paths]
"#,
    )
    .unwrap();
    let nvim_dir = tmp.path().join("nvim");
    fs::create_dir_all(&nvim_dir).unwrap();
    fs::write(nvim_dir.join("init.lua"), "vim.o.number = true").unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("add")
        .arg(&nvim_dir)
        .assert()
        .success();

    assert!(nvim_dir.is_symlink());
    assert!(
        roost_dir
            .join("work")
            .join("nvim")
            .join("init.lua")
            .exists()
    );

    let shared: toml::Value =
        toml::from_str(&fs::read_to_string(roost_dir.join("roost.toml")).unwrap()).unwrap();
    let default_apps = shared["profiles"]["default"]["apps"].as_array().unwrap();
    let work_apps = shared["profiles"]["work"]["apps"].as_array().unwrap();
    let on_profiles = shared["apps"]["nvim"]["on_profiles"].as_array().unwrap();
    assert!(default_apps.iter().any(|app| app.as_str() == Some("nvim")));
    assert!(work_apps.iter().any(|app| app.as_str() == Some("nvim")));
    assert!(
        on_profiles
            .iter()
            .any(|profile| profile.as_str() == Some("default"))
    );
    assert!(
        on_profiles
            .iter()
            .any(|profile| profile.as_str() == Some("work"))
    );

    let local = fs::read_to_string(roost_dir.join("local.toml")).unwrap();
    assert!(local.contains("nvim"));
    assert!(local.contains(&nvim_dir.display().to_string()));
}

#[test]
fn add_existing_unlinked_app_on_active_profile_fails_with_recovery_hint() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);
    fs::write(
        roost_dir.join("roost.toml"),
        r#"
ignored = []

[profiles]
[profiles.default]
apps = ["nvim"]
app_sources = {}

[apps.nvim]
is_dir = true
on_profiles = ["default"]
ignore = []
"#,
    )
    .unwrap();
    let nvim_dir = tmp.path().join("nvim");
    fs::create_dir_all(&nvim_dir).unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("add")
        .arg(&nvim_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("already on this profile"))
        .stderr(predicate::str::contains("roost add"));
}

#[test]
fn add_existing_app_with_local_path_still_fails_as_already_managed() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);
    let existing_path = tmp.path().join("existing-nvim");
    fs::create_dir_all(&existing_path).unwrap();
    fs::write(
        roost_dir.join("roost.toml"),
        r#"
ignored = []

[profiles]
[profiles.default]
apps = ["nvim"]
app_sources = {}

[apps.nvim]
is_dir = true
on_profiles = ["default"]
ignore = []
"#,
    )
    .unwrap();
    fs::write(
        roost_dir.join("local.toml"),
        format!(
            r#"
active_profile = "default"

[os_info]
os = "test"
arch = "x86_64"

[link_paths]
nvim = "{}"
"#,
            existing_path.display()
        ),
    )
    .unwrap();
    let nvim_dir = tmp.path().join("nvim");
    fs::create_dir_all(&nvim_dir).unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("add")
        .arg(&nvim_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("already managed"));
}

#[test]
fn remove_all_cleans_config() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);
    let test_file = tmp.path().join("testconfig.toml");
    fs::write(&test_file, "content").unwrap();
    let test_dir = tmp.path().join("myapp");
    fs::create_dir_all(&test_dir).unwrap();
    fs::write(test_dir.join("config.yml"), "data").unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("add")
        .arg(&test_file)
        .assert()
        .success();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("add")
        .arg(&test_dir)
        .assert()
        .success();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("remove")
        .arg("--all")
        .assert()
        .success();

    let config = fs::read_to_string(roost_dir.join("roost.toml")).unwrap();
    let parsed: toml::Value = toml::from_str(&config).unwrap();
    let apps = parsed.get("apps").unwrap().as_table().unwrap();
    assert!(apps.is_empty());
    let profiles = parsed.get("profiles").unwrap().as_table().unwrap();
    let default = profiles.get("default").unwrap().as_table().unwrap();
    let apps_list = default.get("apps").unwrap().as_array().unwrap();
    assert!(apps_list.is_empty());
}

#[test]
fn remove_all_empty_profile() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("remove")
        .arg("--all")
        .assert()
        .success();
}

#[test]
fn remove_all_requires_all_flag_or_app_name() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("remove")
        .assert()
        .failure();
}

#[test]
fn remove_unknown_app_fails() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .arg("remove")
        .arg("nonexistent")
        .assert()
        .failure();
}
