# Plan: Fix Multi-Device Sync Gaps

## Overview

This plan addresses the critical and high-severity gaps identified in `TODO.md` that prevent Roost from working correctly across multiple devices. The goal is SPEC compliance for the sync subsystem.

## Commit Strategy

**Small, targeted commits — one concern per commit.** Each commit message will reference the specific files modified (e.g., `fix(git): add push after rebase — src/git/mod.rs, src/main.rs`).

---

## Phase 1: Critical — Sync is Pull-Only

### Commit 1.1: `fix(git): add git push after successful rebase`
**Files:** `src/git/mod.rs`, `src/main.rs`, `src/tui/main_view/mod.rs`

**Problem:** `git::sync()` fetches, merges, commits, and rebases — but never pushes. Changes never leave the local machine.

**Fix:** After the rebase succeeds (or was resolved) at `src/git/mod.rs:272`, add:
```rust
run_git(roost_dir, &["push", "origin", "main"])?;
```

**Why after rebase:** Per SPEC, sync must ensure local main is fast-forwarded onto origin/main (or rebased), then push. Pushing before rebase would fail if local is behind.

**CLI impact:** `main.rs:405-424` — sync output should report push success/failure.
**TUI impact:** `tui/main_view/mod.rs:215-228` — same, plus handle push error gracefully.

---

## Phase 2: Critical — New Device Can't Reconstruct `local.toml`

### Commit 2.1: `feat(init): when roost.toml exists but local.toml is missing, reconstruct local state instead of adding a new empty profile`
**Files:** `src/init.rs`, `src/linker/mod.rs`

**Problem:** Fresh clone of roost repo has `roost.toml` but no `local.toml` (gitignored). Current init wizard detects `existing_shared=true` but still adds a new empty profile and creates `local.toml` with empty `link_paths`. Existing apps from `roost.toml` have no symlinks because `ensure_links()` skips apps with missing `link_paths`.

**Design:** In `src/init.rs`, when `existing_shared=true` and `existing_local=false`:
1. **Detect the scenario:** Print "Existing shared config found. Reconstructing local state..."
2. **Profile selection:** Prompt user to pick an existing profile from `roost.toml` as active, instead of creating a new one. Default to the first profile.
3. **Auto-discovery of link_paths:** For each app already in the chosen profile:
   - Check if a symlink already exists at a likely original path (e.g. `~/.config/<app>`).
   - If a symlink is found and it points into the roost profile dir, auto-register the original path in `link_paths`.
   - If no symlink is found, prompt user with `dialoguer::Input` for the original path, with a default guess of `~/.config/<app>` (or skip).
4. **Write `local.toml`** with populated `link_paths` and chosen active profile.
5. **Run `linker::ensure_links()`** to create symlinks for all discovered paths.

**Edge cases:**
- App has no discoverable path and user skips → `link_paths` entry is absent (same as current behavior, but now intentional and explicit).
- Multiple profiles → user picks one as active; other profiles remain in `roost.toml` for future switching.
- No existing profiles in `roost.toml` → fall back to normal init behavior (create new profile).

---

### Commit 2.2: `fix(linker): rebuild link_paths during ensure_links when origin is a known symlink`
**Files:** `src/linker/mod.rs`

**Problem:** `linker::ensure_links()` skips apps with missing `link_paths` (`src/linker/mod.rs:148-154`). Even if the app is in `roost.toml`, a fresh device can't create symlinks.

**Design:** In `ensure_links()`, before skipping due to missing `link_paths`:
1. Check if a known default path for the app exists (e.g. `~/.config/<app>`) and is already a symlink pointing into the roost profile dir.
2. If so, derive the `link_path` from the symlink's target, update `local.link_paths`, save `local.toml`, and proceed with linking.
3. If not, skip as before (don't silently back up real files without user confirmation).

**Why this is safe:** We only auto-register paths that are already roost-managed symlinks. This handles the case where a user manually re-cloned or restored symlinks outside of roost.

---

## Phase 3: Critical — No `ensure_links()` After Sync

### Commit 3.1: `fix(sync): call ensure_links after successful sync`
**Files:** `src/main.rs`, `src/tui/main_view/mod.rs`

**Problem:** Neither CLI nor TUI calls `linker::ensure_links()` after `git::sync()`. New apps from remote are in `roost.toml` but have no symlinks locally.

**Fix:** After `git::sync()` returns `Ok(...)`:
1. Reload configs (shared may have changed).
2. Call `linker::ensure_links(&shared, &local, &roost_dir)?`.
3. Report any `actions` (linked, skipped, backed up) to the user/TUI status line.

**TUI specific:** `tui/main_view/mod.rs:215-228` currently discards `SyncResult`. Fix to:
1. Capture `SyncResult`.
2. Reload `shared` and `local` into `state`.
3. Run `ensure_links()`.
4. Set `state.status_message` with summary: e.g. "Sync complete. 3 new apps linked."

---

## Phase 4: High — Tilde-Path Serialization for `primary_config`

### Commit 4.1: `feat(app): add tilde-path serde for primary_config and link_paths`
**Files:** `src/app/mod.rs`, `src/app/tests.rs`

**Problem:** `primary_config: Option<PathBuf>` in shared config serializes as absolute path (e.g. `/Users/alice/.config/nvim/init.lua`), meaningless on other devices.

**Design:** Implement custom serde using `serialize_with` / `deserialize_with`:
- Serialize: Convert home-dir prefix to `~/...`.
- Deserialize: Expand `~/...` to current home dir at runtime.
- Apply to `Application::primary_config`.
- Also apply to `LocalAppConfig::link_paths` values (optional — these are local-only, but nice for portability).

**Helper functions:**
```rust
fn serialize_tilde<S>(path: &Option<PathBuf>, serializer: S) -> Result<S::Ok, S::Error>
fn deserialize_tilde<'de, D>(deserializer: D) -> Result<Option<PathBuf>, D::Error>
```

**Unit tests:** Serialize `/Users/test/.config/nvim/init.lua` → `"~/.config/nvim/init.lua"`. Deserialize `"~/.config/nvim/init.lua"` → `/Users/test/.config/nvim/init.lua`.

---

## Phase 5: High — Structural Merge Must Handle Deletions & All Fields

### Commit 5.1: `fix(git): detect deletions in structural merge and reconcile all app fields`
**Files:** `src/git/mod.rs`, `src/git/tests.rs`

**Problem:** Merge is purely additive. Removing an app from a profile or removing an ignore pattern is silently lost. Only `is_dir` conflict is detected.

**Design:**
1. **App-level merge:** Beyond `is_dir`, also reconcile:
   - `primary_config`: conflict if both sides changed; preference wins.
   - `on_profiles`: union (additive) is acceptable — removing from a profile is a profile-level change.
   - `ignore`: union (additive) is acceptable for now.
2. **Profile-level merge:** Detect apps removed from a profile:
   - Compare `local_profile.apps` vs `remote_profile.apps`.
   - If an app exists locally but not remotely → remove it (if `preference == Remote`), or keep it (if `preference == Local`).
3. **Ignore merge:** Detect patterns removed remotely:
   - If `preference == Remote`, replace local `ignored` with remote `ignored`.
   - If `preference == Local`, keep local `ignored`.
   - For now, additive-only is safer; changing this is a behavior change that needs a flag.

**Actually, the safest fix:** Make the structural merge **overwrite** remote config fields when `preference == Remote`, and **preserve** local fields when `preference == Local`. Currently it's always "union", which is the bug.

---

## Phase 6: High — Fix Rebase Error Swallowing

### Commit 6.1: `fix(git): propagate rebase --continue errors and avoid false Clean results`
**Files:** `src/git/mod.rs`

**Problem:** `git/mod.rs:246-250` uses `let _ = run_git(...)` for `checkout --theirs`, `add`, and `rebase --continue`. If continue fails, repo is left in mid-rebase with no recovery. Local-preference branch can return `SyncResult::Clean` even if `get_conflict_files()` errored.

**Fix:**
1. In `Remote` preference branch:
   - Check `rebase --continue` result.
   - If it fails, check if still in rebase state (`git status --porcelain` or `.git/rebase-apply` exists).
   - If still in rebase, abort and return an error: "Rebase failed. Manual resolution required."
2. In `Local` preference branch:
   - If `get_conflict_files()` errors, don't assume clean. Return the original rebase error.
   - After `rebase --abort`, check `get_conflict_files()` again; if still error, return the error.

---

## Phase 7: High — Integration Tests for Sync

### Commit 7.1: `test: add integration tests for sync command`
**Files:** `tests/sync.rs`

**Scenarios to test:**
1. Sync with no remote → error.
2. Sync when local == remote → `SyncResult::Clean`.
3. Sync with remote app additions → new apps appear, symlinks created.
4. Sync with remote app deletions (Remote preference) → app removed locally.
5. Sync with conflicting `is_dir` → `SyncResult::ConfigConflict`.
6. Sync with dirty working tree → auto-save, then sync.
7. Verify `git push` occurred (check remote repo state).

**Test helper:** Create a second temp dir as "remote" bare repo, set it as origin, push initial state, then simulate changes from another clone.

---

## Phase 8: Medium — Concurrency Protection & No-Op Migration

### Commit 8.1: `fix(app): atomic config writes via temp-file-then-rename`
**Files:** `src/app/mod.rs`, `src/gitignore.rs`

**Problem:** `std::fs::write()` directly to config path. Crash mid-write → corrupted TOML.

**Fix:** Write to `path.tmp`, then `fs::rename(path.tmp, path)` (atomic on POSIX and Windows NTFS).

### Commit 8.2: `feat(app): implement migrate_shared() for dual-format apps and link_path`
**Files:** `src/app/mod.rs`

**Problem:** `migrate_shared()` is a no-op. If config format changes, older clients fail.

**Fix:** Implement actual migration:
1. Detect old `apps` list format (`Vec<OldApp>`) and convert to `BTreeMap<String, Application>`.
2. Detect old `link_path: Option<PathBuf>` and convert to `link_paths: BTreeMap<String, PathBuf>`.

---

## Phase 9: Medium — TUI Post-Sync Reload

### Commit 9.1: `fix(tui): reload configs and refresh state after sync`
**Files:** `src/tui/main_view/mod.rs`, `src/tui/main_view/state.rs`

**Problem:** After sync, TUI continues with stale in-memory config. New apps/profiles don't appear until restart.

**Fix:** After `git::sync()` returns, re-read `roost.toml` and `local.toml` into `state.shared` and `state.local`, then update panel lists.

---

## Execution Order

The recommended order balances risk and testability:

1. **1.1** Add `git push` (simple, high impact)
2. **3.1** `ensure_links` after sync (simple, pairs with 1.1)
3. **4.1** Tilde-path serde (self-contained, well-testable)
4. **6.1** Rebase error handling (safety, prevents broken repos)
5. **7.1** Integration tests for sync (validates 1.1, 3.1, 6.1)
6. **2.1** Init reconstruction from existing `roost.toml` (complex, requires user interaction design)
7. **2.2** `link_paths` auto-rebuild from existing symlinks (pairs with 2.1)
8. **5.1** Structural merge improvements (risky behavior change, needs tests)
9. **8.1** Atomic writes (infrastructure, low risk)
10. **8.2** Config migration (only needed if we change format — can be deferred)
11. **9.1** TUI post-sync reload (UI polish, lower priority than backend fixes)

---

## Open Questions

1. For init reconstruction, should we attempt to auto-discover paths from common conventions (e.g. `~/.config/<app>`), or always prompt? Auto-discovery is friendlier but may guess wrong.
2. For structural merge deletions: is `Remote` preference a safe default for all users, or should we add a CLI flag `--preference` (as noted in Low Severity)?
3. Should `link_paths` auto-rebuild on `ensure_links` for *all* missing apps, or only when reconstructing from an existing `roost.toml`? Silent path changes are dangerous.

## Risks

- **5.1 (Structural merge)** is the riskiest change. Changing from additive to overwrite-on-preference could surprise users who rely on the current "keep everything" behavior. Consider adding a `--merge-strategy` flag or keeping additive as default with a warning.
- **2.1 (Init reconstruction)** requires careful prompting to not be annoying. A user with 20 apps doesn't want 20 prompts. Batch confirmation ("Auto-discovered 5 paths. Use them?") is better than per-app prompting.
- **1.1 (git push)** could fail if the remote requires authentication or has branch protection. Error handling must surface this clearly.
