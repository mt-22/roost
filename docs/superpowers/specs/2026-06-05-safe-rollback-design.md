# Safe Rollback: Selective Rollback with App Preservation

**Date:** 2026-06-05
**Status:** Draft

## Problem

Two bugs in the current rollback/undo system:

**Bug 1 — Stale in-memory state after undo:** `Action::Undo` runs `git reset --hard HEAD~1` but never reloads in-memory configs (`state.shared` / `state.local`). The TUI continues showing stale apps that no longer exist on disk.

**Bug 2 — Data loss from destructive rollback:** `git reset --hard <hash>` destroys managed app files for any app that was added after the target commit. The original location still has a symlink pointing to the now-nonexistent managed path, leaving a broken symlink and no way to recover the data.

## Design Overview

Replace `git reset --hard` with a **selective checkout** strategy:

1. Before any destructive operation, load and parse the target commit's `roost.toml`
2. Compare with current config to identify **preserved apps** (exist at target → rolled back) and **protected apps** (added after target → left intact)
3. Use `git checkout <hash> -- <path>` to selectively restore only preserved app directories and the config file
4. Surgically re-add protected app entries back into the rolled-back config
5. Commit the result as a new forward commit

Protected apps' managed files are **never touched**. No backup copies needed.

## Architecture

### New function: `git::safe_rollback()`

Added to `src/git/mod.rs`. A single function encapsulating the entire safe rollback operation:

```rust
pub fn safe_rollback(
    roost_dir: &Path,
    hash: &str,
    pre_shared: &SharedAppConfig,
    pre_local: &LocalAppConfig,
    profile_name: &str,
) -> Result<()>
```

Called by both TUI (`Action::Rollback`, `Action::Undo`) and CLI (`cmd_rollback`, `cmd_undo`).

## Detailed Flow

### Phase 1: Analysis (inside `safe_rollback`)

1. Read target config: `git show <hash>:roost.toml` → parse as `SharedAppConfig`
2. If `roost.toml` doesn't exist at target (very old commit), treat all current apps as protected
3. Compute sets:
   - `target_apps`: union of all apps across all profiles in target config
   - `current_apps`: union of all apps across all profiles in current config
   - `protected_apps` = `current_apps` - `target_apps` (apps added after target commit)
   - `preserved_apps` = `current_apps` ∩ `target_apps` (apps that existed at target → get rolled back)

### Phase 2: Selective checkout

4. For each **preserved app**: `git checkout <hash> -- <profile>/<app_name>/`
   - Resolve the correct profile path using the active profile
   - This restores the managed directory to its state at the target commit

5. Restore top-level tracked files: `git checkout <hash> -- roost.toml`
6. Restore `.gitignore`: `git checkout <hash> -- .gitignore`

Note: `git checkout` with a commit hash + path restores the file from that commit WITHOUT moving HEAD. The working tree reflects the target state for these files. Protected app directories are never passed to `git checkout`, so they remain untouched.

### Phase 3: Config repair

7. Reload `roost.toml` from disk (now at target state)
8. For each **protected app**:
   - Restore `shared.apps[app_name]` from the pre-rollback config (saved as `pre_shared`)
   - For each profile that contained this app pre-rollback, re-add the app to the profile's `apps` set
   - Restore any `app_sources` entries for cross-profile symlinks
   - Restore `local.link_paths[app_name]`

9. Call `linker::ensure_links(&shared, &mut local, roost_dir)` to fix up symlinks
10. Save `local.toml` with updated `link_paths`

### Phase 4: Commit

11. Stage all changes: `git add -A`
12. Commit: `git commit -m "rollback to <hash> + preserve N app(s)"`
13. Set `pending_auto_commit = None` (already committed here)

## TUI Changes

### Confirm dialog message

The `r` handler in `event.rs` will do the analysis phase before opening the confirm dialog, and store the results (preserved app list, protected app list) to render a richer message:

```
Rollback to abc1234?

2 apps rolled back: git, nvim
1 app preserved (did not exist at this commit): lazyvim

Preserved apps' configs and files will not be touched.
A new commit will be created.
```

The existing `ConfirmDialog` is reused — just a richer message string.

### `Action::Rollback` handler

Changes from:
```rust
Action::Rollback(hash) => {
    // suspend TUI
    crate::git::rollback(&roost_dir, &hash)?;  // git reset --hard
    // reload configs
    // ensure_links
    // reload state
}
```

To:
```rust
Action::Rollback(hash) => {
    // suspend TUI
    git::safe_rollback(&roost_dir, &hash, &state.shared, &state.local, state.active_profile_name())?;
    // reload configs from disk
    // rebuild state
}
```

### `Action::Undo` handler

Currently runs `git::undo()` (a simple `git reset --hard HEAD~1`) with no config reload or link repair. Changes to call `git::safe_rollback()` with `hash = "HEAD~1"`:

```rust
Action::Undo => {
    git::safe_rollback(&roost_dir, "HEAD~1", &state.shared, &state.local, state.active_profile_name())?;
    // reload and rebuild
}
```

This fixes the stale-state bug (Bug 1) because `safe_rollback` always returns a clean new state, and the caller reloads configs afterward.

### Git Log dialog `r` key handler

The `r` handler currently just opens a confirm dialog. It needs to additionally:
1. Compute the analysis (protected vs preserved apps) before showing the confirm dialog
2. Include the analysis results in the confirm dialog message
3. This requires access to `state.shared` and `state.local`

The analysis results (`preserved_apps`, `protected_apps`) are computed in the event handler and passed as part of the confirm dialog message string.

## CLI Changes

### `cmd_rollback` and `cmd_undo`

Both currently just call `git::rollback()` / `git::undo()` with no config reload or link repair. They need to:

1. Load current shared/local configs before the operation
2. Call `git::safe_rollback()` (same function as TUI)
3. Reload configs afterward
4. Print confirmation with app count info

## Edge Cases

- **No `roost.toml` at target**: Target config parse fails. Treat all current apps as protected. Commit still happens.
- **Protected app with `app_sources`**: Cross-profile link preserved by restoring the `app_sources` entry along with the app config.
- **Protected app in multiple profiles**: All profile memberships are restored from `pre_shared.profiles`.
- **Target hash is current HEAD**: No-op. `git checkout` should have nothing to change. Still commit? Probably skip.
- **Empty preserved list**: No app directories to checkout. Only config files are restored. Protected apps carry forward.
- **Empty protected list**: All apps are preserved apps. The rollback is a complete `git checkout` to target state, followed by a commit. No config repair needed.
- **Rollback via CLI with `roost rollback <hash>`**: Same `safe_rollback` function, but needs to load current configs first (currently `cmd_rollback` doesn't do this).

## Files to Modify

| File | Change |
|------|--------|
| `src/git/mod.rs` | Add `safe_rollback()` function. `undo()` and `rollback()` remain for non-TUI usage? Or remove entirely? |
| `src/tui/main_view/mod.rs` | Update `Action::Rollback` and `Action::Undo` handlers to call `safe_rollback` + reload configs |
| `src/tui/main_view/event.rs` | Update `r` key handler to compute analysis before confirm dialog, build richer message |
| `src/main.rs` | Update `cmd_rollback` and `cmd_undo` to load configs, call `safe_rollback`, reload, print info |
