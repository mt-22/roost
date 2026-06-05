# Multi-Device Sync Gap Tracker

## Critical (sync fundamentally broken without these)

- [ ] **No `git push` after sync** — `git/mod.rs:148-289` — `sync()` fetches and rebases but never pushes. Changes never leave the local machine. Sync is pull-only.
- [ ] **No `local.toml` reconstruction on new device** — `init.rs:98-102` — Fresh clone of roost repo has no `local.toml` (gitignored). No `roost init --existing` workflow. User must re-run full init wizard and re-select every app.
- [ ] **`link_paths` must be rebuilt per device** — `app/mod.rs:25`, `linker/mod.rs:148-154` — `ensure_links()` skips apps without `link_paths`. Even after git pull, new device can't create symlinks because `link_paths` is empty.
- [ ] **No `ensure_links()` call after sync** — `main.rs:405-424`, `tui/main_view/mod.rs:215-228` — Neither CLI nor TUI calls `ensure_links()` after sync. New apps from remote don't get symlinks created locally.

## High Severity

- [ ] **`primary_config` stores absolute paths in shared config** — `app/mod.rs:38` — `primary_config: Option<PathBuf>` serializes as absolute path (e.g. `/Users/alice/.config/nvim/init.lua`), meaningless on other devices. Needs tilde-path serde (`~/...`).
- [ ] **Structural merge ignores most fields** — `git/mod.rs:176-219` — Merge is purely additive. Deletions (removing an app from a profile, removing an ignore pattern) are silently lost. Only `is_dir` conflict is detected; `primary_config`, `on_profiles`, per-app `ignore` are not reconciled.
- [ ] **Swallowed errors in rebase continuation** — `git/mod.rs:246-261` — Remote-preference branch uses `let _ = run_git(..., "rebase", "--continue")`. If continue fails (remaining conflicts), repo is left in mid-rebase state with no recovery. Local-preference branch can return false `SyncResult::Clean` if `get_conflict_files()` returns empty on error.
- [ ] **No integration tests for sync** — `tests/` — Most complex feature has zero test coverage. Any of these gaps could silently regress.

## Medium Severity

- [ ] **Rebase-after-merge duplicates work** — `git/mod.rs:222-272` — Structural merge is committed, then rebase replays all local commits on top of `origin/main`. The merge commit + replayed commits create confusing history and unnecessary conflicts.
- [ ] **No recovery from interrupted rebase** — `git/mod.rs` (general) — No startup check for mid-rebase or mid-merge state. If sync is interrupted (crash, SIGKILL), subsequent operations fail until manually running `git rebase --abort`.
- [ ] **Config saves are non-atomic** — `app/mod.rs:76-79,88-91` — `save_shared()` and `save_local()` use `std::fs::write()` directly. Crash mid-write corrupts config. Should use temp-file-then-rename pattern. Same issue in `gitignore.rs:70`.
- [ ] **`migrate_shared()` is no-op** — `app/mod.rs:94-100` — Passes through raw TOML unchanged. If config format changes (e.g. `link_path` → `link_paths`, `apps` list → table), older clients pulling newer format will fail to deserialize.
- [ ] **TUI discards `SyncResult`** — `tui/main_view/mod.rs:219` — `let _ = git::sync(...)` discards both errors and conflict information. TUI always shows "Sync complete" regardless of outcome.
- [ ] **No post-sync config reload in TUI** — `tui/main_view/mod.rs:215-228` — After sync, TUI continues using stale in-memory config. New apps/profiles don't appear until restart.
- [ ] **Symlinks use absolute paths** — `linker/mod.rs:480-492` — `create_symlink()` uses absolute target paths. On a different device with a different home directory, all symlinks point to the wrong location.

## Low Severity

- [ ] **No conflict preference choice** — `main.rs:407`, `tui/main_view/mod.rs:219` — Both CLI and TUI hardcode `ConflictPreference::Local`. No `--preference` flag or dialog to choose local vs remote.
- [ ] **`.gitignore` not regenerated after rebase** — `git/mod.rs:252-272` — After rebase completes (which may change the final `roost.toml`), `.gitignore` is not regenerated. New ignore patterns from remote may not be applied.