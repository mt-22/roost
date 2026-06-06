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
    tests.rs           -- Unit tests (13 tests)

  linker/
    mod.rs             -- Symlink operations (ingest, restore, unlink, ensure_links, switch, import, copy)
    tests.rs           -- Unit tests (19 tests)

  scanner/
    mod.rs             -- App discovery, confidence scoring, multi-source scanning
    tests.rs           -- Unit tests (19 tests)

  git/
    mod.rs             -- Git CLI wrappers (commit, sync, log, diff, undo, rollback, read_shared_at, safe_rollback)
    tests.rs           -- Unit tests (11 tests)

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

**Test targets:** ~184 tests total (~112 unit + ~71 integration + 1 doctest). All CLI commands have integration test coverage except init. Rollback and sync tests included.

---

## UI Style (Preserve Verbatim)

- **Color palette:** Cyan (focused borders), Yellow (key labels, dialog borders), DarkGray (unfocused, hints), Red (destructive confirms), Green (affirmative, active profile), White (bold highlights)
- **Highlight:** `bg(DarkGray) + bold` for cursor
- **Status bar:** Yellow keys, e.g. `j/k nav  Tab focus  / search  ...`
- **Miller columns:** Responsive: 3 equal thirds ≥100w, 2-col ≥55w, vertical stack < 55w
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
| Miller columns | **Solid** | Reusable widget with 10 unit tests, 3 responsive modes |
| Fuzzy search | **Solid** | Reusable FuzzyEngine with tests |
| Confirm dialog | **Solid** | Reusable yes/no dialog |
| Symlink ops | **Solid** | ingest, restore, unlink, switch, import, copy all tested |
| Rooster animation | **Solid** | Hardcoded braille-dot pixel grid with pecking state; `RoosterState` tick system |
| Config validation | **Solid** | Cycle detection, unknown app/profile checks |
| Ignore system | **Solid** | Global + per-app patterns, .gitignore regeneration |

### Release-Prep Branch Accomplishments

All tasks completed on `release-prep` branch (10 commits beyond base):

| Fix | Description |
|-----|-------------|
| **Fuzzy search** | Query sync with FuzzyEngine, filter persistence, j/k routing through engine, filtered rendering |
| **RemoveApp action** | Mirrors CLI `cmd_remove` — unlink symlinks, remove from configs, save atomically, auto-commit |
| **Profile switch** | Calls `linker::switch_profile()` so symlinks update on disk, not just local.toml |
| **Source marker** | `←` rendered for cross-profile linked apps (was dead code) |
| **Help text** | `s`=Save, `S`=Sync, `r` only in git log, panel-specific keys documented |
| **Hash slicing** | `&hash[..7]` replaced with `hash[..hash.len().min(7)]` in 3 locations |
| **Status message** | No longer cleared on every keypress |
| **Atomic writes** | Concurrency-safe via unique temp filenames (PID + ms timestamp) in `app/mod.rs` and `gitignore.rs` |
| **Path traversal** | `validate_path_in_home()` helper with 3 unit tests in `linker/mod.rs` |
| **App name sanitization** | `sanitize_app_name()` replaces `/`, `\`, `\0` with `_`; applied in `cmd_add`, `init.rs`, TUI add-app |
| **Profile deletion confirm** | y/n confirm before deleting profile; fixed render z-order so confirm shows on top |
| **Panic hook / Ctrl-C** | Consolidated into `tui::init()` with `OnceLock`, 3 call sites unified |
| **Ignore pattern confirm** | y/n confirm before removing ignore pattern |
| **j/k in text input** | `j`/`k` no longer consumed for navigation in ignore Add and profile Create modes |
| **Rooster pecking animation** | Replaced algorithmic shift with hardcoded `ROOSTER_PECK` pixel grid for natural head-tilt bend |

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
| `git reset --hard` in rollback destroys new apps | **Fixed** | `safe_rollback()` uses selective `git checkout` per app, preserving apps not at the target commit |

---

## v0.2.0 Release Checklist

### Release Blockers (Must Fix)

| # | Task | Status | Notes |
|---|------|--------|-------|
| 1 | **Profile deletion confirm** | ✅ | y/n confirm before deleting profile |
| 2 | **Panic hook leak** | ✅ | Consolidated into `tui::init()` via `OnceLock` |
| 3 | **Ctrl-C handler leak** | ✅ | Same — consolidated into `tui::init()` |
| 4 | **Ignore pattern removal confirm** | ✅ | y/n confirm before removing pattern |
| 5 | **j/k typing in dialogs** | ✅ | `j`/`k` no longer consumed for nav in text input modes |
| 6 | **README** | ⬜ | Needs install, usage, config docs |
| 7 | **LICENSE** | ⬜ | Needs license file |
| 8 | **Cargo.toml metadata** | ⬜ | description, authors, repository, keywords, categories |
| 9 | **CHANGELOG** | ⬜ | Document v0.2.0 changes since v0.1.0 |

### Quality Improvements (Should Fix)

| # | Task | Status | Notes |
|---|------|--------|-------|
| 9 | **File preview limit** | ⬜ | Miller file preview reads full file; should cap at N lines (e.g. 100) |
| 10 | **Unicode display width** | ⬜ | Search/miller may misalign multi-byte characters |
| 11 | **Status bar overflow** | ⬜ | Long status messages overflow the bar width |
| 12 | **init.rs integration test** | ⬜ | Missing TUI/non-TUI init test coverage |
| 13 | **Git pull --rebase UX** | ⬜ | Sync surfaces conflicts but aborts rebase rather than prompting user |

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
