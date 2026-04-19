# ROOST MVP Specification -- Rebuild

## What Roost Is

Roost is a **terminal-based dotfile manager**. It moves application config files (dotfiles) into a central directory (`~/.roost/`), organized by device-specific **profiles**, and creates **symlinks** back to original locations. Apps continue reading/writing at standard paths. The roost directory is a git repo for cross-device sync.

---

## What to Preserve Verbatim

These assets carry over from the existing project without modification:

### ASCII Art Logo (`logo.rs`)

```
                  ,.
                 (\(\)
 ,_              ;  o >
  {`-.          /  (_)
  `={\`-._____/`   |
   `-{ /    -=`\   |
    `={  -= = _/   /
       `\  .-'   /`               _ __ ___   ___  ___| |_
        {`-,__.'=                | '__/ _ \ / _ \/ __| __|
         ||                      | | | (_) | (_) \__ \ |_
         | \                     |_|  \___/ \___/|___/\__|
 --------/\/\---------------------------------------------
```

### Data Files

- `data/known_apps.txt` -- 120+ known app directory names (confidence 150)
- `data/known_dotfiles.txt` -- 66 known dotfile names in $HOME (confidence 200)

### UI Style (not code -- the *look*)

- Color palette: Cyan (focused borders), Yellow (key labels, dialog borders), DarkGray (unfocused, hints), Red (destructive confirms), Green (affirmative, active profile), White (bold highlights)
- Highlight style: `bg(DarkGray) + bold` for cursor
- Status bar format: `j/k nav  Tab focus  / search  ...` with Yellow keys
- Miller columns: 3 equal thirds (Parent | Current | Preview)
- Unicode symbols: `★` primary, `»` cursor, `←` sourced, `✓` selected, `●` bullet
- Dialog overlay pattern: centered, bordered block with `Clear` widget behind

### Key Bindings (complete map)

**Base:** `j/k` nav, `Tab` focus, `h/l` miller, `/` search, `?` help, `q/Esc` quit

**Apps panel:** `o` open primary, `x` remove, `f` import-from, `m` paste-into

**Files panel:** `e/Enter` edit, `p` set primary

**Actions:** `s` sync, `a` add app, `i` ignore, `P` profile, `g` git log, `d` diff, `u` undo

**Dialogs:** `y/n` confirm, `Tab` cycle modes, `Esc` cancel, `j/k` navigate, typing for search

---

## Architecture

### Module Structure

```
src/
  main.rs              -- CLI entry, dispatches subcommands OR launches TUI
  lib.rs               -- Re-exports all modules (for integration tests)

  app/
    mod.rs             -- Data models + config load/save (SharedAppConfig, LocalAppConfig)
    tests.rs           -- Unit tests

  linker/
    mod.rs             -- All symlink operations (ingest, restore, unlink, ensure_links, switch, import, copy)
    tests.rs

  scanner/
    mod.rs             -- App discovery, confidence scoring, file collection
    tests.rs

  git/
    mod.rs             -- Git CLI wrappers (commit, sync, log, diff, undo, rollback)
    tests.rs

  init.rs              -- `roost init` wizard (dialoguer-based CLI prompts)
  os_detect.rs         -- Runtime OS detection
  logo.rs              -- ASCII art constant
  pager.rs             -- External pager ($PAGER / less)

  tui/
    mod.rs             -- Onboarding TUI entry + run loop
    state.rs           -- Onboarding state (tabs, Miller, selections)
    event.rs           -- Onboarding key handler
    ui.rs              -- Onboarding rendering

    search/
      mod.rs           -- Fuzzy search (matching + overlay)
      tests.rs

    main_view/
      mod.rs           -- Main TUI entry + run loop + action handlers
      state.rs         -- MainViewTui state machine + all dialog states
      event.rs         -- Main TUI key dispatch + dialog routing
      ui.rs            -- Main TUI rendering (all panels + all dialogs)

      dialogs/
        mod.rs         -- Re-exports
        confirm.rs     -- Confirmation dialog state
        help.rs        -- Keybind reference with search
        ignore.rs      -- Add/remove ignore patterns
        profile.rs     -- Switch/create/delete profiles
        git_log.rs     -- Git history browser
        undo.rs        -- Undo confirmation
        app_link.rs    -- Import/paste multi-step wizard
        diff_view.rs   -- Diff viewer state

tests/                  -- Integration tests (one file per subcommand)
```

### Core Data Models

```
SharedAppConfig (roost.toml, git-tracked):
  remote: Option<String>
  profiles: HashMap<String, Profile>
  apps: HashMap<String, Application>
  ignored: HashSet<String>

LocalAppConfig (local.toml, git-ignored):
  active_profile: String
  os_info: OsInfo
  link_paths: HashMap<String, PathBuf>    // app -> original path on THIS device

Application:
  name: String
  primary_config: Option<PathBuf>         // tilde-relative in TOML
  on_profiles: Vec<String>

Profile:
  apps: HashSet<String>
  app_sources: HashMap<String, String>    // app -> source_profile (for cross-profile symlinks)
```

### Key Architecture Decisions to Rebuild

1. **Elm Architecture** for TUI: `State` + `handle_event(&mut State, KeyEvent) -> Action` + `render(&mut State, Frame)`. No retained widgets.

2. **Two-config split**: `roost.toml` shared via git, `local.toml` per-device gitignored.

3. **Tilde-relative paths** in shared config: serialize as `~/...`, deserialize to current device's home. Custom serde module.

4. **Symlink chains** for cross-profile sharing: `original -> roost/profile-A/app -> roost/profile-B/app`. Cycle detection required.

5. **Suspend/resume** for external tools: TUI drops out to regular terminal for `$EDITOR`, `$PAGER`, `git`, then re-inits.

6. **Priority-based dialog stack**: flat `Option<Dialog>` fields on state, checked in fixed order. Only one active at a time.

7. **Auto-commit batching**: state mutations set `pending_auto_commit: Option<String>`, processed after event loop iteration.

8. **All backend code shared** between CLI and TUI. CLI subcommands call the same `app::`, `linker::`, `git::`, `scanner::` functions.

---

## Feature List (MVP = all of these)

### CLI Subcommands (15)

| Command | Description |
|---------|-------------|
| `roost` (no args) | Launch main TUI |
| `roost init` | Interactive setup wizard (dialoguer prompts + onboarding TUI for app selection) |
| `roost add <path>` | Ingest a single path into active profile |
| `roost remove <app>` | Unlink symlinks, restore files, remove from config |
| `roost status` | Show all managed apps and symlink status (linked/broken/missing) |
| `roost sync` | Auto-commit + pull --rebase + push |
| `roost profile` | Subcommands: `list`, `switch <name>`, `add <name>`, `delete <name>`, `rename <old> <new>` |
| `roost diff` | Show uncommitted changes via git diff |
| `roost log` | Show recent commits (up to 20) |
| `roost undo [n]` | Hard-reset HEAD by n commits (default 1) |
| `roost rollback <hash>` | Hard-reset to specific commit |
| `roost restore <app>` | Restore app files to original locations (reverse of ingest) |
| `roost remote` | Show/set git remote URL |
| `roost doctor` | Diagnostics: broken symlinks, orphaned files, config inconsistencies |
| `roost adopt` | Register orphaned apps found in profile directories |
| `roost where <app>` | Show where app files live in roost directory |
| `roost help` | Print help text |

### TUI Screens

**Onboarding TUI** (used during `roost init` and `roost add`):

```
+-------------------------------------------------------------+
| roost setup                                                  |
| SourceTab1│SourceTab2│Browse...                              |
+---------------------------+---------------------------------+
|                           |                                 |
|  Source list OR            |   Managed (N)                  |
|  Miller columns            |   ● item1                      |
|  (70% width)              |   ● item2                      |
|                           |   ● item3                      |
|                           |   (30% width)                  |
+---------------------------+---------------------------------+
| j/k navigate  ␣ select  Tab next tab  / search  ...        |
+-------------------------------------------------------------+
```

- Tab-based source browsing (one tab per source: `~/.config`, `~/Library/Application Support`, `~/.local/bin`, `~/.ssh`, `$HOME`)
- "Browse" tab with Miller columns (3-column: Parent / Current / Preview)
- Right panel: selected items with `●` bullets
- Fuzzy search overlay (`/`)
- Space to toggle, `w` to finalize, `q/Esc` to cancel
- Pre-selects existing apps when resuming from git pull

**Main View TUI** (primary daily interface):

```
+-------------------------------------------------------------+
| roost · profile: <name>  N apps managed                     |
+------------------------+------------------------------------+
| Apps                   | Parent                             |
| » ★ nvim              | ┌─────────────────────────────┐   |
|   ★ zsh               | │ (miller columns)             │   |
|   git  ←default       | │ Parent | Current | Preview  │   |
|   ★ tmux              | │        |         |          │   |
|   ★ alacritty         | └─────────────────────────────┘   |
+------------------------+------------------------------------+
| j/k nav  Tab focus  / search  s sync  P profiles  ? help   |
|   ·  [apps] o=open  a=add  x=remove  f=link-from  m=paste |
+-------------------------------------------------------------+
```

- Header: `roost · profile: <name>  N apps managed`
- Left panel (24 chars): App list with `★` for primary, `←source` for linked apps
- Right panel: Miller columns for file browsing (Parent | Current | Preview)
- File preview: inline content for files, children for directories
- 8 dialog overlays: Search, Confirm, Ignore, Profile, Git Log, Undo, App Link, Help

### Dialog Overlays

| Dialog | Trigger | Border Color | Width | Purpose |
|---|---|---|---|---|
| Search | `/` | Yellow | 40 | Fuzzy search across apps or files |
| Confirm (Set Primary) | `p` (on file) | Yellow | 54 | Confirm setting a file as primary config |
| Confirm (Remove App) | `x` (on app) | Red | 54 | Confirm removing an app |
| Ignore (Add) | `i` | Yellow | 60 | Type pattern to add to ignore list |
| Ignore (Remove) | `i` + Tab | Yellow | 60 | Select pattern to remove from ignores |
| Profile (Switch) | `P` | Yellow | 50 | List profiles, switch active |
| Profile (Create) | `P` + Tab | Yellow | 50 | Type name, choose [current]/[empty] |
| Profile (Delete) | `P` + Tab+Tab | Red | 50 | Select profile, confirm deletion |
| Git Log | `g` | Yellow | 58 | Browse last 50 commits |
| Rollback | `r` (in git log) | Red | 50 | Confirm rolling back to a commit |
| Undo | `u` | Red | 50 | Confirm undoing last commit |
| App Link (Import) | `f` (on app) | Cyan | 60 | 2-step: pick profile, then pick app |
| App Link (Paste) | `m` (on app) | Yellow | 60 | Copy app to another profile |
| Help | `?` | Yellow/Cyan | 72 | Searchable keybind reference |

### Core Operations

| Operation | Description |
|-----------|-------------|
| Ingest | Move original config to `~/.roost/<profile>/<app>`, create symlink back |
| Restore | Create symlink at original pointing into roost |
| Unlink | Reverse of ingest -- remove symlink, move files back |
| Ensure links | Verify all configured symlinks exist, create missing ones, back up conflicts |
| Switch profile | Remove old profile symlinks, create new profile symlinks |
| Import from profile | Create symlink chain between profiles (zero-copy) |
| Copy to profile | Independent copy of files into another profile |
| Auto-commit | `git add -A && git commit -m <msg>` (if dirty) |
| Sync | Auto-commit + `git pull --rebase` + `git push` |

### Scanner

- Scan directories: `~/.config`, `~/Library/Application Support`, `~/.local/bin`, `~/.ssh`, `$HOME`
- Confidence scoring: known dotfiles (200), known app dirs (150), dirs with config children (100), config files (80), other dirs (50), unknown (10)
- Ignore patterns: exact match and suffix wildcard (`*.log`)
- 16 suggested defaults during init: `node_modules`, `.git`, `.DS_Store`, `*.log`, `*.tmp`, `*.bak`, `*.swp`, `Thumbs.db`, `__pycache__`, `.cache`, `.npm`, `.venv`, `*.pyc`, `.tox`, `dist`, `build`

---

## Fragile Areas to Fix in Rebuild

| Current Problem | Fix |
|-----------------|-----|
| `roost_dest()` heuristic guesses dir vs file based on disk state | Make storage type explicit in the Application model |
| Temp backups go to global `/tmp/roost-backups/` with potential clobber | Use per-app or per-profile backup paths |
| `find_app_on_filesystem()` uses real home, breaks with overridden HOME | Accept base path as parameter |
| `status_message` consumed on every keypress (flashes for 1 frame) | Only consume after meaningful action |
| No concurrency protection on config files | File locking or at minimum, read-modify-write atomicity |
| `git pull --rebase` failures silently ignored | Surface conflicts to user with resolution options |
| Dialog states are flat `Option<T>` fields on a giant struct | Consider state machine enum for type safety |
| `DiffViewState` defined but unused | Don't build dead code |
| `Esc` in onboarding discards selections without confirmation | Add "discard selections?" confirm |
| `ensure_links` and `switch_links` swallow errors | Propagate errors properly |
| Cross-profile symlink cycles only checked at link time | Validate on config load too |

---

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `ratatui` | 0.30+ | TUI framework |
| `crossterm` | 0.29+ | Terminal backend for ratatui |
| `color-eyre` | 0.6+ | Error reporting with backtraces |
| `dialoguer` | 0.12+ | CLI interactive prompts (init wizard) |
| `dirs` | 6+ | Platform-aware paths |
| `serde` | 1+ (derive) | Serialization |
| `toml` | 0.8+ | TOML config files |

**Dev dependencies:** `assert_cmd`, `predicates`, `tempfile`

**Runtime external:** `git` CLI (required for sync/log/diff/undo/rollback), `$EDITOR` (default `vi`), `$PAGER` (default `less`)

---

## Build & Test

- Rust edition 2024, MSRV 1.85+
- `cargo build --release` produces single binary
- `ROOST_DIR` env var overrides `~/.roost`
- Integration tests: one file per subcommand, using `assert_cmd` subprocess pattern with temp directories
- Unit tests: co-located in `src/*/tests.rs` files
- ~93 tests total target (83 integration + 10 unit)

---

## Onboarding Flow (Step by Step)

1. Resolve `ROOST_DIR` or default `~/.roost`
2. Check state: already initialized / partial / empty
3. Optional: configure git remote for multi-device sync
4. Prompt for profile name (default: hostname)
5. Load or create config (if pulled from remote, use existing ignores/apps)
6. Select ignore patterns (MultiSelect with 16 defaults)
7. Write `local.toml` (profile, OS info)
8. Write `.gitignore` (`local.toml`)
9. Launch onboarding TUI for app selection
10. Ingest selected apps (move files, create symlinks)
11. Build `roost.toml` (apps, profiles, guesses for primary_config)
12. `ensure_links()` verification
13. Initial git commit + push (if remote configured)
14. Print rooster ASCII art

---

## Implementation Order (Suggested)

This order maximizes testability and lets you own each layer before building on it:

1. **`app/`** -- Data models + TOML load/save + tilde-path serde
2. **`os_detect.rs`** -- OS detection
3. **`scanner/`** -- File discovery + confidence scoring
4. **`linker/`** -- All symlink operations
5. **`git/`** -- Git CLI wrappers
6. **`init.rs`** + **`logo.rs`** -- Init wizard
7. **`main.rs`** -- CLI dispatch + all subcommand handlers
8. **`tui/search/`** -- Fuzzy search widget
9. **`tui/`** (onboarding) -- Onboarding TUI
10. **`tui/main_view/`** + **`dialogs/`** -- Main TUI with all dialogs
11. Integration tests alongside each layer

Each layer should have its unit tests passing before moving to the next. The CLI subcommands (step 7) are the first time you get a fully usable tool. The TUI (steps 8-10) adds the interactive interface on top.

---

## Constraints for the Rebuild

- **Single binary** -- no external assets at runtime (data files via `include_str!`)
- **No async** -- blocking I/O throughout, suspend TUI for long operations
- **Same backend for CLI and TUI** -- all logic in `app/`, `linker/`, `scanner/`, `git/`; both interfaces call the same functions
- **Backward-compatible config** -- old `roost.toml` formats must migrate on load (dual-format `apps` field, legacy `link_path` -> `link_paths` migration)
- **Cross-platform symlink support** -- Unix + Windows (`#[cfg]` branches)
- **No secrets in git** -- `local.toml` gitignored, no encryption features

---

## Filesystem Layout (Runtime)

```
~/.roost/
  roost.toml          -- SharedAppConfig (git-tracked)
  local.toml          -- LocalAppConfig (git-ignored)
  .gitignore          -- contains "local.toml"
  <profile_name>/     -- One directory per profile
    <app_name>/       -- Directory-based app configs
    misc/             -- Standalone dotfiles
      .<filename>
```

**Directory configs** (like `~/.config/nvim/`): stored at `~/.roost/<profile>/nvim/`
**Standalone files** (like `~/.gitconfig`): stored at `~/.roost/<profile>/misc/.gitconfig`
**Nested `.git` directories** are removed during ingest to avoid submodule issues

**Symlink examples:**

Simple app:
```
~/.config/nvim  -->  ~/.roost/laptop/nvim
```

Cross-profile linked app:
```
~/.config/nvim  -->  ~/.roost/laptop/nvim  -->  ~/.roost/shared/nvim
```

---

## Init Prompt Theme

The dialoguer-based init wizard uses a custom `roost_theme()` with:
- `?` (cyan, bold) -- prompt prefix
- `✓` (green, bold) -- success prefix
- `✗` (red, bold) -- error prefix
- `›` (cyan, bold) -- active item prefix
- `✓` (green, bold) -- checked item prefix
- `○` (white) -- unchecked item prefix
- Separator: `────────────────────────────────────────────────────────────` (60 `─` chars)
