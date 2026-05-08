use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn setup_roost(dir: &std::path::Path) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        dir.join("roost.toml"),
        "[profiles.default]\napps = []\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("local.toml"),
        "active_profile = \"default\"\n\n[os_info]\nos = \"test\"\narch = \"x86_64\"\n\n[link_paths]\n",
    )
    .unwrap();
    std::fs::write(dir.join(".gitignore"), "local.toml\n").unwrap();
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

fn write_shared_with_app(dir: &std::path::Path, app_name: &str, is_dir: bool) {
    std::fs::write(
        dir.join("roost.toml"),
        format!(
            "[profiles.default]\napps = [\"{app_name}\"]\n\n[apps.{app_name}]\nis_dir = {is_dir}\non_profiles = [\"default\"]\n"
        ),
    )
    .unwrap();
}

#[test]
fn where_reports_correct_path_for_dir_app() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);
    write_shared_with_app(dir, "myapp", true);

    let expected = dir.join("default").join("myapp").display().to_string();
    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .arg("where")
        .arg("myapp")
        .assert()
        .success()
        .stdout(predicate::str::contains(expected));
}

#[test]
fn where_reports_correct_path_for_file_app() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);
    write_shared_with_app(dir, "myconfig", false);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .arg("where")
        .arg("myconfig")
        .assert()
        .success()
        .stdout(predicate::str::contains("misc/myconfig"));
}

#[test]
fn where_errors_for_unknown_app() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .arg("where")
        .arg("nonexistent")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn where_errors_without_init() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .arg("where")
        .arg("foo")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not initialized"));
}
