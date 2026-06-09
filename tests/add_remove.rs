use assert_cmd::Command;
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
    assert_eq!(fs::read_to_string(test_dir.join("config.yml")).unwrap(), "data");
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
