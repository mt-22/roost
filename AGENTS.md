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
    tests.rs           -- Unit tests

  linker/
    mod.rs             -- Symlink operations (ingest, restore, unlink, ensure_links, switch, import, copy)
    tests.rs           -- Unit tests

  scanner/
    mod.rs             -- App discovery, confidence scoring, multi-source scanning
    tests.rs           -- Unit tests

  git/
    mod.rs             -- Git CLI wrappers (commit, sync, log, diff, undo, rollback, read_shared_at, safe_rollback)
    tests.rs           -- Unit tests

  init.rs              -- roost init wizard (dialoguer-based prompts + onboarding TUI)
  app_selector.rs      -- App selection TUI (onboarding + add-app, monolithic)
  miller.rs            -- Miller column browser widget (responsive 3/2/1 column modes)

  tui/
    mod.rs             -- Re-exports confirm, search, suspend, main_view
    confirm.rs         -- Yes/No confirmation dialog
    search/mod.rs      -- Fuzzy search engine (FuzzyEngine)
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
  logo.rs              -- ASCII art constant + random exit banner
  os_detect.rs         -- Compile-time OS detection
  pager.rs             -- External pager wrapper ($PAGER / less -R)
  gitignore.rs         -- .gitignore regeneration with managed blocks

tests/                  -- Integration tests (~71 tests across 14 files)
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
6. **Atomic writes:** Config files are written to a unique temp file then renamed for concurrency safety.
7. **Path validation:** `validate_path_in_home()` prevents ingest of paths outside `~` or containing `..`.

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
- **Miller columns:** Responsive: 3 equal thirds >=100w, 2-col >=55w, vertical stack < 55w
- **Symbols:** `*` primary, `>>` cursor, `<-` sourced, `check` selected, `bullet` bullet
- **Dialog overlays:** Centered, bordered block with `Clear` widget behind

### Key Bindings (Complete Map)

**Base:** `j/k` nav, `Tab` focus, `h/l` miller, `/` search, `?` help, `q` quit
**Apps panel:** `o` open primary, `x` remove, `f` import-from, `m` paste-into
**Files panel:** `e/Enter` edit, `p` set primary
**Actions:** `s` save, `S` sync, `a` add app, `i` ignore, `P` profile, `g` git log, `d` diff, `u` undo
**Dialogs:** `y/n` confirm, `Tab` cycle modes, `Esc` cancel, `j/k` navigate, typing for search

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
- `clap_complete = "4"`, `ctrlc = "3"`

---

## Environment Notes

- `ROOST_DIR` env var overrides the default `~/.roost` directory.
- Integration tests use `tempfile` to create isolated roost directories.
