use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn setup_roost(dir: &std::path::Path) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
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
    std::fs::write(
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

#[test]
fn list_errors_without_init() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .arg("list")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not initialized"));
}

#[test]
fn list_shows_empty_profile() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("No managed apps").or(predicate::str::is_empty()));
}

#[test]
fn list_shows_managed_apps() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);
    std::fs::write(
        dir.join("roost.toml"),
        r#"
ignored = []

[profiles]
[profiles.default]
apps = ["myapp", "other"]
app_sources = {}

[apps.myapp]
is_dir = true
on_profiles = ["default"]

[apps.other]
is_dir = false
on_profiles = ["default"]
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
other = "/fake/path/other"
"#,
    )
    .unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("myapp"))
        .stdout(predicate::str::contains("other"));
}

#[test]
fn list_with_profile_flag() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);
    std::fs::write(
        dir.join("roost.toml"),
        r#"
ignored = []

[profiles]
[profiles.default]
apps = ["shared"]
app_sources = {}

[profiles.work]
apps = ["shared", "workonly"]
app_sources = {}

[apps.shared]
is_dir = true
on_profiles = ["default", "work"]

[apps.workonly]
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
shared = "/fake/path/shared"
workonly = "/fake/path/workonly"
"#,
    )
    .unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["list", "--profile", "work"])
        .assert()
        .success()
        .stdout(predicate::str::contains("workonly"))
        .stdout(predicate::str::contains("shared"));
}

#[test]
fn list_marks_apps_without_local_link_paths_as_unlinked() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);
    std::fs::write(
        dir.join("roost.toml"),
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

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("(!)"))
        .stdout(predicate::str::contains("nvim"))
        .stdout(predicate::str::contains("[unlinked: no local path]"));
}

#[test]
fn list_errors_for_unknown_profile() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    setup_roost(dir);

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .args(["list", "--profile", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}
