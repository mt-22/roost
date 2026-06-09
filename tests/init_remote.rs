use std::fs;
use tempfile::TempDir;

fn git(dir: &std::path::Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?}: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn seed_remote(remote_dir: &std::path::Path) {
    git(remote_dir, &["init", "--bare"]);
    let work = TempDir::new().unwrap();
    git(work.path(), &["init", "-b", "main"]);
    git(work.path(), &["config", "user.email", "test@test.com"]);
    git(work.path(), &["config", "user.name", "Test"]);
    fs::write(
        work.path().join("roost.toml"),
        r#"
remote = "REMOTE"
ignored = []

[profiles]
[profiles.desktop]
apps = ["nvim"]
app_sources = {}

[apps.nvim]
is_dir = true
on_profiles = ["desktop"]
ignore = []
"#,
    )
    .unwrap();
    fs::create_dir_all(work.path().join("desktop/nvim")).unwrap();
    fs::write(
        work.path().join("desktop/nvim/init.lua"),
        "vim.o.number = true\n",
    )
    .unwrap();
    fs::write(work.path().join(".gitignore"), "local.toml\n").unwrap();
    git(work.path(), &["add", "-A"]);
    git(work.path(), &["commit", "-m", "seed remote"]);
    git(
        work.path(),
        &["remote", "add", "origin", &remote_dir.to_string_lossy()],
    );
    git(work.path(), &["push", "-u", "origin", "main"]);
}

#[test]
fn hydrate_existing_remote_fetches_profile_files_before_tui_use() {
    let remote = TempDir::new().unwrap();
    seed_remote(remote.path());
    let roost = TempDir::new().unwrap();

    let result =
        roost::git::hydrate_existing_remote(roost.path(), &remote.path().to_string_lossy())
            .unwrap();

    assert!(matches!(result, roost::git::RemoteHydration::Hydrated));
    assert!(roost.path().join("roost.toml").exists());
    assert!(roost.path().join("desktop/nvim/init.lua").exists());
}

#[test]
fn hydrate_existing_remote_reports_empty_remote_without_creating_shared_config() {
    let remote = TempDir::new().unwrap();
    git(remote.path(), &["init", "--bare"]);
    let roost = TempDir::new().unwrap();

    let result =
        roost::git::hydrate_existing_remote(roost.path(), &remote.path().to_string_lossy())
            .unwrap();

    assert!(matches!(result, roost::git::RemoteHydration::EmptyRemote));
    assert!(roost.path().join(".git").exists());
    assert!(!roost.path().join("roost.toml").exists());
    assert!(!roost.path().join("local.toml").exists());
}
