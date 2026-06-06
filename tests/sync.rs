use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;
use tempfile::TempDir;

fn setup_git_repo(dir: &Path) {
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
}

fn commit_all(dir: &Path, message: &str) {
    std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(dir)
        .output()
        .unwrap();
}

fn setup_roost_with_remote(roost_dir: &Path, remote_dir: &Path) {
    std::fs::create_dir_all(roost_dir).unwrap();
    setup_git_repo(roost_dir);

    // Add remote
    std::process::Command::new("git")
        .args(["remote", "add", "origin", &remote_dir.to_string_lossy()])
        .current_dir(roost_dir)
        .output()
        .unwrap();

    std::fs::write(
        roost_dir.join("roost.toml"),
        format!(
            r#"
remote = "{}"
ignored = []

[profiles]
[profiles.default]
apps = []
app_sources = {{}}

[apps]
"#,
            remote_dir.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();

    std::fs::write(
        roost_dir.join("local.toml"),
        r#"
active_profile = "default"

[os_info]
os = "test"
arch = "x86_64"

[link_paths]
"#,
    )
    .unwrap();

    std::fs::write(roost_dir.join(".gitignore"), "local.toml\n").unwrap();
    commit_all(roost_dir, "init");
}

#[test]
fn sync_errors_without_init() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .arg("sync")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not initialized"));
}

#[test]
fn sync_errors_without_remote() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    std::fs::create_dir_all(dir).unwrap();
    setup_git_repo(dir);

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

    commit_all(dir, "init");

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", dir)
        .arg("sync")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no remote configured"));
}

#[test]
fn sync_no_op_when_up_to_date() {
    let tmp = TempDir::new().unwrap();
    let remote_tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path();
    let remote_dir = remote_tmp.path();

    // Create bare remote
    std::fs::create_dir_all(remote_dir).unwrap();
    std::process::Command::new("git")
        .args(["init", "--bare"])
        .current_dir(remote_dir)
        .output()
        .unwrap();

    setup_roost_with_remote(roost_dir, remote_dir);

    // Push initial state so remote matches local
    std::process::Command::new("git")
        .args(["push", "origin", "main"])
        .current_dir(roost_dir)
        .output()
        .unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", roost_dir)
        .arg("sync")
        .assert()
        .success()
        .stdout(predicate::str::contains("Sync complete"));
}

#[test]
fn sync_pulls_remote_changes() {
    let tmp = TempDir::new().unwrap();
    let remote_tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path();
    let remote_dir = remote_tmp.path();

    std::fs::create_dir_all(remote_dir).unwrap();
    std::process::Command::new("git")
        .args(["init", "--bare"])
        .current_dir(remote_dir)
        .output()
        .unwrap();

    setup_roost_with_remote(roost_dir, remote_dir);
    std::process::Command::new("git")
        .args(["push", "origin", "main"])
        .current_dir(roost_dir)
        .output()
        .unwrap();

    // Simulate another device pushing changes: add an app to remote roost.toml
    let clone_tmp = TempDir::new().unwrap();
    let clone_dir = clone_tmp.path();
    std::process::Command::new("git")
        .args(["clone", &remote_dir.to_string_lossy(), "."])
        .current_dir(clone_dir)
        .output()
        .unwrap();

    std::fs::write(
        clone_dir.join("roost.toml"),
        format!(
            r#"
remote = "{}"
ignored = []

[profiles]
[profiles.default]
apps = ["nvim"]
app_sources = {{}}

[apps.nvim]
primary_config = "init.lua"
is_dir = true
on_profiles = ["default"]
ignore = []
"#,
            remote_dir.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
    setup_git_repo(clone_dir); // already cloned, just ensure identity
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(clone_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(clone_dir)
        .output()
        .unwrap();
    commit_all(clone_dir, "add nvim");
    std::process::Command::new("git")
        .args(["push", "origin", "main"])
        .current_dir(clone_dir)
        .output()
        .unwrap();

    // Now local roost should pull the nvim app on sync
    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", roost_dir)
        .arg("sync")
        .assert()
        .success()
        .stdout(predicate::str::contains("Sync"));

    // Verify roost.toml now contains nvim
    let contents = std::fs::read_to_string(roost_dir.join("roost.toml")).unwrap();
    assert!(contents.contains("nvim"));
}

#[test]
fn sync_pushes_local_changes() {
    let tmp = TempDir::new().unwrap();
    let remote_tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path();
    let remote_dir = remote_tmp.path();

    std::fs::create_dir_all(remote_dir).unwrap();
    std::process::Command::new("git")
        .args(["init", "--bare"])
        .current_dir(remote_dir)
        .output()
        .unwrap();

    setup_roost_with_remote(roost_dir, remote_dir);
    std::process::Command::new("git")
        .args(["push", "origin", "main"])
        .current_dir(roost_dir)
        .output()
        .unwrap();

    // Make local change
    std::fs::write(
        roost_dir.join("roost.toml"),
        format!(
            r#"
remote = "{}"
ignored = []

[profiles]
[profiles.default]
apps = ["bash"]
app_sources = {{}}

[apps.bash]
primary_config = ".bashrc"
is_dir = false
on_profiles = ["default"]
ignore = []
"#,
            remote_dir.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();

    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", roost_dir)
        .arg("sync")
        .assert()
        .success()
        .stdout(predicate::str::contains("Sync complete"));

    // Verify remote has the change
    let clone_tmp = TempDir::new().unwrap();
    let clone_dir = clone_tmp.path();
    std::process::Command::new("git")
        .args(["clone", &remote_dir.to_string_lossy(), "."])
        .current_dir(clone_dir)
        .output()
        .unwrap();
    let contents = std::fs::read_to_string(clone_dir.join("roost.toml")).unwrap();
    assert!(contents.contains("bash"));
}

#[test]
fn sync_detects_conflict() {
    let tmp = TempDir::new().unwrap();
    let remote_tmp = TempDir::new().unwrap();
    let roost_dir = tmp.path();
    let remote_dir = remote_tmp.path();

    std::fs::create_dir_all(remote_dir).unwrap();
    std::process::Command::new("git")
        .args(["init", "--bare"])
        .current_dir(remote_dir)
        .output()
        .unwrap();

    setup_roost_with_remote(roost_dir, remote_dir);
    std::process::Command::new("git")
        .args(["push", "origin", "main"])
        .current_dir(roost_dir)
        .output()
        .unwrap();

    // Remote sets nvim as file, local will set it as dir
    let clone_tmp = TempDir::new().unwrap();
    let clone_dir = clone_tmp.path();
    std::process::Command::new("git")
        .args(["clone", &remote_dir.to_string_lossy(), "."])
        .current_dir(clone_dir)
        .output()
        .unwrap();

    std::fs::write(
        clone_dir.join("roost.toml"),
        format!(
            r#"
remote = "{}"
ignored = []

[profiles]
[profiles.default]
apps = ["nvim"]
app_sources = {{}}

[apps.nvim]
primary_config = ".nvimrc"
is_dir = false
on_profiles = ["default"]
ignore = []
"#,
            remote_dir.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(clone_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(clone_dir)
        .output()
        .unwrap();
    commit_all(clone_dir, "add nvim as file");
    std::process::Command::new("git")
        .args(["push", "origin", "main"])
        .current_dir(clone_dir)
        .output()
        .unwrap();

    // Local sets nvim as dir
    std::fs::write(
        roost_dir.join("roost.toml"),
        format!(
            r#"
remote = "{}"
ignored = []

[profiles]
[profiles.default]
apps = ["nvim"]
app_sources = {{}}

[apps.nvim]
primary_config = "init.lua"
is_dir = true
on_profiles = ["default"]
ignore = []
"#,
            remote_dir.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
    commit_all(roost_dir, "add nvim as dir");

    // Sync with local preference should detect the roost.toml conflict during rebase
    // and report file conflicts (both branches touched roost.toml)
    Command::cargo_bin("roost")
        .unwrap()
        .env("ROOST_DIR", roost_dir)
        .arg("sync")
        .assert()
        .success()
        .stdout(predicate::str::contains("file conflicts"));
}
