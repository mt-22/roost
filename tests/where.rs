use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn setup_roost(dir: &std::path::Path) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("roost.toml"), "[profiles.default]\napps = []\n").unwrap();
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
    std::fs::write(
        dir.join("local.toml"),
        format!(
            "active_profile = \"default\"\n\n[os_info]\nos = \"test\"\narch = \"x86_64\"\n\n[link_paths]\n{app_name} = \"/fake/path/{app_name}\"\n"
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

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .arg("where")
        .arg("myapp")
        .assert()
        .success()
        .stdout(predicate::str::contains("/fake/path/myapp"));
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
        .stdout(predicate::str::contains("/fake/path/myconfig"));
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

#[test]
fn where_with_profile_flag() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);
    std::fs::write(
        dir.join("roost.toml"),
        r#"
ignored = []

[profiles]
[profiles.default]
apps = ["myapp"]
app_sources = {}

[profiles.work]
apps = ["workapp"]
app_sources = {}

[apps.myapp]
is_dir = true
on_profiles = ["default"]

[apps.workapp]
is_dir = false
on_profiles = ["work"]
"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("local.toml"),
        r#"
active_profile = "default"

[os_info]
os = "test"
arch = "x86_64"

[link_paths]
myapp = "/fake/path/myapp"
workapp = "/fake/path/workapp"
"#,
    )
    .unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["where", "workapp", "--profile", "work"])
        .assert()
        .success()
        .stdout(predicate::str::contains("workapp"));
}

#[test]
fn where_with_profile_flag_errors_for_app_not_in_profile() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);
    std::fs::write(
        dir.join("roost.toml"),
        r#"
ignored = []

[profiles]
[profiles.default]
apps = ["myapp"]
app_sources = {}

[profiles.work]
apps = []
app_sources = {}

[apps.myapp]
is_dir = true
on_profiles = ["default"]
"#,
    )
    .unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["where", "myapp", "--profile", "work"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn where_with_profile_flag_errors_for_unknown_profile() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["where", "foo", "--profile", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}
