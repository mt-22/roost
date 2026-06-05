use crate::app::{SharedAppConfig, load_shared, save_shared, shared_config_path};
use color_eyre::{Result, eyre::bail};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct CommitInfo {
    pub hash: String,
    pub message: String,
    pub timestamp: String,
}

pub enum ConflictPreference {
    Local,
    Remote,
}

pub enum SyncResult {
    Clean,
    ConfigConflict {
        resolved: Vec<String>,
    },
    FileConflict {
        config_conflicts: Vec<String>,
        file_conflicts: Vec<String>,
        backups: Vec<PathBuf>,
        preference: ConflictPreference,
    },
}

// subprocess runner: sets --git-dir and --work-tree from roost_dir
fn run_git(roost_dir: &Path, args: &[&str]) -> Result<String> {
    let git_dir = roost_dir.join(".git");
    let output = Command::new("git")
        .current_dir(roost_dir)
        .arg(format!("--git-dir={}", git_dir.display()))
        .arg(format!("--work-tree={}", roost_dir.display()))
        .args(args)
        .output()?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = if stderr.is_empty() {
            stdout.trim().to_string()
        } else if stdout.trim().is_empty() {
            stderr.trim().to_string()
        } else {
            format!("{}\n{}", stdout.trim(), stderr.trim())
        };
        bail!("git {} failed: {}", args.join(" "), detail);
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn init(roost_dir: &Path) -> Result<()> {
    run_git(roost_dir, &["init", "-b", "main"])?;

    // Ensure git identity is set so the first commit never fails.
    // Prefer local repo config; only set if missing globally/locally.
    if git_config_value(roost_dir, "user.name")?.is_none() {
        run_git(roost_dir, &["config", "user.name", "Roost"])?;
    }
    if git_config_value(roost_dir, "user.email")?.is_none() {
        run_git(roost_dir, &["config", "user.email", "roost@localhost"])?;
    }

    Ok(())
}

/// Read a git config value, returning `None` if unset (exit code 1) or empty.
fn git_config_value(roost_dir: &Path, key: &str) -> Result<Option<String>> {
    let git_dir = roost_dir.join(".git");
    let output = Command::new("git")
        .current_dir(roost_dir)
        .arg(format!("--git-dir={}", git_dir.display()))
        .arg(format!("--work-tree={}", roost_dir.display()))
        .args(["config", key])
        .output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if val.is_empty() {
        Ok(None)
    } else {
        Ok(Some(val))
    }
}

pub fn save(roost_dir: &Path, message: &str) -> Result<bool> {
    run_git(roost_dir, &["add", "-A"])?;
    match run_git(roost_dir, &["commit", "-m", message]) {
        Ok(_) => Ok(true),
        Err(e) if e.to_string().contains("nothing to commit") => Ok(false),
        Err(e) => Err(e),
    }
}

pub fn diff_stat(roost_dir: &Path) -> Result<String> {
    let output = run_git(roost_dir, &["diff", "--stat"])?;
    if output.trim().is_empty() {
        return Ok(String::new());
    }
    let summary = parse_git_stat(&output);
    Ok(summary)
}

fn parse_git_stat(output: &str) -> String {
    let lines: Vec<&str> = output.lines().filter(|l| !l.contains("files changed")).collect();
    let parts: Vec<String> = lines
        .iter()
        .filter_map(|line| {
            let file = line.split_whitespace().next()?;
            let additions = line.split('+').nth(1).and_then(|s| {
                s.split_whitespace().next()?.parse::<u32>().ok()
            });
            let deletions = if line.contains('-') {
                let parts: Vec<&str> = line.split('-').collect();
                parts.get(1).and_then(|s| s.split_whitespace().next()?.parse::<u32>().ok())
            } else {
                None
            };
            if let (Some(a), Some(d)) = (additions, deletions) {
                Some(format!("{}+{}/-{}", file, a, d))
            } else {
                None
            }
        })
        .take(5)
        .collect();
    if parts.is_empty() {
        output.lines().last().map(|s| s.to_string()).unwrap_or_default()
    } else {
        parts.join(", ")
    }
}

// back up a conflicting local file before overwriting with remote
fn backup_conflict_file(roost_dir: &Path, file_path: &str) -> Result<PathBuf> {
    let date = format_time_date();
    let clean_name = file_path.replace(['/', '\\'], "_");
    let backup_name = format!("{}_{}.local", date, clean_name);

    let backup_dir = roost_dir.join(BACKUP_DIR_NAME).join("sync");
    fs::create_dir_all(&backup_dir)?;

    let src = roost_dir.join(file_path);
    let dst = backup_dir.join(&backup_name);
    if src.is_dir() {
        copy_dir_recursive(&src, &dst)?;
    } else {
        fs::copy(&src, &dst)?;
    }

    Ok(dst)
}

fn format_time_date() -> String {
    time::OffsetDateTime::now_local()
        .map(|dt| format!("{:04}-{:02}-{:02}", dt.year(), dt.month() as u8, dt.day()))
        .unwrap_or_else(|_| "unknown-date".to_string())
}

const BACKUP_DIR_NAME: &str = ".backups";

// recursive directory copy
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

pub fn sync(roost_dir: &Path, preference: ConflictPreference) -> Result<SyncResult> {
    if get_remote(roost_dir)?.is_none() {
        bail!("no remote configured");
    }

    if is_dirty(roost_dir)? {
        save(roost_dir, "sync: auto-save pending edits")?;
    }

    run_git(roost_dir, &["fetch", "origin"])?;

    let local_head = run_git(roost_dir, &["rev-parse", "HEAD"])?;
    let remote_head = run_git(roost_dir, &["rev-parse", "origin/main"])?;
    if local_head == remote_head {
        return Ok(SyncResult::Clean);
    }

    // structural merge of roost.toml
    let local_config_path = shared_config_path(roost_dir);
    let local_config = load_shared(&local_config_path)?;

    let remote_toml = run_git(roost_dir, &["show", "origin/main:roost.toml"])?;
    let remote_config: SharedAppConfig = toml::from_str(&remote_toml)?;

    let mut merged = local_config.clone();
    let mut config_conflicts: Vec<String> = Vec::new();

    // merge apps: union by key, detect genuine conflicts
    for (name, remote_app) in &remote_config.apps {
        if let Some(local_app) = merged.apps.get(name) {
            if local_app.is_dir != remote_app.is_dir {
                let winner = match preference {
                    ConflictPreference::Local => local_app.clone(),
                    ConflictPreference::Remote => remote_app.clone(),
                };
                config_conflicts.push(format!(
                    "apps.{}.is_dir: local={}, remote={}, kept {}",
                    name,
                    local_app.is_dir,
                    remote_app.is_dir,
                    match preference {
                        ConflictPreference::Local => "local",
                        ConflictPreference::Remote => "remote",
                    }
                ));
                merged.apps.insert(name.clone(), winner);
            }
        } else {
            merged.apps.insert(name.clone(), remote_app.clone());
        }
    }

    // merge profiles: union by key, union app sets (additive)
    for (name, remote_profile) in &remote_config.profiles {
        if let Some(local_profile) = merged.profiles.get_mut(name) {
            for app in &remote_profile.apps {
                local_profile.apps.insert(app.clone());
            }
            for (app_name, source) in &remote_profile.app_sources {
                local_profile
                    .app_sources
                    .insert(app_name.clone(), source.clone());
            }
        } else {
            merged.profiles.insert(name.clone(), remote_profile.clone());
        }
    }

    // merge ignored: always additive
    for pattern in &remote_config.ignored {
        merged.ignored.insert(pattern.clone());
    }

    // write merged config and commit
    save_shared(&local_config_path, &merged)?;
    crate::gitignore::regenerate(roost_dir, &merged.ignored, &merged.apps)?;
    run_git(roost_dir, &["add", "-A"])?;
    match run_git(
        roost_dir,
        &["commit", "-m", "merge: resolve sync conflicts"],
    ) {
        Ok(_) => {}
        Err(e) if e.to_string().contains("nothing to commit") => {}
        Err(e) => return Err(e),
    }

    // if remote preference, back up conflicting local files before rebase
    let mut backups: Vec<PathBuf> = Vec::new();
    if matches!(preference, ConflictPreference::Remote) {
        // attempt rebase to discover which files conflict
        if let Err(rebase_err) = run_git(roost_dir, &["rebase", "origin/main"]) {
            let conflict_files = get_conflict_files(roost_dir);
            for file in &conflict_files {
                if let Ok(backup) = backup_conflict_file(roost_dir, file) {
                    backups.push(backup);
                }
            }
            // check out remote version of conflicting files, then continue rebase
            for file in &conflict_files {
                let _ = run_git(roost_dir, &["checkout", "--theirs", file]);
                let _ = run_git(roost_dir, &["add", file]);
            }
            if let Err(e) = run_git(roost_dir, &["rebase", "--continue"]) {
                if is_rebasing(roost_dir) {
                    let _ = run_git(roost_dir, &["rebase", "--abort"]);
                }
                return Err(color_eyre::eyre::eyre!(
                    "rebase failed after resolving conflicts: {}. Manual resolution may be required.",
                    e
                ));
            }
        }
    } else {
        // local preference: attempt rebase, capture conflicts, abort
        match run_git(roost_dir, &["rebase", "origin/main"]) {
            Ok(_) => {}
            Err(rebase_err) => {
                let conflict_files = get_conflict_files(roost_dir);
                let _ = run_git(roost_dir, &["rebase", "--abort"]);

                if conflict_files.is_empty() {
                    return Err(color_eyre::eyre::eyre!(
                        "rebase failed and no conflict files detected: {}. Check git status.",
                        rebase_err
                    ));
                }

                return Ok(SyncResult::FileConflict {
                    config_conflicts,
                    file_conflicts: conflict_files,
                    backups,
                    preference,
                });
            }
        }
    }

    // push merged/rebased state to remote
    run_git(roost_dir, &["push", "origin", "main"])?;

    // rebase succeeded (or was resolved)
    if config_conflicts.is_empty() && backups.is_empty() {
        Ok(SyncResult::Clean)
    } else if !config_conflicts.is_empty() && backups.is_empty() {
        Ok(SyncResult::ConfigConflict {
            resolved: config_conflicts,
        })
    } else {
        Ok(SyncResult::FileConflict {
            config_conflicts,
            file_conflicts: Vec::new(),
            backups,
            preference,
        })
    }
}

// get list of conflicting files during a rebase
fn get_conflict_files(roost_dir: &Path) -> Vec<String> {
    match run_git(roost_dir, &["diff", "--name-only", "--diff-filter=U"]) {
        Ok(output) => output.lines().map(|s| s.to_string()).collect(),
        Err(_) => Vec::new(),
    }
}

fn is_rebasing(roost_dir: &Path) -> bool {
    roost_dir.join(".git/rebase-apply").exists() || roost_dir.join(".git/rebase-merge").exists()
}

pub fn log(roost_dir: &Path, n: usize) -> Result<Vec<CommitInfo>> {
    let output = run_git(
        roost_dir,
        &[
            "log",
            "-n",
            &n.to_string(),
            "--pretty=format:%H%x00%s%x00%ci%x01",
        ],
    )?;

    if output.is_empty() {
        return Ok(Vec::new());
    }

    let commits = output
        .split('\x01')
        .filter(|s| !s.is_empty())
        .map(|record| {
            let parts: Vec<&str> = record.split('\0').collect();
            CommitInfo {
                hash: parts[0].to_string(),
                message: parts[1].to_string(),
                timestamp: parts[2].to_string(),
            }
        })
        .collect();

    Ok(commits)
}

pub fn diff(roost_dir: &Path) -> Result<String> {
    let output = run_git(roost_dir, &["diff", "HEAD"])?;
    Ok(output)
}

pub fn undo(roost_dir: &Path, n: usize) -> Result<()> {
    let target = format!("HEAD~{}", n);
    run_git(roost_dir, &["reset", "--hard", &target])?;
    Ok(())
}

pub fn rollback(roost_dir: &Path, hash: &str) -> Result<()> {
    run_git(roost_dir, &["reset", "--hard", hash])?;
    Ok(())
}

pub fn set_remote(roost_dir: &Path, url: &str) -> Result<()> {
    run_git(roost_dir, &["remote", "add", "origin", url])?;
    Ok(())
}

pub fn get_remote(roost_dir: &Path) -> Result<Option<String>> {
    match run_git(roost_dir, &["remote", "get-url", "origin"]) {
        Ok(url) => Ok(Some(url)),
        Err(e) if e.to_string().contains("No such remote") => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn is_dirty(roost_dir: &Path) -> Result<bool> {
    let output = run_git(roost_dir, &["status", "--porcelain"])?;
    Ok(!output.is_empty())
}

#[cfg(test)]
mod tests;
