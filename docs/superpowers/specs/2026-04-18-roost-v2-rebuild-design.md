# Roost v2 Rebuild Design

## Summary

Clean rewrite of roost (terminal-based dotfile manager) in the `roost-v1.2/` directory. Phase 1 delivers the full CLI with all 15 subcommands. Phase 2 adds TUI later.

## Decisions

- **Approach:** Spec-ordered layers. Each module built, tested, and owned before stacking the next.
- **Old code:** Reference only. No porting.
- **Git sync:** Explicit `roost save` to commit, `roost sync` to pull+push. No auto-commit. Dirty indicator in TUI (phase 2).
- **CLI parser:** `clap` with derive macros for 15 subcommands.
- **Error handling:** `color-eyre` with backtraces.
- **Testing:** Unit tests co-located (`src/*/tests.rs`), integration tests in `tests/` using `assert_cmd` + temp dirs. ~93 tests.
- **Learning:** Maximum. Code-ownership mode active. You stub, I fill mechanical parts.

## Project Scaffolding

```
roost-v1.2/
  Cargo.toml
  src/
    main.rs
    lib.rs
    app/        mod.rs, tests.rs
    linker/     mod.rs, tests.rs
    scanner/    mod.rs, tests.rs
    git/        mod.rs, tests.rs
    data/       mod.rs       (include_str! for known_apps.txt, known_dotfiles.txt)
    init.rs
    os_detect.rs
    logo.rs
    pager.rs
  tests/        (integration tests, one file per subcommand)
  MVP/          (preserved as-is)
```

### Dependencies

```toml
[dependencies]
ratatui = "0.30+"
crossterm = "0.29+"
color-eyre = "0.6+"
dialoguer = "0.12+"
dirs = "6+"
serde = { version = "1+", features = ["derive"] }
toml = "0.8+"
clap = { version = "4+", features = ["derive"] }

[dev-dependencies]
assert_cmd = "2+"
predicates = "3+"
tempfile = "3+"
```

Rust edition 2024, MSRV 1.85. Single binary. No external assets at runtime.

## Data Models (`app/`)

### SharedAppConfig (roost.toml, git-tracked)

```rust
struct SharedAppConfig {
    remote: Option<String>,
    profiles: HashMap<String, Profile>,
    apps: HashMap<String, Application>,
    ignored: HashSet<String>,
}
```

### LocalAppConfig (local.toml, git-ignored)

```rust
struct LocalAppConfig {
    active_profile: String,
    os_info: OsInfo,
    link_paths: HashMap<String, PathBuf>,  // app -> original path on this device
}
```

### Application

```rust
struct Application {
    name: String,
    primary_config: Option<PathBuf>,  // relative to app's roost dir
    storage_type: AppStorageType,     // explicit: Dir | File
    on_profiles: Vec<String>,
}

enum AppStorageType { Dir, File }
```

### Profile

```rust
struct Profile {
    apps: HashSet<String>,
    app_sources: HashMap<String, String>,  // app -> source_profile
}
```

### TOML Format

```toml
# roost.toml
remote = "git@github.com:user/dotfiles"
ignored = ["*.log", ".DS_Store"]

[profiles.laptop]
apps = ["nvim", "zsh", "git"]
[profiles.laptop.app_sources]
nvim = "shared"

[apps.nvim]
primary_config = "init.lua"
storage_type = "dir"
on_profiles = ["laptop"]
```

```toml
# local.toml
active_profile = "laptop"
[os_info]
os = "macos"
arch = "aarch64"
[link_paths]
nvim = "/Users/mars/.config/nvim"
```

### Key Design Points

- `primary_config` is relative to `ROOST_DIR/<profile>/<app>/`. No tilde expansion needed. Resolves at runtime.
- `storage_type` is determined once at ingest time and persisted. Eliminates v1's `roost_dest()` heuristic bug.
- `link_paths` stays in local.toml (device-specific).
- Backward compat on load: old `apps` list format migrated to table, `link_path` -> `link_paths`.
- Advisory file locking (`flock` on Unix) for config read-modify-write.

## Scanner (`scanner/`)

Scans 5 directories: `~/.config`, `~/Library/Application Support`, `~/.local/bin`, `~/.ssh`, `$HOME`.

Confidence scoring:
| Source | Score |
|--------|-------|
| Known dotfile name | 200 |
| Known app dir name | 150 |
| Dir with config-file children | 100 |
| Config file | 80 |
| Other directory | 50 |
| Unknown | 10 |

Known lists embedded via `include_str!`. 16 default ignore patterns for init.

Returns `Vec<DiscoveredItem>` sorted by confidence descending.

## Linker (`linker/`)

All symlink operations. Each function accepts explicit paths (no hardcoded HOME).

| Function | Description |
|----------|-------------|
| `ingest` | Move original to roost, create symlink back |
| `restore` | Create symlink at original pointing into roost |
| `unlink` | Remove symlink, move roost files back |
| `ensure_links` | Verify/create all configured symlinks, back up conflicts |
| `switch_profile` | Remove old profile links, create new profile links |
| `import_from` | Create symlink chain between profiles (zero-copy) |
| `copy_to` | Independent copy of files into another profile |

Fixes:
- Per-app backups: `~/.roost/.backups/<timestamp>-<app>/`
- All errors propagated (no swallowed errors)
- Cycle detection on cross-profile links, validated at config load

## Git Module (`git/`)

Wraps git CLI. Returns structured results.

| Function | Returns |
|----------|---------|
| `init` | `Result<()>` |
| `save` | `Result<bool>` (true if committed, false if clean) |
| `sync` | `Result<SyncResult>` |
| `log` | `Result<Vec<CommitInfo>>` |
| `diff` | `Result<String>` |
| `undo` | `Result<()>` |
| `rollback` | `Result<()>` |
| `set_remote` | `Result<()>` |
| `get_remote` | `Result<Option<String>>` |
| `is_dirty` | `Result<bool>` |

`SyncResult` enum: `Clean`, `Conflict { files: Vec<PathBuf>, message: String }`.

No auto-commit. `save` = explicit `git add -A && git commit`. `sync` = `git pull --rebase && git push`. On conflict, surface files and resolution instructions.

## Init Wizard (`init.rs`)

Dialoguer-based, following 14-step onboarding flow from spec:
1. Resolve ROOST_DIR
2. Check existing state
3. Optional: configure git remote
4. Prompt for profile name (default: hostname)
5. Load or create config
6. Select ignore patterns (MultiSelect with 16 defaults)
7. Write local.toml, .gitignore
8. Launch scanner, present discovered apps
9. User selects apps
10. Ingest selected apps
11. Build roost.toml
12. ensure_links verification
13. Initial git commit
14. Print rooster ASCII art

## CLI Commands (`main.rs`)

15 subcommands via clap. Thin dispatch layer -- no business logic.

| Command | Backend calls |
|---------|--------------|
| `roost` (no args) | Launch TUI (phase 2) |
| `roost init` | `init::run_wizard()` |
| `roost add <path>` | `linker::ingest`, `app::add_app` |
| `roost remove <app>` | `linker::unlink`, `app::remove_app` |
| `roost status` | `linker::check_links`, display |
| `roost sync` | `git::sync` |
| `roost save` | `git::save` |
| `roost profile list/switch/add/delete/rename` | `app::profile_ops` |
| `roost diff` | `git::diff` |
| `roost log` | `git::log` |
| `roost undo [n]` | `git::undo` |
| `roost rollback <hash>` | `git::rollback` |
| `roost restore <app>` | `linker::restore` |
| `roost remote [url]` | `git::get/set_remote` |
| `roost doctor` | `linker::ensure_links`, consistency checks |
| `roost adopt` | `scanner::scan`, `app::adopt_orphaned` |
| `roost where <app>` | Display roost path for app |
| `roost help` | Clap auto-generated |

Note: `roost save` added (not in original spec) for explicit git committing.

## Module Dependency Graph

```
main.rs --> app/, linker/, scanner/, git/, init.rs
init.rs --> app/, scanner/, linker/, git/
linker/  --> app/
scanner/ --> data/ (standalone)
git/     --> (standalone, wraps git CLI)
app/     --> (standalone, serde/toml)
data/    --> (standalone, include_str!)
```

No circular dependencies. `app/` is the leaf module.

## Fragile Areas Fixed

| v1 Problem | v2 Fix |
|------------|--------|
| `roost_dest()` heuristic | Explicit `storage_type` field on Application |
| Global `/tmp/roost-backups/` clobber | Per-app backups in `~/.roost/.backups/` |
| `find_app_on_filesystem()` uses real HOME | All functions accept base path parameter |
| `status_message` consumed every keypress | Only consume after meaningful action (TUI phase) |
| No config file locking | Advisory `flock` on config files |
| `git pull --rebase` failures silently ignored | `SyncResult::Conflict` with resolution instructions |
| Flat `Option<Dialog>` fields on giant struct | State machine enum for dialog states (TUI phase) |
| `DiffViewState` unused dead code | Don't build it (TUI phase) |
| `Esc` discards selections without confirmation | Add confirm dialog (TUI phase) |
| `ensure_links`/`switch_links` swallow errors | Propagate all errors with color-eyre |
| Cross-profile cycles only at link time | Validate on config load too |
| Auto-commit creates phantom conflicts | Explicit save, no auto-commit |

## Implementation Order

1. **Project scaffolding** -- Cargo.toml, directory structure, data embedding
2. **`app/`** -- Data models, TOML load/save, backward compat
3. **`os_detect.rs`** -- Runtime OS detection
4. **`scanner/`** -- File discovery + confidence scoring
5. **`linker/`** -- All symlink operations
6. **`git/`** -- Git CLI wrappers
7. **`init.rs`** + `logo.rs` -- Init wizard
8. **`main.rs`** -- CLI dispatch, all subcommands
9. **Integration tests** -- Alongside each layer
10. **Phase 2: TUI** -- Deferred

Each layer: you stub, I fill mechanical parts, tests pass, recap, then next layer.

## Testing

- Unit tests in `src/*/tests.rs`
- Integration tests in `tests/*.rs` using `assert_cmd` subprocesses with temp dirs
- `ROOST_DIR` env var for test isolation
- ~93 tests target (83 integration + 10 unit)
- Tests written alongside each module, not batched at the end

## Filesystem Layout (Runtime)

```
~/.roost/
  roost.toml          -- SharedAppConfig (git-tracked)
  local.toml          -- LocalAppConfig (git-ignored)
  .gitignore          -- contains "local.toml"
  .backups/           -- Per-operation backups
  <profile>/          -- One directory per profile
    <app>/            -- Directory-based configs (storage_type = Dir)
    misc/             -- Standalone dotfiles (storage_type = File)
      .<filename>
```

Symlink examples:
```
Simple:     ~/.config/nvim --> ~/.roost/laptop/nvim
Cross-prof: ~/.config/nvim --> ~/.roost/laptop/nvim --> ~/.roost/shared/nvim
```
