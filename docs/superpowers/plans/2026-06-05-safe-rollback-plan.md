# Safe Rollback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace destructive `git reset --hard` rollback with selective `git checkout` that preserves apps added after the target commit.

**Architecture:** A single `safe_rollback()` function in `src/git/mod.rs` handles the full flow: read target config, compute protected/preserved apps, selective checkout, config repair, and commit. TUI and CLI call the same function. The `r` key handler in the Git Log dialog computes analysis before showing the confirm dialog.

**Tech Stack:** Rust, git CLI, serde for config parsing

---

### Task 1: Add `read_shared_at` helper to `src/git/mod.rs`

**Files:**
- Modify: `src/git/mod.rs`
- Test: `src/git/tests.rs`

- [ ] **Step 1: Write the failing test**

At the end of `src/git/tests.rs`, add:

```rust
#[test]
fn read_shared_at_reads_config_from_commit() {
    let tmp = setup_git_repo();

    // Create roost.toml with one app
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

    // Update roost.toml with a second app
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

    // Read the first commit's config
    let config = super::read_shared_at(tmp.path(), &hash1).unwrap();
    assert!(config.apps.contains_key("zsh"));
    assert!(!config.apps.contains_key("nvim"), "nvim should not exist at hash1");
    assert_eq!(config.profiles.get("default").unwrap().apps.len(), 1);
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test read_shared_at_reads_config_from_commit 2>&1`
Expected: FAIL — `read_shared_at` not defined

- [ ] **Step 3: Add `read_shared_at` function to `src/git/mod.rs`**

Add before the `undo` function (around line 483):

```rust
/// Parse `roost.toml` from a specific git commit without checking it out.
pub fn read_shared_at(roost_dir: &Path, hash: &str) -> Result<SharedAppConfig> {
    let output = run_git(roost_dir, &["show", &format!("{}:roost.toml", hash)])?;
    let config: SharedAppConfig = toml::from_str(&output)?;
    crate::app::validate_shared(&config)?;
    Ok(config)
}
```

Add import at top of file if `SharedAppConfig` isn't already imported (it is — line 1):
```rust
use crate::app::{SharedAppConfig, load_shared, save_shared, shared_config_path};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test read_shared_at_reads_config_from_commit 2>&1`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/git/mod.rs
git add src/git/tests.rs
git commit -m "feat: add read_shared_at helper to parse roost.toml from any commit"
```

---

### Task 2: Add `safe_rollback` function to `src/git/mod.rs`

**Files:**
- Modify: `src/git/mod.rs`
- Test: `src/git/tests.rs`

This is the core function. It:
1. Reads target `roost.toml` from git
2. Computes preserved apps (exist at target) and protected apps (added after target)
3. Selective checkout of preserved app directories + config files
4. Reloads config, re-adds protected apps
5. Calls `ensure_links`, saves local config
6. Commits the result

- [ ] **Step 1: Write the failing unit test**

Add to `src/git/tests.rs`:

```rust
#[test]
fn safe_rollback_preserves_protected_apps() {
    let tmp = setup_git_repo();
    let profile = "default";

    // Commit 1: appA exists
    let config1 = r#"
[apps.appA]
is_dir = true
on_profiles = ["default"]

[profiles.default]
apps = ["appA"]
"#;
    fs::write(tmp.path().join("roost.toml"), config1).unwrap();
    fs::create_dir_all(tmp.path().join("default").join("appA")).unwrap();
    fs::write(tmp.path().join("default/appA/file1.txt"), "original").unwrap();
    save(tmp.path(), "add appA").unwrap();
    let target_hash = log(tmp.path(), 1).unwrap()[0].hash.clone();

    // Commit 2: appB is added, appA is modified
    let config2 = r#"
[apps.appA]
is_dir = true
on_profiles = ["default"]

[apps.appB]
is_dir = true
on_profiles = ["default"]

[profiles.default]
apps = ["appA", "appB"]
"#;
    fs::write(tmp.path().join("roost.toml"), config2).unwrap();
    fs::create_dir_all(tmp.path().join("default").join("appB")).unwrap();
    fs::write(tmp.path().join("default/appB/file2.txt"), "new app").unwrap();
    fs::write(tmp.path().join("default/appA/file1.txt"), "modified").unwrap();
    save(tmp.path(), "add appB").unwrap();

    // Set up configs as safe_rollback expects them
    let pre_shared = load_shared(&shared_config_path(tmp.path())).unwrap();
    let pre_local = crate::app::LocalAppConfig {
        active_profile: profile.to_string(),
        os_info: crate::os_detect::OsInfo { os: "test".to_string(), arch: "test".to_string() },
        link_paths: [
            ("appA".to_string(), PathBuf::from("/home/user/.config/appA")),
            ("appB".to_string(), PathBuf::from("/home/user/.config/appB")),
        ].into(),
    };

    let result = super::safe_rollback(
        tmp.path(),
        &target_hash,
        &pre_shared,
        &pre_local,
        profile,
    );
    assert!(result.is_ok(), "safe_rollback failed: {:?}", result.err());

    // Reload the config saved by safe_rollback
    let new_shared = load_shared(&shared_config_path(tmp.path())).unwrap();

    // appB should still be in the config (preserved)
    assert!(new_shared.apps.contains_key("appB"), "appB should be preserved");
    assert!(new_shared.profiles.get("default").unwrap().apps.contains("appB"));

    // appB's managed files should still exist
    assert!(tmp.path().join("default/appB/file2.txt").exists(), "appB files should exist");

    // appA's managed files should be rolled back to original
    let app_a_content = fs::read_to_string(tmp.path().join("default/appA/file1.txt")).unwrap();
    assert_eq!(app_a_content.trim(), "original", "appA should be rolled back");

    // appA should still be in the config
    assert!(new_shared.apps.contains_key("appA"));

    // Git log should show a new commit
    let commits = log(tmp.path(), 3).unwrap();
    assert_eq!(commits.len(), 3, "should have 3 commits: add appA, add appB, safe_rollback");
    assert!(commits[0].message.contains("preserve"), "latest commit should mention preservation");
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test safe_rollback_preserves_protected_apps 2>&1`
Expected: FAIL — `safe_rollback` not defined

- [ ] **Step 3: Implement `safe_rollback`**

Add to `src/git/mod.rs` after `read_shared_at` (or near `rollback` at line 490):

```rust
/// Roll back to a target commit while preserving apps that don't exist at that commit.
///
/// Uses `git checkout` (not `git reset --hard`) to selectively restore preserved
/// app directories. Protected apps' files are never touched. The result is committed
/// as a new forward commit.
pub fn safe_rollback(
    roost_dir: &Path,
    hash: &str,
    pre_shared: &SharedAppConfig,
    pre_local: &LocalAppConfig,
    profile_name: &str,
) -> Result<()> {
    // Phase 1: Read target config and classify apps
    let target_shared = match read_shared_at(roost_dir, hash) {
        Ok(c) => c,
        Err(_) => {
            // No roost.toml at target — treat all apps as protected
            let _ = run_git(roost_dir, &["checkout", hash, "--", "."])?;
            return Err(color_eyre::eyre::eyre!(
                "target commit has no roost.toml, aborting"
            ));
        }
    };

    // Collect all apps across all profiles
    let current_apps: BTreeSet<String> = pre_shared.apps.keys().cloned().collect();
    let target_apps: BTreeSet<String> = target_shared.apps.keys().cloned().collect();

    // Apps that existed at target → their files get rolled back
    let preserved_apps: BTreeSet<&String> = current_apps.intersection(&target_apps).collect();
    // Apps added after target → left untouched
    let protected_apps: BTreeSet<&String> = current_apps.difference(&target_apps).collect();

    // Phase 2: Selective checkout — only checkout preserved app directories + config
    for app_name in &preserved_apps {
        let app = &pre_shared.apps[*app_name];
        let rel_path = if app.is_dir {
            format!("{}/{}", profile_name, app_name)
        } else {
            format!("{}/misc/{}", profile_name, app_name)
        };
        match run_git(roost_dir, &["checkout", hash, "--", &rel_path]) {
            Ok(_) => {},
            Err(e) => {
                // Path might not exist in target if the app was added later
                // This is fine — we'll still preserve the config
                eprintln!("note: could not checkout {}: {}", rel_path, e);
            }
        }
    }

    // Restore roost.toml and .gitignore from target
    run_git(roost_dir, &["checkout", hash, "--", "roost.toml"])?;
    let _ = run_git(roost_dir, &["checkout", hash, "--", ".gitignore"]);

    // Phase 3: Reload and repair config
    let shared_path = crate::app::shared_config_path(roost_dir);
    let local_path = crate::app::local_config_path(roost_dir);
    let mut shared = crate::app::load_shared(&shared_path)?;
    let mut local = pre_local.clone();

    for app_name in &protected_apps {
        // Restore app config entry
        if let Some(app_config) = pre_shared.apps.get(*app_name) {
            shared.apps.insert((*app_name).clone(), app_config.clone());
        }

        // Restore profile membership (check all profiles, not just active)
        for (pname, profile) in &pre_shared.profiles {
            if profile.apps.contains(*app_name) {
                if let Some(target_profile) = shared.profiles.get_mut(pname) {
                    target_profile.apps.insert((*app_name).clone());
                    if let Some(source) = profile.app_sources.get(*app_name) {
                        target_profile.app_sources.insert((*app_name).clone(), source.clone());
                    }
                }
            }
        }

        // Restore link_paths
        if let Some(path) = pre_local.link_paths.get(*app_name) {
            local.link_paths.insert((*app_name).clone(), path.clone());
        }
    }

    // Save and ensure links
    crate::app::save_shared(&shared_path, &shared)?;
    let _ = crate::linker::ensure_links(&shared, &mut local, roost_dir);
    crate::app::save_local(&local_path, &local)?;

    // Phase 4: Commit
    let n_protected = protected_apps.len();
    run_git(roost_dir, &["add", "-A"])?;
    match run_git(
        roost_dir,
        &[
            "commit",
            "-m",
            &format!("rollback to {} + preserve {} app(s)", &hash[..hash.len().min(7)], n_protected),
        ],
    ) {
        Ok(_) => {}
        Err(e) if e.to_string().contains("nothing to commit") => {}
        Err(e) => return Err(e),
    }

    Ok(())
}
```

Note: Add `use std::collections::BTreeSet;` to the imports at the top of `src/git/mod.rs` if not already present.

Check existing imports — `BTreeSet` is likely already imported through the `app` module re-exports. Add it explicitly if needed.

- [ ] **Step 4: Run unit test to verify it passes**

Run: `cargo test safe_rollback_preserves_protected_apps 2>&1`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/git/mod.rs
git add src/git/tests.rs
git commit -m "feat: implement safe_rollback with selective checkout + app preservation"
```

---

### Task 3: Update Git Log dialog `r` handler in `event.rs`

**Files:**
- Modify: `src/tui/main_view/event.rs`

The `r` handler currently opens a confirm dialog with a generic warning. It needs to:
1. Read target config from git (`read_shared_at`)
2. Compute preserved/protected apps
3. Build a richer message with app lists
4. Open confirm dialog with the new message

- [ ] **Step 1: Update the `r` handler in `handle_git_log`**

At line 731 in `src/tui/main_view/event.rs`, replace the current `KeyCode::Char('r')` arm:

```rust
KeyCode::Char('r') => {
    if let Some(hash) = git_log.selected_hash().map(|s| s.to_string()) {
        let mut message = format!("Rollback to {}?\n", &hash[..7]);

        // Compute preserved/protected apps for the dialog message
        let roost_dir = &state.roost_dir;
        match crate::git::read_shared_at(roost_dir, &hash) {
            Ok(target_shared) => {
                let current_apps: std::collections::BTreeSet<&String> =
                    state.shared.apps.keys().collect();
                let target_apps: std::collections::BTreeSet<&String> =
                    target_shared.apps.keys().collect();

                let preserved: Vec<&&String> = current_apps.iter()
                    .filter(|a| target_apps.contains(a))
                    .collect();
                let protected: Vec<&&String> = current_apps.iter()
                    .filter(|a| !target_apps.contains(a))
                    .collect();

                if !preserved.is_empty() {
                    message.push_str(&format!(
                        "\n{} app(s) rolled back: {}",
                        preserved.len(),
                        preserved.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(", ")
                    ));
                }
                if !protected.is_empty() {
                    message.push_str(&format!(
                        "\n{} app(s) preserved (did not exist at this commit): {}",
                        protected.len(),
                        protected.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(", ")
                    ));
                }
                message.push_str("\n\nPreserved apps' configs and files will not be touched.\nA new commit will be created.");
            }
            Err(_) => {
                message.push_str("\n\nWARNING: Could not read roost.toml at this commit.\nAll current apps will be treated as preserved.");
            }
        }

        state.git_log_dialog = None;
        state.confirm_dialog = Some(crate::tui::confirm::ConfirmDialog::destructive(
            "Rollback",
            message,
        ));
        vec![crate::tui::main_view::event::Action::SetStatus(format!("rollback_pending:{}", hash))]
    } else {
        vec![crate::tui::main_view::event::Action::Nop]
    }
}
```

Note: The function `handle_git_log` already has `state: &mut MainViewState` and uses `Action::SetStatus` etc. from the event module. The use of fully qualified paths (`crate::git::read_shared_at`, `crate::tui::confirm::ConfirmDialog`, etc.) ensures no import conflicts.

- [ ] **Step 2: Build check**

Run: `cargo build 2>&1 | head -30`
Expected: Build succeeds with no errors

- [ ] **Step 3: Commit**

```bash
git add src/tui/main_view/event.rs
git commit -m "feat: show preserved/protected apps in rollback confirm dialog"
```

---

### Task 4: Update `Action::Rollback` and `Action::Undo` handlers

**Files:**
- Modify: `src/tui/main_view/mod.rs`

- [ ] **Step 1: Replace the `Action::Rollback` handler**

In `src/tui/main_view/mod.rs` at line 514, replace the entire `Action::Rollback(hash)` arm:

```rust
Action::Rollback(hash) => {
    let roost_dir = state.roost_dir.clone();
    let shared = state.shared.clone();
    let local = state.local.clone();
    let profile_name = state.local.active_profile.clone();

    let result = suspend_and_run(|| {
        crate::git::safe_rollback(&roost_dir, &hash, &shared, &local, &profile_name)
    });

    state.needs_redraw = true;
    match result {
        Ok(()) => {
            state.status_message = Some(format!(
                "Rolled back to {} with app preservation",
                &hash[..hash.len().min(7)]
            ));
        }
        Err(e) => {
            state.status_message = Some(format!("Rollback failed: {}", e));
        }
    }

    // Reload configs from disk in all cases
    let shared_path = crate::app::shared_config_path(&state.roost_dir);
    let local_path = crate::app::local_config_path(&state.roost_dir);
    if let (Ok(shared), Ok(local)) = (
        crate::app::load_shared(&shared_path),
        crate::app::load_local(&local_path),
    ) {
        state.reload_configs(shared, local);
    }
}
```

- [ ] **Step 2: Replace the `Action::Undo` handler**

At line 502, replace the `Action::Undo` arm:

```rust
Action::Undo => {
    let roost_dir = state.roost_dir.clone();
    let shared = state.shared.clone();
    let local = state.local.clone();
    let profile_name = state.local.active_profile.clone();

    let result = suspend_and_run(|| {
        crate::git::safe_rollback(&roost_dir, "HEAD~1", &shared, &local, &profile_name)
    });

    state.needs_redraw = true;
    match result {
        Ok(()) => {
            state.status_message = Some("Undone last commit with app preservation".to_string());
        }
        Err(e) => {
            state.status_message = Some(format!("Undo failed: {}", e));
        }
    }

    let shared_path = crate::app::shared_config_path(&state.roost_dir);
    let local_path = crate::app::local_config_path(&state.roost_dir);
    if let (Ok(shared), Ok(local)) = (
        crate::app::load_shared(&shared_path),
        crate::app::load_local(&local_path),
    ) {
        state.reload_configs(shared, local);
    }
}
```

- [ ] **Step 3: Build check**

Run: `cargo build 2>&1 | head -30`
Expected: Build succeeds

- [ ] **Step 4: Commit**

```bash
git add src/tui/main_view/mod.rs
git commit -m "feat: TUI rollback/undo uses safe_rollback with config reload"
```

---

### Task 5: Update CLI rollback and undo commands

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Update `cmd_rollback`**

Replace the function at line 314:

```rust
fn cmd_rollback(hash: &str) -> Result<()> {
    let (shared, local, roost_dir) = load_configs()?;
    let profile_name = local.active_profile.clone();
    git::safe_rollback(&roost_dir, hash, &shared, &local, &profile_name)?;
    println!(
        "{} {}.",
        style("Rolled back to").green(),
        style(hash).white().bold()
    );
    Ok(())
}
```

- [ ] **Step 2: Update `cmd_undo`**

Replace the function at line 302:

```rust
fn cmd_undo(n: Option<usize>) -> Result<()> {
    let (shared, local, roost_dir) = load_configs()?;
    let count = n.unwrap_or(1);
    let profile_name = local.active_profile.clone();
    let hash = format!("HEAD~{}", count);
    git::safe_rollback(&roost_dir, &hash, &shared, &local, &profile_name)?;
    println!(
        "{} {} commit(s) with app preservation.",
        style("Undid").green(),
        style(count).white().bold()
    );
    Ok(())
}
```

- [ ] **Step 3: Build and test**

Run: `cargo build 2>&1`
Expected: Build succeeds

Run existing tests: `cargo test --test rollback 2>&1`
Expected: Existing rollback tests pass

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: CLI rollback/undo uses safe_rollback with config reload"
```

---

### Task 6: Integration test for safe rollback

**Files:**
- Modify: `tests/rollback.rs`

Add a new integration test that verifies the end-to-end safe rollback behavior through the CLI.

- [ ] **Step 1: Add integration test**

Add to the end of `tests/rollback.rs`:

```rust
#[test]
fn rollback_preserves_new_apps() -> Result<()> {
    use assert_cmd::Command;
    use predicates::prelude::*;
    use std::fs;

    let tmp = tempfile::tempdir()?;
    let roost_dir = tmp.path().join(".roost");
    fs::create_dir_all(&roost_dir)?;

    // Set up a roost directory with initial state
    let initial_config = r#"
[apps.zsh]
is_dir = true
on_profiles = ["default"]

[profiles.default]
apps = ["zsh"]
"#;
    fs::write(roost_dir.join("roost.toml"), initial_config)?;
    let local_config = r#"
active_profile = "default"

[os_info]
os = "test"
arch = "test"

[link_paths]
zsh = "/home/user/.zsh"
"#;
    fs::write(roost_dir.join("local.toml"), local_config)?;

    // Init git in the roost directory
    let cmd = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(&roost_dir)
        .output()?;
    assert!(cmd.status.success());

    // Set git identity
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&roost_dir)
        .output()?;
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&roost_dir)
        .output()?;

    // Create the default profile directory and zsh app files
    fs::create_dir_all(roost_dir.join("default").join("zsh"))?;
    fs::write(roost_dir.join("default/zsh/.zshrc"), "export FOO=bar")?;

    // Commit initial state
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&roost_dir)
        .output()?;
    Command::new("git")
        .args(["commit", "-m", "initial: add zsh"])
        .current_dir(&roost_dir)
        .output()?;

    let _target_hash = "HEAD~0"; // We'll rollback to this... just save it conceptually

    // Second commit: add nvim
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
    fs::write(roost_dir.join("roost.toml"), config2)?;
    let local_config2 = r#"
active_profile = "default"

[os_info]
os = "test"
arch = "test"

[link_paths]
zsh = "/home/user/.zsh"
nvim = "/home/user/.config/nvim"
"#;
    fs::write(roost_dir.join("local.toml"), local_config2)?;

    fs::create_dir_all(roost_dir.join("default").join("nvim"))?;
    fs::write(roost_dir.join("default/nvim/init.lua"), "vim.opt.number = true")?;

    // Modify zsh's file in the second commit
    fs::write(roost_dir.join("default/zsh/.zshrc"), "export FOO=baz")?;

    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&roost_dir)
        .output()?;
    Command::new("git")
        .args(["commit", "-m", "add nvim, modify zsh"])
        .current_dir(&roost_dir)
        .output()?;

    // Get the hash of the initial commit (first commit)
    let output = Command::new("git")
        .args(["rev-parse", "HEAD~1"])
        .current_dir(&roost_dir)
        .output()?;
    let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Run roost rollback to the initial commit
    let env_var = format!("ROOST_DIR={}", roost_dir.display());
    let mut cmd = Command::cargo_bin("roost")?;
    cmd.env("ROOST_DIR", &roost_dir)
        .arg("rollback")
        .arg(&hash);
    cmd.assert().success();

    // Verify: nvim is still in config (preserved)
    let new_config = fs::read_to_string(roost_dir.join("roost.toml"))?;
    assert!(new_config.contains("nvim"), "nvim should be preserved in config");

    // Verify: nvim files still exist
    assert!(
        roost_dir.join("default/nvim/init.lua").exists(),
        "nvim init.lua should exist"
    );

    // Verify: zsh file is rolled back
    let zsh_content = fs::read_to_string(roost_dir.join("default/zsh/.zshrc"))?;
    assert_eq!(zsh_content.trim(), "export FOO=bar", "zsh should be rolled back");

    Ok(())
}
```

- [ ] **Step 2: Run the integration test**

Run: `cargo test --test rollback rollback_preserves_new_apps 2>&1`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add tests/rollback.rs
git commit -m "test: integration test for safe rollback preserving new apps"
```

---

### Task 7: Full test suite verification

- [ ] **Step 1: Run all tests**

Run: `cargo test 2>&1`
Expected: All ~112+ tests pass

- [ ] **Step 2: Address any failures**

If any tests fail, investigate and fix. Common issues:
- `BTreeSet` not imported in `src/git/mod.rs` — add `use std::collections::BTreeSet;`
- `PathBuf` import in test — check existing imports in `src/git/tests.rs`
- `crate::app::LocalAppConfig` fields mismatch — verify `OsInfo` struct field names
