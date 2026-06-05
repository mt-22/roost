# AGENTS.md — Roost

## Project Overview

**Roost** is a terminal-based dotfile manager written in Rust. It moves application config files into a central `~/.roost/` directory organized by device-specific **profiles**, creates **symlinks** back to original locations, and uses git for cross-device sync.

- **Version:** 0.2.0
- **Edition:** Rust 2024, MSRV 1.85+
- **Binary:** Single binary, no runtime assets (data embedded via `include_str!`)
- **No async** — blocking I/O, suspend TUI for external tools

---

## Architecture

### Module Map

```
src/
  main.rs              -- CLI entry, dispatches subcommands OR launches TUI
  lib.rs               -- Re-exports all modules (for integration tests)

  app/
    mod.rs             -- Data models + config load/save (SharedAppConfig, LocalAppConfig)
    tests.rs           -- Unit tests (8 tests)

  linker/
    mod.rs             -- Symlink operations (ingest, restore, unlink, ensure_links, switch, import, copy)
    tests.rs           -- Unit tests (16 tests)

  scanner/
    mod.rs             -- App discovery, confidence scoring, multi-source scanning
    tests.rs           -- Unit tests (19 tests)

  git/
    mod.rs             -- Git CLI wrappers (commit, sync, log, diff, undo, rollback)
    tests.rs           -- Unit tests (9 tests)

  init.rs              -- roost init wizard (dialoguer-based prompts + onboarding TUI)
  app_selector.rs      -- App selection TUI (onboarding + add-app, monolithic, 819 lines)
  miller.rs            -- Miller column browser widget (585 lines, 10 tests)

  tui/
    mod.rs             -- Re-exports confirm, search, suspend, main_view
    confirm.rs         -- Yes/No confirmation dialog
    search/mod.rs      -- Fuzzy search engine (FuzzyEngine, 328 lines)
    suspend.rs         -- Generic suspend/resume helper for external tools
    main_view/
      mod.rs           -- Main TUI entry point and run loop
      state.rs         -- MainViewState, Focus, SearchState
      event.rs         -- Key dispatch, dialog routing, Action enum
      ui.rs            -- Rendering: header, panels, status bar, dialogs
      dialogs/
        mod.rs         -- Re-exports for dialog state types
        help.rs        -- Searchable keybind reference
        profile.rs     -- Switch/create/delete profiles (3-mode, Tab cycle)
        ignore.rs      -- Add/remove ignore patterns (2-mode, Tab cycle)
        git_log.rs     -- Git history browser + rollback trigger
        undo.rs        -- Undo confirmation dialog
        app_link.rs    -- Import/paste multi-step wizard
        diff_view.rs   -- Inline scrollable diff viewer

  cli.rs               -- Clap CLI definitions
  logo.rs              -- ASCII art constant
  os_detect.rs         -- Compile-time OS detection
  pager.rs             -- External pager wrapper ($PAGER / less -R)
  gitignore.rs         -- .gitignore regeneration with managed blocks

tests/                  -- Integration tests (~62 tests across 14 files)
  sync.rs               -- Sync integration tests (no init, no remote, up-to-date, pull, push, conflict)
```

### Data Models

**`SharedAppConfig`** (`roost.toml`, git-tracked):
- `remote: Option<String>`
- `profiles: BTreeMap<String, Profile>`
- `apps: BTreeMap<String, Application>`
- `ignored: BTreeSet<String>`

**`LocalAppConfig`** (`local.toml`, git-ignored):
- `active_profile: String`
- `os_info: OsInfo`
- `link_paths: BTreeMap<String, PathBuf>`

**`Application`**:
- `primary_config: Option<PathBuf>`
- `on_profiles: BTreeSet<String>`
- `is_dir: bool`
- `ignore: Vec<String>`

**`Profile`**:
- `apps: BTreeSet<String>`
- `app_sources: BTreeMap<String, String>` (app -> source_profile for cross-profile symlinks)

### Key Design Decisions

1. **Two-config split:** `roost.toml` shared via git, `local.toml` per-device gitignored.
2. **Same backend for CLI and TUI:** All logic in `app/`, `linker/`, `scanner/`, `git/`.
3. **Cross-platform symlinks:** `#[cfg]` branches for Unix/Windows.
4. **Suspend/resume for external tools:** TUI must drop to regular terminal for `$EDITOR`, `$PAGER`, `git`.
5. **Auto-commit batching:** State mutations set `pending_auto_commit`, processed after event loop.

---

## Build & Test

```bash
# Full test suite
cargo test

# Release build
cargo build --release

# Environment override
ROOST_DIR=/tmp/test-roost cargo run -- init
```

**Dev dependencies:** `assert_cmd`, `predicates`, `tempfile`
**Runtime external:** `git` CLI, `$EDITOR` (default `vi`), `$PAGER` (default `less`)

**Test targets:** ~112+ tests total (~68 unit + ~62 integration). Most CLI commands now have integration test coverage. Sync integration tests added.

---

## UI Style (Preserve Verbatim)

- **Color palette:** Cyan (focused borders), Yellow (key labels, dialog borders), DarkGray (unfocused, hints), Red (destructive confirms), Green (affirmative, active profile), White (bold highlights)
- **Highlight:** `bg(DarkGray) + bold` for cursor
- **Status bar:** Yellow keys, e.g. `j/k nav  Tab focus  / search  ...`
- **Miller columns:** 3 equal thirds (Parent | Current | Preview)
- **Symbols:** `★` primary, `»` cursor, `←` sourced, `✓` selected, `●` bullet
- **Dialog overlays:** Centered, bordered block with `Clear` widget behind

### Key Bindings (Complete Map)

**Base:** `j/k` nav, `Tab` focus, `h/l` miller, `/` search, `?` help, `q/Esc` quit
**Apps panel:** `o` open primary, `x` remove, `f` import-from, `m` paste-into
**Files panel:** `e/Enter` edit, `p` set primary
**Actions:** `s` sync, `a` add app, `i` ignore, `P` profile, `g` git log, `d` diff, `u` undo
**Dialogs:** `y/n` confirm, `Tab` cycle modes, `Esc` cancel, `j/k` navigate, typing for search

---

## Current State Assessment

### What's Fully Working

| Area | Status | Notes |
|------|--------|-------|
| Backend modules | **Strong** | app/, linker/, scanner/, git/ are well-tested and functional |
| CLI commands | **Complete** | All 15+ subcommands implemented and wired |
| Suspend/resume | **Done** | `tui/suspend.rs` implemented, wired into main TUI for $EDITOR/$PAGER/git |
| Main TUI | **Done** | Full event loop, rendering, panel navigation, search, actions wired to main.rs |
| Onboarding TUI | **Functional** | app_selector.rs works for roost init and add-app; has search, miller, confirm, signal handling |
| Miller columns | **Solid** | Reusable widget with 10 unit tests |
| Fuzzy search | **Solid** | Reusable FuzzyEngine with tests |
| Confirm dialog | **Solid** | Reusable yes/no dialog |
| Symlink ops | **Solid** | ingest, restore, unlink, switch, import, copy all tested |
| Config validation | **Solid** | Cycle detection, unknown app/profile checks |
| Ignore system | **Solid** | Global + per-app patterns, .gitignore regeneration |

### What's Missing or Incomplete

| Area | Priority | Gap |
|------|----------|-----|
| **File preview in Miller columns** | **Done** | Inline text preview + binary indicator. Responsive layout (drops parent column below 100 width). |
| **Git Log UX improvements** | **Done** | `r` rollback documented in help, footer hint in dialog, stronger confirm warning. |
| **Terminal size enforcement** | **Done** | Minimum 40x12 with graceful too-small message. Narrow-width panic fixed via saturating_sub. |
| **Responsive miller columns** | **Done** | Drops parent column below 100 width, shows current dir name in header. |
| **Add App dialog** | **Done** | Reuses `app_selector.rs` for full adoption TUI; `auto_select=false` from main TUI. |
| **Primary config highlight** | **Done** | `★` marker in Miller columns on primary config file; cursor auto-focuses on it. |
| **Restore from Git Log** | **P1** | Git Log dialog (`g`) can rollback (`r`), but cannot restore individual files or apps from a past commit. Need per-commit restore action. |
| **Tilde-path serde** | **Done** | Custom serde for `PathBuf` that serializes as `~/...` and deserializes using current home. Applied to `Application::primary_config`. |
| **Config migration** | **P2** | `migrate_shared()` is a no-op stub. Needs dual-format `apps` and `link_path` -> `link_paths` handling |
| **git push in sync** | **Done** | `sync()` now pushes to `origin main` after successful rebase. Tested in integration tests. |
| **Concurrency protection** | **Done** | Config writes use atomic temp-file-then-rename pattern in `save_shared()`, `save_local()`, and `gitignore::regenerate()`. |
| **Integration tests** | **P2** | Missing: init. Done: diff, ignore, restore, rollback, adopt, list, save, where --profile, **sync**. |
| **Init reconstruction** | **Done** | When `roost.toml` exists but `local.toml` is missing, `roost init` now reconstructs `local.toml` by picking an existing profile and auto-discovering `link_paths` from common paths or existing symlinks. |
| **Structural merge** | **Done** | Merge now handles field-level conflicts (`primary_config`, `ignore`), profile/app deletion on `Remote` preference, and ignored-pattern replacement — not just `is_dir`. |
| **Rebase error handling** | **Done** | `rebase --continue` errors are propagated, not swallowed. `Local` preference no longer returns false `Clean` when `get_conflict_files()` returns empty. |

### Known Issues / Fragile Areas

| Issue | Status | Fix Needed |
|-------|--------|------------|
| No concurrency protection on config files | **Fixed** | Atomic writes via temp file + rename in `save_shared()`, `save_local()`, `gitignore::regenerate()` |
| `git pull --rebase` failures | **Partial** | `sync()` surfaces conflicts but aborts rebase rather than prompting user |
| Dialog states flat `Option<T>` on giant struct | **In use in Main TUI** | Consider state machine enum for type safety in future refactor |
| Narrow terminal panic | **Fixed** | `saturating_sub` used for all width calculations; min size enforcement with graceful message |
| `Esc` in onboarding without confirmation | **Fixed** | Already has discard confirmation |
| Cross-profile symlink cycles | **Fixed** | `validate_shared()` detects cycles on config load |
| `ensure_links` / `switch_links` error swallowing | **Fixed** | Both propagate errors properly |
| Temp backup clobbering | **Fixed** | Backups go to `.backups/` inside roost dir |
| Git identity missing in test helpers | **Fixed** | All `setup_roost` helpers and `git::tests.rs` now set `user.name`/`user.email` |
| Rebase --continue errors swallowed | **Fixed** | `git::sync()` now propagates `rebase --continue` failures and avoids false `SyncResult::Clean` |
| Structural merge ignores most fields | **Fixed** | Merge now reconciles `primary_config`, `ignore`, profile/app deletions, and ignored patterns — not just `is_dir` |
| No `ensure_links()` after sync | **Fixed** | Both CLI and TUI call `linker::ensure_links()` after successful sync |

---

## Work Streams

This is the ordered breakdown of remaining work to reach SPEC compliance:

### Stream 1: Suspend/Resume Infrastructure
Generic helper to leave alternate screen, run external command, restore TUI.

**Status:** ✅ `tui/suspend.rs` implemented with tests. Wired into main TUI event handlers for `$EDITOR`, `$PAGER`, and `git`.

---

### Stream 2: Main TUI Core (`tui/main_view/`)
Build the primary daily interface launched by `roost` with no args.

**Status:** ✅ Complete.

**Files:**
- `src/tui/main_view/mod.rs` — Entry point, run loop, action processing, suspend/resume integration
- `src/tui/main_view/state.rs` — MainViewState with panels, dialog stack, pending_auto_commit
- `src/tui/main_view/event.rs` — Key dispatch, dialog routing, Action enum
- `src/tui/main_view/ui.rs` — Rendering: header, apps panel, miller files panel, status bar, all dialogs

**Key behaviors:**
- Header: `roost · profile: <name>  N apps managed`
- Left panel: App list with `★` primary marker, `←source` for linked apps
- Right panel: Miller columns for file browsing (no outer box, just 'Files' header)
- Focus switching between panels with `Tab`; `h/l` navigate in/out of miller
- Action keys: `o` open, `x` remove, `f` import, `m` paste, `e/Enter` edit, `p` set primary, `s` sync, `a` add, `i` ignore, `P` profile, `g` git log, `d` diff, `u` undo

### Stream 3: Dialog System (`tui/main_view/dialogs/`)
Implement the 7 dialog overlays on top of Main TUI core.

**Status:** ✅ All 7 dialogs implemented and wired.

**Files:**
- `src/tui/main_view/dialogs/mod.rs` — Re-exports
- `src/tui/main_view/dialogs/help.rs` — Searchable keybind reference, j/k nav
- `src/tui/main_view/dialogs/ignore.rs` — Add/remove ignore patterns, Tab cycles modes
- `src/tui/main_view/dialogs/profile.rs` — Switch/create/delete profiles, Tab cycles modes, visual mode hint
- `src/tui/main_view/dialogs/git_log.rs` — Git history browser, `r` triggers rollback confirm
- `src/tui/main_view/dialogs/undo.rs` — Undo confirmation dialog
- `src/tui/main_view/dialogs/app_link.rs` — Import/paste multi-step wizard
- `src/tui/main_view/dialogs/diff_view.rs` — Inline scrollable diff viewer with color coding

### Stream 4: Backend Hardening
Fix remaining backend gaps that affect both CLI and TUI.

**Tasks:**
1. **Tilde-path serde module** — Custom serde for `PathBuf` that serializes as `~/...` and deserializes using current home
2. **Config migration** — Implement `migrate_shared()` for dual-format `apps` and `link_path` -> `link_paths`
3. **git push in sync** — Add `git push origin main` after successful rebase in `git::sync()`
4. **Concurrency protection** — Atomic config writes via temp file + rename
5. **File preview** — Inline content preview for files in Miller columns

**Status:**
- ✅ Tilde-path serde — Done
- ⬜ Config migration — Still a no-op
- ✅ git push in sync — Done
- ✅ Concurrency protection — Done
- ✅ File preview — Done (pre-existing)

### Stream 5: Test Coverage
Fill in missing integration tests.

**Files to create / extend:**
- ✅ `tests/diff.rs` — diff command
- ✅ `tests/ignore.rs` — ignore command
- ✅ `tests/restore.rs` — restore command
- ✅ `tests/rollback.rs` — rollback command
- ✅ `tests/adopt.rs` — adopt command
- ✅ `tests/list.rs` — list command
- ✅ `tests/save.rs` — save command
- ✅ `tests/where.rs` — extend with `--profile` tests
- ✅ `tests/sync.rs` — sync command (6 tests: no init, no remote, up-to-date, pulls remote, pushes local, detects conflict)
- ⬜ `tests/init.rs` — init wizard + onboarding TUI (mock selection)

---

## Implementation Order Recommendation

The SPEC suggests this order for maximum testability. Current progress is through step 7:

1. ✅ `app/` — Data models + TOML load/save
2. ✅ `os_detect.rs`
3. ✅ `scanner/`
4. ✅ `linker/`
5. ✅ `git/`
6. ✅ `init.rs` + `logo.rs`
7. ✅ `main.rs` — CLI dispatch + all subcommand handlers
8. ✅ `tui/search/` — Fuzzy search (exists, functional)
9. ✅ `app_selector.rs` — App selection TUI (functional, monolithic is acceptable)
10. ⬜ `tui/main_view/` + `dialogs/` — **Biggest remaining piece**
11. 🔄 Integration tests alongside each layer

**Recommended next sequence (user-directed):**
1. ✅ **Streams 1-3** — Suspend/resume, Main TUI, Dialogs — all done
2. ✅ **File preview in Miller columns** — Inline content for files (first N lines), skip binary
3. ✅ **Git Log UX** — Rollback warning, footer hint, keybind documentation
4. ✅ **Responsive miller + Terminal size enforcement** — Narrow terminal fixes, min size enforcement
5. ✅ **Add App dialog** — Reuses `app_selector.rs` for full adoption TUI with multi-select
6. ✅ **Primary config highlight** — `★` marker in Miller columns on primary config file; cursor auto-focuses on it
7. ⬜ **Restore from Git Log** — Per-commit restore: checkout individual files or apps from a past commit
7. ✅ **Backend hardening** (Stream 4) — Tilde serde, git push, atomic writes, config migration (except migrate_shared)
8. ✅ **Test coverage** (Stream 5) — sync.rs done, init.rs still needed

---

## Coding Conventions

- **Error handling:** Use `color_eyre::Result` and `eyre::bail!` for errors.
- **Collections:** Prefer `BTreeMap`/`BTreeSet` over `HashMap`/`HashSet` for deterministic ordering.
- **Tests:** Unit tests co-located in `src/*/tests.rs`. Integration tests in `tests/*.rs` using `assert_cmd` subprocess pattern.
- **UI:** Follow the SPEC palette and symbols exactly. No deviation without user approval.
- **Architecture:** Elm-style for TUI: `State` + `handle_event(&mut State, KeyEvent) -> Action` + `render(&mut State, Frame)`. No retained widgets.
- **No async:** Blocking I/O throughout.
- **No secrets in git:** `local.toml` is gitignored by design.

---

## Dependency Notes

Already in `Cargo.toml`:
- `ratatui = "0.30"`, `crossterm = "0.29"`, `color-eyre = "0.6"`, `dialoguer = "0.12"`, `dirs = "6"`, `serde = "1"`, `toml = "0.8"`, `clap = "4"`, `time = "0.3"`
- Added: `clap_complete = "4"`, `ctrlc = "3"`

No new dependencies expected for the remaining work.

---

## Environment Notes

- Rust/Cargo installed at `~/.cargo/bin/` but not always on PATH. Use `~/.cargo/bin/cargo` or source `~/.cargo/env` if needed.
- `ROOST_DIR` env var overrides the default `~/.roost` directory.
- Integration tests use `tempfile` to create isolated roost directories.
