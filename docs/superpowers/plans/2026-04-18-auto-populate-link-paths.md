# Auto-populate link_paths for cross-device profiles

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When switching profiles or syncing, automatically detect apps that exist on the filesystem but have no entry in `local.link_paths`, and populate the mapping so they work without manual `roost add`. Also fix `import_app_from_profile` to create `config.apps` entries when they're missing, and add a `roost adopt` command for recovery.

**Architecture:** Add a `resolve_missing_link_paths` function in `src/linker.rs` that scans the known source directories (`~/.config`, `$HOME`, `~/.local/bin`, etc. — same candidates as `scanner::get_likely_sources`) for apps referenced in the active profile but missing from `link_paths`. Call it from `run_sync`, `roost profile switch`, TUI profile switch, TUI launch, and `import_app_from_profile`. When the filesystem entry isn't found, silently skip (no link_path = no symlink = app is simply inactive on this device, which is harmless).

**Tech Stack:** Rust, existing `scanner::get_likely_sources()`, `dirs` crate

**Root cause analysis:**

Two bugs conspired:

1. **`import_app_from_profile` doesn't create `config.apps` entries** (linker.rs:614). When importing an app from one profile to another, the code does `if let Some(app) = config.apps.get_mut(app_name)` — if the app was removed from `config.apps` (by a bad git merge or removal), the import silently skips creating the entry. The app gets added to `profile.apps` and `profile.app_sources`, but has no corresponding `[apps.*]` section. This means the TUI (which filters by `config.apps`) never shows it.

2. **`local.link_paths` is device-local and not auto-populated**. The `link_paths` map is only populated during `roost init` / `roost add`. Apps from other devices have no entry, so `ensure_links` and `switch_links` silently skip them.

**Recovery (before implementing code changes):**

Your current `roost.toml` is missing all entries for nvim, opencode, ghostty, sketchybar, aerospace, raycast, borders, and aider. Rolling back `d6485c9` (the doctor --fix commit) would partially restore things but the `config.apps` entries were already missing before that commit (lost during the `git pull --rebase`). The `roost adopt` command in Task 6 will fix this automatically. Alternatively, a manual rollback to `cdd84dc` would restore the profile structure but still needs `config.apps` entries created.

---

### Task 1: Write `resolve_missing_link_paths` in `src/linker.rs`

**Files:**
- Modify: `src/linker.rs` (add new public function after `switch_links`, ~line 336)

- [ ] **Step 1: Write the failing test**

In `tests/multi_profile.rs`, add a test that simulates the cross-device scenario:

```rust
#[test]
fn test_switch_to_profile_auto_detects_link_paths() {
    let roost = TestRoost::new();
    roost.init_minimal();

    // Add nvim on default profile (simulating macbook setup)
    let nvim = roost.path(".config/nvim");
    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "config").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    // Create second profile with nvim referenced but no local link_path
    // (simulates git sync pulling profile from another device)
    roost
        .cmd()
        .args(["profile", "add", "laptop", "--empty"])
        .assert()
        .success();

    // Manually add nvim to laptop profile in shared config (simulating sync)
    let mut config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    config
        .profiles
        .get_mut("laptop")
        .unwrap()
        .apps
        .insert("nvim".to_string());
    config
        .apps
        .get_mut("nvim")
        .unwrap()
        .on_profiles
        .push("laptop".to_string());
    config.save(&roost.roost_config).unwrap();

    // Create nvim files in laptop profile dir (simulating sync pulling files)
    let laptop_prof_dir = roost.roost_dir.join("laptop");
    fs::create_dir_all(laptop_prof_dir.join("nvim")).unwrap();
    fs::write(laptop_prof_dir.join("nvim").join("init.lua"), "config").unwrap();

    // Switch to laptop — nvim has no link_path entry yet
    roost
        .cmd()
        .args(["profile", "switch", "laptop"])
        .assert()
        .success();

    // Verify the symlink was created even though link_path wasn't manually set
    let link = roost.path(".config/nvim");
    assert!(link.is_symlink(), "nvim should be symlinked after profile switch");
    let target = fs::read_link(&link).unwrap();
    assert!(
        target.starts_with(&roost.roost_dir),
        "symlink should point into roost dir"
    );
    assert!(
        link.exists(),
        "nvim symlink should be valid (not broken)"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test multi_profile test_switch_to_profile_auto_detects_link_paths`
Expected: FAIL — nvim is silently skipped because it has no `link_paths` entry

- [ ] **Step 3: Implement `resolve_missing_link_paths`**

Add to `src/linker.rs` after `switch_links` (~line 336):

```rust
/// For apps in the given profile that have no entry in `local.link_paths`,
/// try to find them on the filesystem by scanning known source directories.
/// Populates `link_paths` for any matches found. Returns the number resolved.
pub fn resolve_missing_link_paths(
    profile_name: &str,
    config: &crate::app::SharedAppConfig,
    local: &mut crate::app::LocalAppConfig,
) -> usize {
    let profile = match config.profiles.get(profile_name) {
        Some(p) => p,
        None => return 0,
    };

    let sources = crate::scanner::get_likely_sources();
    let mut resolved = 0;

    for app_name in &profile.apps {
        if local.link_paths.contains_key(app_name) {
            continue;
        }

        if let Some(path) = find_app_on_filesystem(app_name, &sources) {
            local.link_paths.insert(app_name.clone(), path);
            resolved += 1;
        }
    }

    resolved
}

/// Search known source directories for an entry matching `app_name`.
/// Checks for both a directory and a file with the exact name.
/// Returns None if not found — the app simply won't be linked on this device.
pub fn find_app_on_filesystem(
    app_name: &str,
    sources: &[std::path::PathBuf],
) -> Option<std::path::PathBuf> {
    for source in sources {
        let dir_candidate = source.join(app_name);
        if dir_candidate.is_dir() {
            return Some(dir_candidate);
        }
        let file_candidate = source.join(app_name);
        if file_candidate.is_file() {
            return Some(file_candidate);
        }
    }

    // Check for dotfile variant in $HOME (e.g. "gitconfig" → "$HOME/.gitconfig")
    if let Some(home) = dirs::home_dir() {
        let dot_candidate = home.join(format!(".{}", app_name));
        if dot_candidate.exists() {
            return Some(dot_candidate);
        }
    }

    None
}
```

**Behavior when not found:** The function returns 0 for that app and it stays out of `link_paths`. It will be silently skipped by `ensure_links`/`switch_links`. This is the desired behavior — an app like `sketchybar` (macOS-only) won't exist in `~/.config/sketchybar` on a Linux box, so it won't get a link_path and won't be symlinked. No error, no warning — it's just inactive on this device.

- [ ] **Step 4: Wire into `run_sync` and `roost profile switch`**

In `src/main.rs`, `run_sync` (line 90-98), call `resolve_missing_link_paths` before `ensure_links`:

Change:
```rust
fn run_sync() -> color_eyre::Result<()> {
    let (roost_dir, roost_config, local_config_path) = roost_paths()?;
    git::sync(&roost_dir)?;
    let config = app::SharedAppConfig::load(&roost_config)?;
    let mut local = app::LocalAppConfig::load(&local_config_path)?;
    app::migrate_link_paths_if_needed(&roost_config, &mut local, &local_config_path)?;
    linker::ensure_links(&config, &local, &roost_dir);
    Ok(())
}
```

To:
```rust
fn run_sync() -> color_eyre::Result<()> {
    let (roost_dir, roost_config, local_config_path) = roost_paths()?;
    git::sync(&roost_dir)?;
    let config = app::SharedAppConfig::load(&roost_config)?;
    let mut local = app::LocalAppConfig::load(&local_config_path)?;
    app::migrate_link_paths_if_needed(&roost_config, &mut local, &local_config_path)?;
    let active = local.active_profile.clone();
    let resolved = linker::resolve_missing_link_paths(&active, &config, &mut local);
    if resolved > 0 {
        local.save(&local_config_path)?;
    }
    linker::ensure_links(&config, &local, &roost_dir);
    Ok(())
}
```

In `src/main.rs`, `roost profile switch` (line 148-163), call it before `switch_links`:

Change:
```rust
            let old_profile = local.active_profile.clone();
            linker::switch_links(&old_profile, name, &config, &local, &roost_dir);
            local.active_profile = name.to_string();
            local.save(&local_config_path)?;
            linker::ensure_links(&config, &local, &roost_dir);
```

To:
```rust
            let old_profile = local.active_profile.clone();
            linker::resolve_missing_link_paths(name, &config, &mut local);
            linker::switch_links(&old_profile, name, &config, &local, &roost_dir);
            local.active_profile = name.to_string();
            local.save(&local_config_path)?;
            linker::ensure_links(&config, &local, &roost_dir);
```

Note: `resolve_missing_link_paths` is called for the *new* profile (`name`), not the active one, so it populates link_paths for apps in the profile we're switching *to*.

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --test multi_profile test_switch_to_profile_auto_detects_link_paths`
Expected: PASS

- [ ] **Step 6: Run full test suite**

Run: `cargo test`
Expected: All existing tests pass (no regressions)

- [ ] **Step 7: Commit**

```
git add src/linker.rs src/main.rs tests/multi_profile.rs
git commit -m "feat: auto-populate link_paths for apps found on filesystem"
```

---

### Task 2: Wire into TUI profile switch and launch

**Files:**
- Modify: `src/tui/main_view/state.rs` (`profile_accept_switch` at line 509, `new` at line 64)

- [ ] **Step 1: Add `resolve_missing_link_paths` call in `profile_accept_switch`**

In `src/tui/main_view/state.rs`, `profile_accept_switch` (line 509-538):

Change:
```rust
        let old_profile = self.active_profile.clone();
        crate::linker::switch_links(
            &old_profile,
            &name,
            &self.config,
            &self.local,
            &self.roost_dir,
        );
```

To:
```rust
        let old_profile = self.active_profile.clone();
        crate::linker::resolve_missing_link_paths(&name, &self.config, &mut self.local);
        crate::linker::switch_links(
            &old_profile,
            &name,
            &self.config,
            &self.local,
            &self.roost_dir,
        );
```

- [ ] **Step 2: Add `resolve_missing_link_paths` call in `MainViewTui::new`**

In `src/tui/main_view/state.rs`, after line 78 (`app_names.sort()`), add:

```rust
        let _ = crate::linker::resolve_missing_link_paths(&active_profile, &config, &mut local);
```

The `let _ =` is intentional — we don't need to save here because `local` gets saved later when the TUI exits or on next profile switch.

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: All pass

- [ ] **Step 4: Commit**

```
git add src/tui/main_view/state.rs
git commit -m "feat: auto-populate link_paths in TUI profile switch and launch"
```

---

### Task 3: Fix `import_app_from_profile` to create `config.apps` entries

**Files:**
- Modify: `src/linker.rs` (update `import_app_from_profile` at ~line 614)
- Modify: `tests/multi_profile.rs` (add test)

**Bug:** At linker.rs:614, `import_app_from_profile` does `if let Some(app) = config.apps.get_mut(app_name)` — if the app doesn't exist in `config.apps`, the `on_profiles` update is silently skipped. The app gets added to `profile.apps` and `profile.app_sources`, but has no `[apps.*]` entry, so the TUI never shows it.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn test_import_creates_missing_apps_entry() {
    let roost = TestRoost::new();
    roost.init_minimal();

    // Add nvim to default profile
    let nvim = roost.path(".config/nvim");
    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "config").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    // Remove nvim from config.apps entirely (simulates bad git merge)
    let mut config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    config.apps.remove("nvim");
    config.save(&roost.roost_config).unwrap();

    // Create laptop profile
    roost
        .cmd()
        .args(["profile", "add", "laptop", "--empty"])
        .assert()
        .success();

    // Import nvim from default into laptop — should recreate the apps entry
    let mut config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    let mut local = roost::app::LocalAppConfig::load(&roost.local_config).unwrap();
    let result = roost::linker::import_app_from_profile(
        "nvim",
        "laptop",
        "default",
        &mut config,
        &roost.roost_config,
        &roost.roost_dir,
        &mut local,
    );
    assert!(result.is_ok(), "import should succeed");

    // Verify apps entry was recreated
    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(config.apps.contains_key("nvim"), "nvim should be in config.apps after import");
    assert!(
        config.apps["nvim"].on_profiles.contains(&"laptop".to_string()),
        "nvim should list laptop in on_profiles"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test multi_profile test_import_creates_missing_apps_entry`
Expected: FAIL — nvim is not in config.apps after import

- [ ] **Step 3: Fix `import_app_from_profile`**

Change linker.rs:614-617 from:

```rust
    if let Some(app) = config.apps.get_mut(app_name)
        && !app.on_profiles.contains(&to_profile.to_string()) {
            app.on_profiles.push(to_profile.to_string());
        }
```

To:

```rust
    if let Some(app) = config.apps.get_mut(app_name) {
        if !app.on_profiles.contains(&to_profile.to_string()) {
            app.on_profiles.push(to_profile.to_string());
        }
    } else {
        // App was removed from config.apps (e.g. bad git merge) — recreate it
        config.apps.insert(
            app_name.to_string(),
            crate::app::Application {
                name: app_name.to_string(),
                primary_config: None,
                on_profiles: vec![to_profile.to_string()],
            },
        );
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test multi_profile test_import_creates_missing_apps_entry`
Expected: PASS

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: All pass

- [ ] **Step 6: Commit**

```
git add src/linker.rs tests/multi_profile.rs
git commit -m "fix: import_app_from_profile creates config.apps entry when missing"
```

---

### Task 4: Wire auto-detect into `import_app_from_profile` link_path

**Files:**
- Modify: `src/linker.rs` (update `import_app_from_profile` signature and link_path lookup)
- Modify: `src/tui/main_view/state.rs` (update caller at line 850)
- Modify: `tests/multi_profile.rs` (add test)

Currently `import_app_from_profile` (linker.rs:567-569) hard-fails if the app has no `link_path`:
```rust
let link_path = link_paths
    .get(app_name)
    .ok_or_else(|| eyre!("No local link path for app '{}' on this device.", app_name))?;
```

This means the TUI "f" command fails for any app that hasn't been `roost add`-ed on the current device.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn test_import_auto_detects_missing_link_path() {
    let roost = TestRoost::new();
    roost.init_minimal();

    // Add nvim to default profile
    let nvim = roost.path(".config/nvim");
    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "config").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    // Create laptop profile with empty link_paths (simulates cross-device)
    roost
        .cmd()
        .args(["profile", "add", "laptop", "--empty"])
        .assert()
        .success();

    // Clear link_paths to simulate fresh device
    let mut local = roost::app::LocalAppConfig::load(&roost.local_config).unwrap();
    local.link_paths.clear();
    local.save(&roost.local_config).unwrap();

    // Import nvim — should auto-detect link_path from filesystem
    let mut config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    let mut local = roost::app::LocalAppConfig::load(&roost.local_config).unwrap();
    let result = roost::linker::import_app_from_profile(
        "nvim",
        "laptop",
        "default",
        &mut config,
        &roost.roost_config,
        &roost.roost_dir,
        &mut local,
    );
    assert!(result.is_ok(), "import should auto-detect link_path");

    // Verify symlink was created
    let link = roost.path(".config/nvim");
    assert!(link.is_symlink(), "nvim should be symlinked after import");
    assert!(link.exists(), "symlink should be valid");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test multi_profile test_import_auto_detects_missing_link_path`
Expected: FAIL with "No local link path"

- [ ] **Step 3: Update `import_app_from_profile` signature and link_path lookup**

Change the signature (linker.rs:514-521) to accept `&mut LocalAppConfig` instead of `&HashMap<String, PathBuf>`:

```rust
pub fn import_app_from_profile(
    app_name: &str,
    to_profile: &str,
    source_profile: &str,
    config: &mut crate::app::SharedAppConfig,
    config_path: &Path,
    roost_dir: &Path,
    local: &mut crate::app::LocalAppConfig,
) -> color_eyre::Result<()> {
```

Replace the link_path lookup (old line 567-569):

```rust
    let link_path = link_paths
        .get(app_name)
        .ok_or_else(|| eyre!("No local link path for app '{}' on this device.", app_name))?;
```

With:

```rust
    let link_path = if let Some(lp) = local.link_paths.get(app_name) {
        lp.clone()
    } else if let Some(detected) = find_app_on_filesystem(app_name, &crate::scanner::get_likely_sources()) {
        local.link_paths.insert(app_name.clone(), detected.clone());
        detected
    } else {
        return Err(eyre!(
            "No link path for '{}' on this device. Place its config at a standard location (e.g. ~/.config/{}) or run `roost add <path>` first.",
            app_name, app_name
        ));
    };
```

- [ ] **Step 4: Update the TUI caller**

In `src/tui/main_view/state.rs:850-858`:

Change:
```rust
                    crate::linker::import_app_from_profile(
                        &app_name,
                        &active,
                        &source_profile,
                        &mut self.config,
                        &self.config_path,
                        &self.roost_dir,
                        &self.local.link_paths,
                    )?;
```

To:
```rust
                    crate::linker::import_app_from_profile(
                        &app_name,
                        &active,
                        &source_profile,
                        &mut self.config,
                        &self.config_path,
                        &self.roost_dir,
                        &mut self.local,
                    )?;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --test multi_profile test_import_auto_detects_missing_link_path`
Expected: PASS

- [ ] **Step 6: Run full test suite**

Run: `cargo test`
Expected: All pass

- [ ] **Step 7: Commit**

```
git add src/linker.rs src/tui/main_view/state.rs tests/multi_profile.rs
git commit -m "feat: auto-detect link_path when importing apps across profiles"
```

---

### Task 5: Verify sourced apps work end-to-end

**Files:**
- Modify: `tests/multi_profile.rs` (add test)

Sourced apps (e.g. nvim in venus sourced from Shared) are in `profile.apps` AND in `profile.app_sources`. The function iterates `profile.apps`, so sourced apps are included in the scan. This test confirms the full flow.

- [ ] **Step 1: Write the test**

```rust
#[test]
fn test_auto_detect_resolves_sourced_apps() {
    let roost = TestRoost::new();
    roost.init_minimal();

    // Add nvim to default
    let nvim = roost.path(".config/nvim");
    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "config").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    // Create laptop profile with nvim sourced from default
    roost
        .cmd()
        .args(["profile", "add", "laptop", "--empty"])
        .assert()
        .success();

    let mut config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    config
        .profiles
        .get_mut("laptop")
        .unwrap()
        .apps
        .insert("nvim".to_string());
    config
        .profiles
        .get_mut("laptop")
        .unwrap()
        .app_sources
        .insert("nvim".to_string(), "default".to_string());
    config
        .apps
        .get_mut("nvim")
        .unwrap()
        .on_profiles
        .push("laptop".to_string());
    config.save(&roost.roost_config).unwrap();

    // Switch to laptop — nvim is sourced, not local to this profile
    roost
        .cmd()
        .args(["profile", "switch", "laptop"])
        .assert()
        .success();

    let link = roost.path(".config/nvim");
    assert!(link.is_symlink(), "sourced nvim should be symlinked");
    assert!(link.exists(), "sourced nvim symlink should not be broken");
}
```

- [ ] **Step 2: Run test**

Run: `cargo test --test multi_profile test_auto_detect_resolves_sourced_apps`
Expected: PASS

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: All pass

- [ ] **Step 4: Commit**

```
git add tests/multi_profile.rs
git commit -m "test: verify auto-detect works for sourced apps"
```

---

### Task 6: Add `roost adopt` command for recovery

**Files:**
- Modify: `src/main.rs` (add `adopt` subcommand)
- Modify: `src/linker.rs` (add `adopt_orphaned_apps` function)
- Create: `tests/adopt.rs`

This command scans all profile directories for files that aren't referenced in `config.apps` and creates the missing entries. It's the recovery path for the current broken state (and for future git merge conflicts).

- [ ] **Step 1: Write the failing test**

```rust
// tests/adopt.rs
mod helpers;

use helpers::TestRoost;
use std::fs;

fn setup() -> TestRoost {
    let roost = TestRoost::new();
    roost.init_minimal();
    roost
}

#[test]
fn test_adopt_creates_missing_app_entries() {
    let roost = setup();

    // Add nvim to default profile normally
    let nvim = roost.path(".config/nvim");
    fs::create_dir_all(&nvim).unwrap();
    fs::write(nvim.join("init.lua"), "config").unwrap();
    roost.cmd().arg("add").arg(&nvim).assert().success();

    // Remove nvim from config.apps (simulates bad merge)
    let mut config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    config.apps.remove("nvim");
    config.save(&roost.roost_config).unwrap();

    // Files still exist in profile dir
    assert!(
        roost.roost_dir.join("default/nvim").exists(),
        "nvim files should still be in profile dir"
    );

    // Run adopt
    roost.cmd().arg("adopt").assert().success();

    // Verify nvim is back in config.apps
    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(config.apps.contains_key("nvim"), "adopt should recreate nvim entry");
    assert!(
        config.profiles["default"].apps.contains("nvim"),
        "profile should still reference nvim"
    );
}

#[test]
fn test_adopt_creates_entries_for_orphaned_files() {
    let roost = setup();

    // Create a file in the profile dir that has no config entry at all
    fs::create_dir_all(roost.roost_dir.join("default/ghostty")).unwrap();
    fs::write(roost.roost_dir.join("default/ghostty/config"), "test").unwrap();

    // Add it to the profile's apps set (simulating a partial sync)
    let mut config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    config
        .profiles
        .get_mut("default")
        .unwrap()
        .apps
        .insert("ghostty".to_string());
    config.save(&roost.roost_config).unwrap();

    // Run adopt
    roost.cmd().arg("adopt").assert().success();

    // Verify ghostty entry was created
    let config = roost::app::SharedAppConfig::load(&roost.roost_config).unwrap();
    assert!(config.apps.contains_key("ghostty"), "adopt should create ghostty entry");
    assert!(
        config.apps["ghostty"].on_profiles.contains(&"default".to_string()),
        "ghostty should list default in on_profiles"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test adopt`
Expected: FAIL — no `adopt` subcommand exists

- [ ] **Step 3: Implement `adopt_orphaned_apps` in `src/linker.rs`**

```rust
/// Scan all profiles for apps referenced in `profile.apps` but missing from
/// `config.apps`. Create minimal Application entries for any found.
/// Returns the number of entries created.
pub fn adopt_orphaned_apps(
    config: &mut crate::app::SharedAppConfig,
    config_path: &Path,
) -> usize {
    let mut adopted = 0;

    for (prof_name, profile) in &config.profiles {
        for app_name in &profile.apps {
            if config.apps.contains_key(app_name) {
                continue;
            }

            config.apps.insert(
                app_name.clone(),
                crate::app::Application {
                    name: app_name.clone(),
                    primary_config: None,
                    on_profiles: vec![prof_name.clone()],
                },
            );
            adopted += 1;
        }
    }

    if adopted > 0 {
        // Deduplicate on_profiles: if an app was adopted into multiple profiles,
        // merge their on_profiles lists.
        for app in config.apps.values_mut() {
            app.on_profiles.sort();
            app.on_profiles.dedup();
        }
        let _ = config.save(config_path);
    }

    adopted
}
```

- [ ] **Step 4: Wire into CLI in `src/main.rs`**

Add a new match arm in the subcommand dispatch. Find where other subcommands like `doctor` are dispatched and add:

```rust
        "adopt" => {
            let (roost_dir, roost_config, local_config_path) = roost_paths()?;
            let mut config = app::SharedAppConfig::load(&roost_config)?;
            let adopted = linker::adopt_orphaned_apps(&mut config, &roost_config);
            if adopted == 0 {
                println!("Nothing to adopt. All apps are properly registered.");
            } else {
                println!("Adopted {} app(s). Run `roost sync` to push changes.", adopted);
                if git::is_git_repo(&roost_dir) {
                    let _ = git::auto_commit(&roost_dir, &format!("adopt {} app(s)", adopted));
                }
            }
            Ok(())
        }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test adopt`
Expected: PASS

- [ ] **Step 6: Run full test suite**

Run: `cargo test`
Expected: All pass

- [ ] **Step 7: Commit**

```
git add src/main.rs src/linker.rs tests/adopt.rs
git commit -m "feat: add roost adopt command to recover missing app entries"
```

---

### Self-Review

**1. Spec coverage:**
- Auto-populate link_paths on sync → Task 1 Step 4
- Auto-populate on profile switch (CLI) → Task 1 Step 4
- Auto-populate on profile switch (TUI) and TUI launch → Task 2
- Import creates config.apps entry when missing → Task 3
- Import auto-detects link_path when missing → Task 4
- Sourced apps work end-to-end → Task 5
- Recovery from broken state → Task 6 (`roost adopt`)
- Apps not found on filesystem → silently skipped

**2. Placeholder scan:** All steps have concrete code. No TBDs.

**3. Type consistency:**
- `resolve_missing_link_paths`: `(profile_name: &str, config: &SharedAppConfig, local: &mut LocalAppConfig) -> usize`
- `adopt_orphaned_apps`: `(config: &mut SharedAppConfig, config_path: &Path) -> usize`
- `import_app_from_profile`: last param changed from `&HashMap<String, PathBuf>` to `&mut LocalAppConfig`
- `find_app_on_filesystem`: `(app_name: &str, sources: &[PathBuf]) -> Option<PathBuf>` — `pub` so it can be reused
