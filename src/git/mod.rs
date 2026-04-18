use color_eyre::eyre::eyre;
use std::path::Path;
use std::process::Command;

/// Run a git command in the given directory.
pub fn git(dir: &Path, args: &[&str]) -> color_eyre::Result<()> {
    let output = Command::new("git").args(args).current_dir(dir).output()?;
    if output.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        Err(eyre!(
            "git {} failed: {}",
            args.first().copied().unwrap_or("git"),
            err
        ))
    }
}

/// Run a git command and return its stdout.
pub fn git_output(dir: &Path, args: &[&str]) -> color_eyre::Result<String> {
    let output = Command::new("git").args(args).current_dir(dir).output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        Err(eyre!(
            "git {} failed: {}",
            args.first().copied().unwrap_or("git"),
            err
        ))
    }
}

/// Check if a directory is a git repository.
pub fn is_git_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// Stage all changes and commit if there is anything to commit.
/// Returns Ok(true) if a commit was made, Ok(false) if nothing to commit.
pub fn auto_commit(roost_dir: &Path, message: &str) -> color_eyre::Result<bool> {
    if !is_git_repo(roost_dir) {
        return Ok(false);
    }

    git(roost_dir, &["add", "-A"])?;

    let status = git_output(roost_dir, &["status", "--porcelain"])?;
    if status.trim().is_empty() {
        return Ok(false);
    }

    git(roost_dir, &["commit", "-m", message])?;
    Ok(true)
}

/// Check if there are uncommitted changes in the working tree.
pub fn is_dirty(roost_dir: &Path) -> color_eyre::Result<bool> {
    if !is_git_repo(roost_dir) {
        return Ok(false);
    }
    let status = git_output(roost_dir, &["status", "--porcelain"])?;
    Ok(!status.trim().is_empty())
}

/// Return the unified diff of all uncommitted changes (staged + unstaged).
pub fn diff_text(roost_dir: &Path) -> color_eyre::Result<String> {
    if !is_git_repo(roost_dir) {
        return Err(eyre!("Not a git repo."));
    }
    git_output(roost_dir, &["diff", "HEAD"])
}

/// Return the diff introduced by a specific commit.
pub fn diff_for_commit(roost_dir: &Path, hash: &str) -> color_eyre::Result<String> {
    git_output(roost_dir, &["show", "--stat", "--patch", hash])
}

/// A single log entry parsed from `git log`.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub hash: String,
    pub short_hash: String,
    pub date: String,
    pub message: String,
}

/// Return recent log entries.
pub fn log(roost_dir: &Path, count: usize) -> color_eyre::Result<Vec<LogEntry>> {
    if !is_git_repo(roost_dir) {
        return Err(eyre!("Not a git repo."));
    }
    let format = "%H%x00%h%x00%cr%x00%s";
    let raw = git_output(
        roost_dir,
        &[
            "log",
            &format!("-{}", count),
            &format!("--format={}", format),
        ],
    )?;

    let entries = raw
        .lines()
        .filter(|l| !l.is_empty())
        .map(|line| {
            let parts: Vec<&str> = line.split('\0').collect();
            let hash = parts.first().unwrap_or(&"").to_string();
            let short_hash = parts.get(1).unwrap_or(&"").to_string();
            let date = parts.get(2).unwrap_or(&"").to_string();
            let message = parts.get(3).unwrap_or(&"").to_string();
            LogEntry {
                hash,
                short_hash,
                date,
                message,
            }
        })
        .collect();

    Ok(entries)
}

/// Hard-reset HEAD by `n` commits (destructive — working tree is changed).
pub fn undo(roost_dir: &Path, n: usize) -> color_eyre::Result<()> {
    if !is_git_repo(roost_dir) {
        return Err(eyre!("Not a git repo."));
    }
    git(roost_dir, &["reset", "--hard", &format!("HEAD~{}", n)])
}

/// Hard-reset to a specific commit (destructive — working tree is changed).
pub fn rollback(roost_dir: &Path, hash: &str) -> color_eyre::Result<()> {
    if !is_git_repo(roost_dir) {
        return Err(eyre!("Not a git repo."));
    }
    git(roost_dir, &["reset", "--hard", hash])
}

fn current_branch(roost_dir: &Path) -> color_eyre::Result<String> {
    let branch = git_output(roost_dir, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    Ok(branch.trim().to_string())
}

fn remote_name(roost_dir: &Path) -> color_eyre::Result<String> {
    let remotes = git_output(roost_dir, &["remote"])?;
    remotes
        .lines()
        .next()
        .map(|s| s.trim().to_string())
        .ok_or_else(|| eyre!("No remote configured."))
}

/// Sync the roost directory with the remote.
/// Assumes auto_commit has already been called — only pulls and pushes.
pub fn sync(roost_dir: &Path) -> color_eyre::Result<()> {
    if !is_git_repo(roost_dir) {
        return Err(eyre!(
            "No git repo in {}. Set up a remote with `roost init` first.",
            roost_dir.display()
        ));
    }

    let remotes = git_output(roost_dir, &["remote"])?;
    if remotes.trim().is_empty() {
        return Err(eyre!("No remote configured. Set one up with `roost init`."));
    }

    let branch = current_branch(roost_dir).unwrap_or_else(|_| "main".to_string());
    let remote = remote_name(roost_dir).unwrap_or_else(|_| "origin".to_string());

    // Catch any stragglers that weren't auto-committed
    auto_commit(roost_dir, "pre-sync: pending changes")?;

    // Pull with rebase
    println!("Pulling from remote...");
    match git(roost_dir, &["pull", "--rebase", &remote, &branch]) {
        Ok(_) => {}
        Err(e) => {
            println!("Warning: pull failed ({}). Continuing with push...", e);
        }
    }

    // Push
    println!("Pushing to remote...");
    git(roost_dir, &["push", &remote, &branch])?;

    println!("Sync complete!");
    Ok(())
}

#[cfg(test)]
mod tests;
