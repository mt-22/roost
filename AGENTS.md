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
  init_tui.rs          -- Onboarding TUI (monolithic, 819 lines)
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

  cli.rs               -- Clap CLI definitions
  logo.rs              -- ASCII art constant
  os_detect.rs         -- Compile-time OS detection
  pager.rs             -- External pager wrapper ($PAGER / less -R)
  gitignore.rs         -- .gitignore regeneration with managed blocks

tests/                  -- Integration tests (~62 tests across 14 files)
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

**Test targets:** ~99+ tests total (~64 unit + ~62 integration). Most CLI commands now have integration test coverage.

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
| Suspend/resume | **Partial** | `tui/suspend.rs` implemented with tests. Needs wiring into main TUI |
| Main TUI skeleton | **In progress** | `tui/main_view/state.rs` complete; `event.rs`/`ui.rs`/`mod.rs` stubbed |
| Onboarding TUI | **Functional** | init_tui.rs works for roost init; has search, miller, confirm, signal handling |
| Miller columns | **Solid** | Reusable widget with 10 unit tests |
| Fuzzy search | **Solid** | Reusable FuzzyEngine with tests |
| Confirm dialog | **Solid** | Reusable yes/no dialog |
| Symlink ops | **Solid** | ingest, restore, unlink, switch, import, copy all tested |
| Config validation | **Solid** | Cycle detection, unknown app/profile checks |
| Ignore system | **Solid** | Global + per-app patterns, .gitignore regeneration |

### What's Missing or Incomplete

| Area | Priority | Gap |
|------|----------|-----|
| **Main TUI** | **P0** | `tui/main_view/` skeleton in place (`state.rs` complete; `event.rs`/`ui.rs`/`mod.rs` stubbed). Not yet wired to `main.rs` no-arg launch |
| **Dialog system** | **P0** | Only `ConfirmDialog` exists. Missing: Help, Ignore, Profile, Git Log, Undo, App Link, Diff View |
| **Suspend/resume** | **P1** | `tui/suspend.rs` helper implemented. Not yet wired into main TUI event handlers |
| **Tilde-path serde** | **P1** | SPEC requires custom serde to serialize `~/...` in shared config. Currently uses plain PathBuf serde |
| **Config migration** | **P2** | `migrate_shared()` is a no-op stub. Needs dual-format `apps` and `link_path` -> `link_paths` handling |
| **git push in sync** | **P2** | `sync()` fetches and rebases but never pushes. SPEC requires push |
| **Integration tests** | **P2** | Missing: sync, init. Done: diff, ignore, restore, rollback, adopt, list, save, where --profile |
| **File preview** | **P2** | Miller columns show filenames only; SPEC wants inline content preview for files |

### Known Issues / Fragile Areas

| Issue | Status | Fix Needed |
|-------|--------|------------|
| No concurrency protection on config files | **Open** | File locking or atomic read-modify-write |
| `git pull --rebase` failures | **Partial** | `sync()` surfaces conflicts but aborts rebase rather than prompting user |
| Dialog states flat `Option<T>` on giant struct | **In use in Main TUI** | Consider state machine enum for type safety in future refactor |
| `Esc` in onboarding without confirmation | **Fixed** | Already has discard confirmation |
| Cross-profile symlink cycles | **Fixed** | `validate_shared()` detects cycles on config load |
| `ensure_links` / `switch_links` error swallowing | **Fixed** | Both propagate errors properly |
| Temp backup clobbering | **Fixed** | Backups go to `.backups/` inside roost dir |
| Git identity missing in test helpers | **Fixed** | All `setup_roost` helpers and `git::tests.rs` now set `user.name`/`user.email` |

---

## Work Streams

This is the ordered breakdown of remaining work to reach SPEC compliance:

### Stream 1: Suspend/Resume Infrastructure
Generic helper to leave alternate screen, run external command, restore TUI.

**Status:** `tui/suspend.rs` implemented with tests. Needs wiring into main TUI event handlers.

---

### Stream 2: Main TUI Core (`tui/main_view/`)
Build the primary daily interface launched by `roost` with no args.

**Files created (skeleton):**
- `src/tui/main_view/mod.rs` — Entry point and run loop (stub)
- `src/tui/main_view/state.rs` — State machine complete (MainViewState, Focus, SearchState, helper methods)
- `src/tui/main_view/event.rs` — Key dispatch, dialog routing, Action enum (stub)
- `src/tui/main_view/ui.rs` — Rendering (stub)

**Remaining work:**
- Wire event.rs key handlers for base panel navigation and actions
- Implement ui.rs rendering for header, panels, status bar, dialogs
- Wire mod.rs run loop with terminal setup/restore, panic hook, Ctrl+C handler
- Connect to `main.rs` no-arg launch
- Action processing (pending_auto_commit batching, suspend/resume integration)

**Key behaviors:**
- Header: `roost · profile: <name>  N apps managed`
- Left panel: App list with `★` primary marker, `←source` for linked apps
- Right panel: Miller columns for file browsing with inline file preview
- Focus switching between panels with `Tab`
- Action keys: `o` open, `x` remove, `f` import, `m` paste, `e/Enter` edit, `p` set primary, `s` sync, `a` add, `i` ignore, `P` profile, `g` git log, `d` diff, `u` undo

### Stream 3: Dialog System (`tui/main_view/dialogs/`)
Implement the 7 missing dialog overlays on top of Main TUI core.

**Files to create:**
- `src/tui/main_view/dialogs/mod.rs` — Re-exports (placeholder states exist)
- `src/tui/main_view/dialogs/help.rs` — Searchable keybind reference
- `src/tui/main_view/dialogs/ignore.rs` — Add/remove ignore patterns
- `src/tui/main_view/dialogs/profile.rs` — Switch/create/delete profiles
- `src/tui/main_view/dialogs/git_log.rs` — Git history browser + rollback
- `src/tui/main_view/dialogs/undo.rs` — Undo confirmation
- `src/tui/main_view/dialogs/app_link.rs` — Import/paste multi-step wizard
- `src/tui/main_view/dialogs/diff_view.rs` — Diff viewer state

### Stream 4: Backend Hardening
Fix remaining backend gaps that affect both CLI and TUI.

**Tasks:**
1. **Tilde-path serde module** — Custom serde for `PathBuf` that serializes as `~/...` and deserializes using current home
2. **Config migration** — Implement `migrate_shared()` for dual-format `apps` and `link_path` -> `link_paths`
3. **git push in sync** — Add `git push origin main` after successful rebase in `git::sync()`
4. **Concurrency protection** — Atomic config writes via temp file + rename
5. **File preview** — Inline content preview for files in Miller columns

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
- ⬜ `tests/sync.rs` — sync command (critical gap, also tests git push)
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
9. ✅ `init_tui.rs` — Onboarding TUI (functional, monolithic is acceptable)
10. ⬜ `tui/main_view/` + `dialogs/` — **Biggest remaining piece**
11. 🔄 Integration tests alongside each layer

**Recommended next sequence:**
1. ✅ **Suspend/resume infrastructure** — `tui/suspend.rs` done
2. 🔄 **Main TUI core** (Stream 2) — `state.rs` done; need `event.rs`, `ui.rs`, `mod.rs` run loop, `main.rs` wiring
3. ⬜ **Dialog system** (Stream 3) — Layered on top of main TUI core
4. ⬜ **Backend hardening** (Stream 4) — Smaller focused fixes, can parallelize with 2/3
5. ⬜ **Test coverage** (Stream 5) — Fill gaps after features stabilize

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
