use super::*;
use std::fs;
use tempfile::TempDir;

// helper: create a temp dir and initialize a git repo
fn setup_git_repo() -> TempDir {
    let tmp = TempDir::new().unwrap();
    init(tmp.path()).unwrap();
    // git requires user identity for commits
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(tmp.path())
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(tmp.path())
        .output();
    tmp
}

// helper: write a file and commit it
fn commit_file(repo: &Path, name: &str, content: &str, message: &str) {
    fs::write(repo.join(name), content).unwrap();
    save(repo, message).unwrap();
}

#[test]
fn init_creates_git_dir() {
    let tmp = TempDir::new().unwrap();
    init(tmp.path()).unwrap();
    assert!(tmp.path().join(".git").exists());
}

#[test]
fn init_uses_main_branch() {
    let tmp = setup_git_repo();
    // need at least one commit for branch name to exist
    commit_file(tmp.path(), "a.txt", "a", "initial");
    let branch = run_git(tmp.path(), &["branch", "--show-current"]).unwrap();
    assert_eq!(branch, "main");
}

#[test]
fn save_commits_changes() {
    let tmp = setup_git_repo();

    let committed = save(tmp.path(), "initial commit").unwrap();
    assert!(!committed); // nothing to commit yet

    fs::write(tmp.path().join("test.txt"), "hello").unwrap();
    let committed = save(tmp.path(), "add test.txt").unwrap();
    assert!(committed);

    assert!(!is_dirty(tmp.path()).unwrap());
    let commits = log(tmp.path(), 1).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].message, "add test.txt");
}

#[test]
fn save_returns_false_when_clean() {
    let tmp = setup_git_repo();
    let result = save(tmp.path(), "nothing here").unwrap();
    assert!(!result);
}

#[test]
fn log_returns_commits_in_order() {
    let tmp = setup_git_repo();

    commit_file(tmp.path(), "a.txt", "a", "first");
    commit_file(tmp.path(), "b.txt", "b", "second");
    commit_file(tmp.path(), "c.txt", "c", "third");

    let commits = log(tmp.path(), 3).unwrap();
    assert_eq!(commits.len(), 3);
    assert_eq!(commits[0].message, "third");
    assert_eq!(commits[1].message, "second");
    assert_eq!(commits[2].message, "first");
}

#[test]
fn diff_returns_uncommitted_changes() {
    let tmp = setup_git_repo();
    commit_file(tmp.path(), "a.txt", "a", "initial");

    // no changes → empty diff
    let d = diff(tmp.path()).unwrap();
    assert!(d.is_empty());

    // modify file → non-empty diff
    fs::write(tmp.path().join("a.txt"), "modified").unwrap();
    let d = diff(tmp.path()).unwrap();
    assert!(d.contains("modified"));
}

#[test]
fn undo_resets_head() {
    let tmp = setup_git_repo();

    commit_file(tmp.path(), "a.txt", "a", "first");
    commit_file(tmp.path(), "b.txt", "b", "second");
    commit_file(tmp.path(), "c.txt", "c", "third");

    undo(tmp.path(), 1).unwrap();

    let commits = log(tmp.path(), 10).unwrap();
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].message, "second");

    // file c should be gone (hard reset)
    assert!(!tmp.path().join("c.txt").exists());
}

#[test]
fn rollback_resets_to_hash() {
    let tmp = setup_git_repo();

    commit_file(tmp.path(), "a.txt", "a", "first");
    let first_hash = log(tmp.path(), 1).unwrap()[0].hash.clone();
    commit_file(tmp.path(), "b.txt", "b", "second");
    commit_file(tmp.path(), "c.txt", "c", "third");

    rollback(tmp.path(), &first_hash).unwrap();

    let commits = log(tmp.path(), 10).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].hash, first_hash);
    assert!(!tmp.path().join("b.txt").exists());
    assert!(!tmp.path().join("c.txt").exists());
}

#[test]
fn is_dirty_detects_changes() {
    let tmp = setup_git_repo();

    assert!(!is_dirty(tmp.path()).unwrap());

    fs::write(tmp.path().join("new.txt"), "hello").unwrap();
    assert!(is_dirty(tmp.path()).unwrap());

    save(tmp.path(), "add new.txt").unwrap();
    assert!(!is_dirty(tmp.path()).unwrap());
}

#[test]
fn read_shared_at_reads_config_from_commit() {
    let tmp = setup_git_repo();

    let config1 = r#"
[apps.zsh]
is_dir = true
on_profiles = ["default"]

[profiles.default]
apps = ["zsh"]
"#;
    fs::write(tmp.path().join("roost.toml"), config1).unwrap();
    save(tmp.path(), "add zsh").unwrap();

    let hash1 = log(tmp.path(), 1).unwrap()[0].hash.clone();

    let config2 = r#"
[apps.zsh]
is_dir = true
on_profiles = ["default"]

[apps.nvim]
is_dir = true
on_profiles = ["default"]

[profiles.default]
apps = ["zsh", "nvim"]
"#;
    fs::write(tmp.path().join("roost.toml"), config2).unwrap();
    save(tmp.path(), "add nvim").unwrap();

    let config = super::read_shared_at(tmp.path(), &hash1).unwrap();
    assert!(config.apps.contains_key("zsh"));
    assert!(!config.apps.contains_key("nvim"), "nvim should not exist at hash1");
    assert_eq!(config.profiles.get("default").unwrap().apps.len(), 1);
}

#[test]
fn set_and_get_remote() {
    let tmp = setup_git_repo();

    assert!(get_remote(tmp.path()).unwrap().is_none());

    set_remote(tmp.path(), "https://github.com/user/dotfiles.git").unwrap();
    let url = get_remote(tmp.path()).unwrap();
    assert_eq!(
        url,
        Some("https://github.com/user/dotfiles.git".to_string())
    );
}
