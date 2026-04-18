use super::*;
use std::fs;
use tempfile::TempDir;

const GIT_CONFIG: &[&str] = &["-c", "user.name=test", "-c", "user.email=test@test.com"];

fn init_test_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let path = dir.path();
    Command::new("git")
        .args(GIT_CONFIG)
        .args(["init"])
        .current_dir(path)
        .output()
        .unwrap();
    Command::new("git")
        .args(GIT_CONFIG)
        .args(["commit", "--allow-empty", "-m", "initial"])
        .current_dir(path)
        .output()
        .unwrap();
    dir
}

fn git_cmd(dir: &Path, args: &[&str]) {
    let mut full_args: Vec<&str> = GIT_CONFIG.to_vec();
    full_args.extend_from_slice(args);
    let output = Command::new("git")
        .args(&full_args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {:?} failed", args);
}

#[test]
fn is_git_repo_true_after_init() {
    let dir = init_test_repo();
    assert!(is_git_repo(dir.path()));
}

#[test]
fn is_git_repo_false_for_plain_dir() {
    let dir = TempDir::new().unwrap();
    assert!(!is_git_repo(dir.path()));
}

#[test]
fn git_output_success() {
    let dir = init_test_repo();
    let out = git_output(dir.path(), &["rev-parse", "--git-dir"]).unwrap();
    assert_eq!(out.trim(), ".git");
}

#[test]
fn git_output_error_on_invalid() {
    let dir = init_test_repo();
    let result = git_output(dir.path(), &["checkout", "--nonexistent-branch"]);
    assert!(result.is_err());
}

#[test]
fn auto_commit_returns_true_when_dirty() {
    let dir = init_test_repo();
    fs::write(dir.path().join("file.txt"), "hello").unwrap();
    let committed = auto_commit(dir.path(), "add file").unwrap();
    assert!(committed);
}

#[test]
fn auto_commit_returns_false_when_clean() {
    let dir = init_test_repo();
    let committed = auto_commit(dir.path(), "nothing").unwrap();
    assert!(!committed);
}

#[test]
fn auto_commit_returns_false_for_non_repo() {
    let dir = TempDir::new().unwrap();
    let committed = auto_commit(dir.path(), "test").unwrap();
    assert!(!committed);
}

#[test]
fn is_dirty_true_after_writing_file() {
    let dir = init_test_repo();
    fs::write(dir.path().join("dirty.txt"), "changes").unwrap();
    assert!(is_dirty(dir.path()).unwrap());
}

#[test]
fn is_dirty_false_in_clean_repo() {
    let dir = init_test_repo();
    assert!(!is_dirty(dir.path()).unwrap());
}

#[test]
fn diff_text_returns_diff_when_dirty() {
    let dir = init_test_repo();
    fs::write(dir.path().join("new.txt"), "content").unwrap();
    git_cmd(dir.path(), &["add", "-A"]);
    let diff = diff_text(dir.path()).unwrap();
    assert!(diff.contains("new.txt"));
}

#[test]
fn diff_text_error_for_non_repo() {
    let dir = TempDir::new().unwrap();
    assert!(diff_text(dir.path()).is_err());
}

#[test]
fn log_returns_entries_after_commits() {
    let dir = init_test_repo();
    fs::write(dir.path().join("a.txt"), "a").unwrap();
    git_cmd(dir.path(), &["add", "-A"]);
    git_cmd(dir.path(), &["commit", "-m", "second"]);
    let entries = log(dir.path(), 10).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].message, "second");
    assert_eq!(entries[1].message, "initial");
}

#[test]
fn log_error_for_non_repo() {
    let dir = TempDir::new().unwrap();
    assert!(log(dir.path(), 5).is_err());
}

#[test]
fn log_respects_count_limit() {
    let dir = init_test_repo();
    for i in 0..5 {
        fs::write(dir.path().join(format!("f{}.txt", i)), "x").unwrap();
        git_cmd(dir.path(), &["add", "-A"]);
        git_cmd(dir.path(), &["commit", "-m", &format!("commit {}", i)]);
    }
    let entries = log(dir.path(), 3).unwrap();
    assert_eq!(entries.len(), 3);
}

#[test]
fn diff_for_commit_returns_diff() {
    let dir = init_test_repo();
    fs::write(dir.path().join("c.txt"), "c").unwrap();
    git_cmd(dir.path(), &["add", "-A"]);
    git_cmd(dir.path(), &["commit", "-m", "add c"]);
    let entries = log(dir.path(), 1).unwrap();
    let hash = &entries[0].hash;
    let diff = diff_for_commit(dir.path(), hash).unwrap();
    assert!(diff.contains("c.txt"));
}

#[test]
fn undo_removes_last_commit() {
    let dir = init_test_repo();
    fs::write(dir.path().join("u.txt"), "u").unwrap();
    git_cmd(dir.path(), &["add", "-A"]);
    git_cmd(dir.path(), &["commit", "-m", "to undo"]);
    let count_before = log(dir.path(), 10).unwrap().len();
    undo(dir.path(), 1).unwrap();
    let count_after = log(dir.path(), 10).unwrap().len();
    assert_eq!(count_after, count_before - 1);
}

#[test]
fn undo_error_for_non_repo() {
    let dir = TempDir::new().unwrap();
    assert!(undo(dir.path(), 1).is_err());
}

#[test]
fn rollback_restores_file_content() {
    let dir = init_test_repo();
    fs::write(dir.path().join("r.txt"), "v1").unwrap();
    git_cmd(dir.path(), &["add", "-A"]);
    git_cmd(dir.path(), &["commit", "-m", "v1"]);
    let entries = log(dir.path(), 1).unwrap();
    let hash = entries[0].hash.clone();
    fs::write(dir.path().join("r.txt"), "v2").unwrap();
    git_cmd(dir.path(), &["add", "-A"]);
    git_cmd(dir.path(), &["commit", "-m", "v2"]);
    rollback(dir.path(), &hash).unwrap();
    assert_eq!(fs::read_to_string(dir.path().join("r.txt")).unwrap(), "v1");
}

#[test]
fn rollback_error_for_non_repo() {
    let dir = TempDir::new().unwrap();
    assert!(rollback(dir.path(), "abc123").is_err());
}

#[test]
fn log_entry_fields_populated() {
    let dir = init_test_repo();
    fs::write(dir.path().join("fields.txt"), "data").unwrap();
    git_cmd(dir.path(), &["add", "-A"]);
    git_cmd(dir.path(), &["commit", "-m", "check fields"]);

    let entries = log(dir.path(), 10).unwrap();
    let entry = &entries[0];

    assert!(!entry.hash.is_empty());
    assert!(!entry.short_hash.is_empty());
    assert!(entry.hash.starts_with(&entry.short_hash));
    assert!(!entry.date.is_empty());
    assert_eq!(entry.message, "check fields");
}

#[test]
fn test_auto_commit_with_staged_and_unstaged() {
    let dir = init_test_repo();
    fs::write(dir.path().join("staged.txt"), "staged").unwrap();
    git_cmd(dir.path(), &["add", "staged.txt"]);
    fs::write(dir.path().join("unstaged.txt"), "unstaged").unwrap();

    let committed = auto_commit(dir.path(), "mixed state").unwrap();
    assert!(committed);

    let entries = log(dir.path(), 1).unwrap();
    assert_eq!(entries[0].message, "mixed state");
}

#[test]
fn test_undo_multiple_preserves_initial() {
    let dir = init_test_repo();

    for i in 0..4 {
        fs::write(dir.path().join(format!("f{}.txt", i)), "data").unwrap();
        git_cmd(dir.path(), &["add", "-A"]);
        git_cmd(dir.path(), &["commit", "-m", &format!("commit {}", i)]);
    }

    undo(dir.path(), 3).unwrap();
    let entries = log(dir.path(), 10).unwrap();
    assert_eq!(entries.len(), 2);
}

#[test]
fn test_undo_zero_is_nop() {
    let dir = init_test_repo();
    fs::write(dir.path().join("keep.txt"), "data").unwrap();
    git_cmd(dir.path(), &["add", "-A"]);
    git_cmd(dir.path(), &["commit", "-m", "keep"]);

    let count_before = log(dir.path(), 10).unwrap().len();
    undo(dir.path(), 0).unwrap();
    let count_after = log(dir.path(), 10).unwrap().len();
    assert_eq!(count_before, count_after);
}

#[test]
fn test_rollback_to_current_hash_is_nop() {
    let dir = init_test_repo();
    fs::write(dir.path().join("keep.txt"), "data").unwrap();
    git_cmd(dir.path(), &["add", "-A"]);
    git_cmd(dir.path(), &["commit", "-m", "keep"]);

    let entries = log(dir.path(), 1).unwrap();
    let current_hash = entries[0].hash.clone();

    rollback(dir.path(), &current_hash).unwrap();
    assert!(dir.path().join("keep.txt").exists());
}

#[test]
fn test_diff_text_with_staged_changes() {
    let dir = init_test_repo();
    fs::write(dir.path().join("new.txt"), "new file").unwrap();
    git_cmd(dir.path(), &["add", "new.txt"]);

    let diff = diff_text(dir.path()).unwrap();
    assert!(!diff.is_empty());
}

#[test]
fn test_is_dirty_after_commit() {
    let dir = init_test_repo();
    assert!(!is_dirty(dir.path()).unwrap());

    fs::write(dir.path().join("new.txt"), "content").unwrap();
    assert!(is_dirty(dir.path()).unwrap());

    git_cmd(dir.path(), &["add", "-A"]);
    assert!(is_dirty(dir.path()).unwrap());
}

#[test]
fn test_log_entry_ordering() {
    let dir = init_test_repo();

    for i in 0..3 {
        fs::write(dir.path().join(format!("f{}.txt", i)), "").unwrap();
        git_cmd(dir.path(), &["add", "-A"]);
        git_cmd(dir.path(), &["commit", "-m", &format!("commit {}", i)]);
    }

    let entries = log(dir.path(), 10).unwrap();
    assert_eq!(entries.len(), 4);
}
