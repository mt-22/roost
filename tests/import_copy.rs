use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn setup_roost(roost_dir: &std::path::Path) {
    fs::create_dir_all(roost_dir.join("laptop")).unwrap();
    fs::create_dir_all(roost_dir.join("desktop")).unwrap();

    let roost_toml = r#"
[apps.nvim]
is_dir = true

[profiles.desktop]
apps = ["nvim"]

[profiles.laptop]
apps = []
"#;
    fs::write(roost_dir.join("roost.toml"), roost_toml).unwrap();

    let local_toml = r#"
active_profile = "laptop"
[os_info]
os = "macos"
arch = "aarch64"
[link_paths]
"#;
    fs::write(roost_dir.join("local.toml"), local_toml).unwrap();

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(roost_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(roost_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(roost_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(roost_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(roost_dir)
        .output()
        .unwrap();
}

fn setup_roost_with_app(roost_dir: &std::path::Path, app_name: &str, profile: &str) {
    setup_roost(roost_dir);

    fs::create_dir_all(roost_dir.join(profile).join(app_name)).unwrap();
    fs::write(
        roost_dir.join(profile).join(app_name).join("config"),
        "content",
    )
    .unwrap();

    let other_profile = if profile == "laptop" {
        "desktop"
    } else {
        "laptop"
    };

    let roost_toml = format!(
        r#"
[apps.{}]
is_dir = true

[profiles.{}]
apps = ["{}"]

[profiles.{}]
apps = []
"#,
        app_name, profile, app_name, other_profile
    );
    fs::write(roost_dir.join("roost.toml"), roost_toml).unwrap();
    // local.toml stays as-is from setup_roost (active_profile = "laptop")

    std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(roost_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "add app"])
        .current_dir(roost_dir)
        .output()
        .unwrap();
}

// --- Import tests ---

#[test]
fn import_app_creates_cross_profile_symlink() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost_with_app(&roost_dir, "nvim", "desktop");

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .args(["import", "nvim", "--from", "desktop"])
        .assert()
        .success();

    let target = roost_dir.join("laptop/nvim");
    assert!(target.is_symlink());
    assert_eq!(
        fs::read_link(&target).unwrap(),
        roost_dir.join("desktop/nvim")
    );

    let config: toml::Value =
        toml::from_str(&fs::read_to_string(roost_dir.join("roost.toml")).unwrap()).unwrap();
    let laptop_apps = config["profiles"]["laptop"]["apps"].as_array().unwrap();
    assert!(laptop_apps.iter().any(|v| v.as_str() == Some("nvim")));
    let app_sources = config["profiles"]["laptop"]["app_sources"]
        .as_table()
        .unwrap();
    assert_eq!(app_sources["nvim"].as_str(), Some("desktop"));
}

#[test]
fn import_app_rejects_missing_app() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .args(["import", "nonexistent", "--from", "desktop"])
        .assert()
        .failure();
}

#[test]
fn import_app_rejects_missing_profile() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .args(["import", "nvim", "--from", "ghost"])
        .assert()
        .failure();
}

// --- Copy tests ---

#[test]
fn copy_app_creates_independent_copy() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost_with_app(&roost_dir, "nvim", "laptop");

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .args(["copy", "nvim", "--to", "desktop"])
        .assert()
        .success();

    let desktop_target = roost_dir.join("desktop/nvim");
    assert!(desktop_target.is_dir());
    assert!(desktop_target.join("config").exists());
    assert!(!desktop_target.is_symlink());

    let laptop_target = roost_dir.join("laptop/nvim");
    assert!(laptop_target.is_dir());
    assert!(laptop_target.join("config").exists());

    let config: toml::Value =
        toml::from_str(&fs::read_to_string(roost_dir.join("roost.toml")).unwrap()).unwrap();
    let desktop_apps = config["profiles"]["desktop"]["apps"].as_array().unwrap();
    assert!(desktop_apps.iter().any(|v| v.as_str() == Some("nvim")));
}

#[test]
fn copy_app_rejects_app_not_in_active_profile() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost(&roost_dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .args(["copy", "nvim", "--to", "desktop"])
        .assert()
        .failure();
}

#[test]
fn copy_app_rejects_missing_profile() {
    let tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path().join("roost");
    setup_roost_with_app(&roost_dir, "nvim", "laptop");

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", &roost_dir)
        .args(["copy", "nvim", "--to", "ghost"])
        .assert()
        .failure();
}
