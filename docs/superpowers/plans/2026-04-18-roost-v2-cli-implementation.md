# Roost v2 Rebuild — Phase 1 (CLI) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a fully functional CLI dotfiles manager with 17 subcommands, backed by testable modules.

**Architecture:** Layered modules with no circular deps. `app/` is the leaf (data models + config). `scanner/`, `linker/`, `git/` depend on `app/`. `main.rs` and `init.rs` orchestrate them. Code-ownership mode: user stubs each module, agent fills mechanical parts.

**Tech Stack:** Rust edition 2024, clap (CLI), serde + toml (config), color-eyre (errors), dialoguer (init wizard), ratatui + crossterm (deferred to phase 2).

---

## Current Status

**Completed:**
- Task 1: Scaffolding ✓ (commit `61d3859`)
- Task 2: `app/` ✓ — 10 tests (commit `8fba82b`)
- Task 3: `os_detect` ✓ (commit `8fba82b`)
- Task 4: `scanner/` ✓ — 12 tests (commit `3fa027e`, refactored in `4c26cc4`)
- Task 5: `linker/` ✓ — 14 tests (commit `f2d9180`, updated `e122e0a`)

**Total: 39 tests passing across 4 modules.**

**In progress:**
- Linker test expansion (need coverage for `ensure_links`, `switch_profile`, backup verification, import symlink rejection)

**Next:**
- Task 6: `git/` module

**Remaining:**
- Tasks 6–11 (git, logo+pager, init wizard, CLI dispatch, integration tests, polish)

**Deviations from original plan:**
- Scanner: removed `scan_home()` (hardcoded targets), replaced with `scan_directory(dir, ignored)` — the caller decides which directories to scan
- `AppStorageType` enum simplified to `is_dir: bool` on `Application`
- `OsInfo` lives in `os_detect.rs` (not duplicated in `app/`), imported via `use crate::os_detect::OsInfo`
- Linker: added `import_from` symlink rejection — source must be real files, not a symlink (prevents chains)
- Linker: uses `Path::is_symlink()` instead of `fs::symlink_metadata().is_symlink()` (cleaner API)
- Git: explicit `roost save` instead of auto-commit (per design doc decision)
- `main.rs` uses `use roost::app;` (library crate), not `mod app;` (binary crate)

---

## Code-Ownership Mode Conventions

Each task follows this pattern:
1. **Agent describes** what needs to exist and why
2. **User stubs** — writes struct definitions, function signatures, empty bodies with `// agent: fill this`
3. **Agent fills** — implements the mechanical logic inside marked bodies
4. **User runs tests** — verifies understanding
5. **Recap** — user walks through what was built and why

If a task is boilerplate-heavy (e.g., repeating a pattern across multiple functions), the agent may ask to switch to **Guided Mode** for that task.

---

## File Structure

```
src/
  main.rs              -- CLI entry, clap dispatch
  lib.rs               -- Re-exports all modules (for integration tests)

  data/
    mod.rs             -- include_str! for known_apps.txt, known_dotfiles.txt, parsed into HashSets
    known_apps.txt     -- copied from MVP/data/
    known_dotfiles.txt -- copied from MVP/data/

  app/
    mod.rs             -- SharedAppConfig, LocalAppConfig, Application, Profile
                        -- TOML load/save, backward compat migration, config validation
    tests.rs           -- Unit tests for config load/save, validation

  os_detect.rs         -- OsInfo struct + detect() function

  scanner/
    mod.rs             -- scan_directory, confidence scoring, DiscoveredItem
    tests.rs           -- Unit tests for scoring, filtering, ignore patterns

  linker/
    mod.rs             -- ingest, restore, unlink, ensure_links, switch_profile, import_from, copy_to
                        -- + helpers: app_dest, copy_dir_recursive, create_symlink
    tests.rs           -- Unit tests for each operation

  git/
    mod.rs             -- init, save, sync, log, diff, undo, rollback, remote ops, is_dirty
    tests.rs           -- Unit tests for git operations

  init.rs              -- 14-step dialoguer-based init wizard
  logo.rs              -- ASCII art constant
  pager.rs             -- External pager ($PAGER / less)

tests/
  init.rs              -- Integration test: roost init
  add.rs               -- Integration test: roost add
  remove.rs            -- Integration test: roost remove
  status.rs            -- Integration test: roost status
  sync.rs              -- Integration test: roost sync
  save.rs              -- Integration test: roost save
  profile.rs           -- Integration test: roost profile subcommands
  diff.rs              -- Integration test: roost diff
  log.rs               -- Integration test: roost log
  undo.rs              -- Integration test: roost undo
  rollback.rs          -- Integration test: roost rollback
  restore.rs           -- Integration test: roost restore
  remote.rs            -- Integration test: roost remote
  doctor.rs            -- Integration test: roost doctor
  adopt.rs             -- Integration test: roost adopt
  where.rs             -- Integration test: roost where
```

---

## Task 1: Project Scaffolding

**Status: ✓ Complete** (commit `61d3859`)

- [x] Initialize Cargo project with dependencies
- [x] Copy data files and create data module (include_str!, HashSet parsing, DEFAULT_IGNORE_PATTERNS)
- [x] Create minimal lib.rs, main.rs, .gitignore
- [x] Verify compiles and data loads (4 data tests passing)

---

## Task 2: Data Models & Config (`app/`)

**Status: ✓ Complete** (commit `8fba82b`)

- [x] User stubbed struct/enum definitions with serde derives
- [x] Agent filled TOML load/save, validation, migration
- [x] Agent filled test implementations (10 tests)
- [x] All tests pass

**Implemented:**
- `SharedAppConfig`, `LocalAppConfig`, `Application`, `Profile`
- `Application.is_dir: bool` (not `AppStorageType` enum — simplified)
- `OsInfo` defined in `os_detect.rs`, imported by `app/`
- `load_shared`, `save_shared`, `load_local`, `save_local`
- `validate_shared` — referential integrity, direct cycle detection in app_sources
- `roost_dir()`, `shared_config_path()`, `local_config_path()`

---

## Task 3: OS Detection (`os_detect.rs`)

**Status: ✓ Complete** (commit `8fba82b`)

- [x] OsInfo struct with serde derives
- [x] detect() using cfg!(target_os) / cfg!(target_arch)
- [x] Compiles and works

---

## Task 4: Scanner (`scanner/`)

**Status: ✓ Complete** (commit `3fa027e`, refactored `4c26cc4`)

- [x] DiscoveredItem, ItemType types
- [x] Confidence scoring (known dotfiles=200, known apps=150, config-children dirs=100, config files=80, other dirs=50, unknown=10)
- [x] Ignore pattern matching (exact + suffix wildcard)
- [x] 12 tests passing

**Refactored:** Removed `scan_home()` (hardcoded scan targets). Now exposes `scan_directory(dir, ignored) -> Vec<DiscoveredItem>`. The caller (init wizard / TUI) decides which directories to scan.

---

## Task 5: Linker (`linker/`)

**Status: ✓ Complete** (commit `f2d9180`, updated `e122e0a`)

- [x] User stubbed function signatures
- [x] Agent filled all implementations in guided mode
- [x] 14 tests passing
- [x] Comments added
- [x] Symlink checks updated to use `Path::is_symlink()`

**Implemented:**
- `ingest` — move origin to roost, symlink back
- `restore` — create symlink at origin pointing to roost (fresh setup)
- `unlink` — remove symlink, move files back
- `ensure_links` — verify/create all configured symlinks, back up conflicts
- `switch_profile` — remove old profile symlinks, create new ones
- `import_from` — cross-profile symlink chain (rejects symlink sources)
- `copy_to` — independent copy into another profile
- `app_dest` — path resolution (dir vs misc)
- `copy_dir_recursive` — recursive directory copy
- `create_symlink` — cross-platform (Unix/Windows)

**Test gaps (need expansion):**
- `ensure_links` — no test coverage
- `switch_profile` — no test coverage
- Backup creation — not verified
- `import_from` symlink rejection — no test
- `unlink` misc/ cleanup — no test

---

## Task 6: Git Module (`git/`)

**Status: Not started**

**Files:**
- Create: `src/git/mod.rs`
- Create: `src/git/tests.rs`
- Modify: `src/lib.rs` (add `pub mod git`)

**What and why:** Wraps git CLI for version control. No auto-commit. Structured return types for sync results.

**Key types:**
```rust
pub struct CommitInfo {
    pub hash: String,
    pub message: String,
    pub timestamp: String,
}

pub enum SyncResult {
    Clean,
    Conflict { files: Vec<PathBuf>, message: String },
}
```

**Key functions:**
```rust
pub fn init(roost_dir: &Path) -> Result<()>
pub fn save(roost_dir: &Path, message: &str) -> Result<bool>      // true if committed, false if clean
pub fn sync(roost_dir: &Path) -> Result<SyncResult>
pub fn log(roost_dir: &Path, n: usize) -> Result<Vec<CommitInfo>>
pub fn diff(roost_dir: &Path) -> Result<String>
pub fn undo(roost_dir: &Path, n: usize) -> Result<()>
pub fn rollback(roost_dir: &Path, hash: &str) -> Result<()>
pub fn set_remote(roost_dir: &Path, url: &str) -> Result<()>
pub fn get_remote(roost_dir: &Path) -> Result<Option<String>>
pub fn is_dirty(roost_dir: &Path) -> Result<bool>
```

All functions invoke `git` as a subprocess via `std::process::Command`. Parse output for structured types. `sync` detects rebase conflicts by checking exit code and `git status` output.

**Unit tests** (using temp git repos):
- Init creates .git directory
- Save commits changes, returns false when clean
- Log returns commits in order
- Diff returns uncommitted changes
- Undo resets HEAD
- Rollback resets to specific hash
- is_dirty detects changes

- [ ] **Step 1: User stubs types and all function signatures**
- [ ] **Step 2: Agent fills implementations**
- [ ] **Step 3: User stubs tests, agent fills test bodies**
- [ ] **Step 4: Run `cargo test --lib git` — all tests pass**
- [ ] **Step 5: Commit**

```bash
git add src/git/ src/lib.rs
git commit -m "feat: git module — CLI wrappers for version control"
```

- [ ] **Step 6: Recap — user walks through sync conflict handling**

---

## Task 7: Logo & Pager Utilities

**Status: Not started**

**Files:**
- Create: `src/logo.rs`
- Create: `src/pager.rs`
- Modify: `src/lib.rs` (add both modules)

- [ ] **Step 1: User creates logo.rs with the ASCII art constant**
- [ ] **Step 2: User stubs pager::open, agent fills implementation**
- [ ] **Step 3: Verify `cargo build` succeeds**
- [ ] **Step 4: Commit**

---

## Task 8: Init Wizard (`init.rs`)

**Status: Not started**

**Files:**
- Create: `src/init.rs`
- Modify: `src/lib.rs` (add `pub mod init`)

**14-step onboarding flow** using dialoguer prompts. Orchestrates app/, scanner/, linker/, git/.

- [ ] **Step 1: User stubs init.rs with function signatures and roost_theme()**
- [ ] **Step 2: Agent fills implementation step by step (user reviews each step)**
- [ ] **Step 3: Manual test — run `cargo run -- init` in a temp directory**
- [ ] **Step 4: Commit**
- [ ] **Step 5: Recap — user walks through the full init flow**

---

## Task 9: CLI Dispatch (`main.rs`)

**Status: Not started**

**Files:**
- Modify: `src/main.rs`

**Thin clap dispatch layer.** 17 subcommands via derive macros. No business logic.

- [ ] **Step 1: User stubs the Cli enum and main match**
- [ ] **Step 2: Agent fills each subcommand handler**
- [ ] **Step 3: Manual test — run each subcommand**
- [ ] **Step 4: Commit**
- [ ] **Step 5: Recap — user walks through the full CLI surface**

---

## Task 10: Integration Tests

**Status: Not started**

**Files:**
- Create: `tests/*.rs` (16 test files, ~83 tests)

End-to-end tests using `assert_cmd` subprocesses with temp dirs.

- [ ] **Step 1: User stubs test files with test function signatures**
- [ ] **Step 2: Agent fills test implementations (batch per file)**
- [ ] **Step 3: Run `cargo test` — all integration tests pass**
- [ ] **Step 4: Commit**
- [ ] **Step 5: Recap — user walks through the test strategy**

---

## Task 11: Polish & Edge Cases

**Status: Not started**

- [ ] **Step 1: Run full test suite, identify failures**
- [ ] **Step 2: Fix edge cases**
- [ ] **Step 3: Run `cargo test` — all tests pass**
- [ ] **Step 4: Run `cargo clippy` — no warnings**
- [ ] **Step 5: Commit**

---

## Dependency Graph (execution order)

```
Task 1: Scaffolding ✓
  ↓
Task 2: app/ ✓
  ↓
Task 3: os_detect ✓
  ↓
Task 4: scanner/ ✓
  ↓
Task 5: linker/ ✓
  ↓
Task 6: git/            ← NEXT
  ↓
Task 7: logo + pager
  ↓
Task 8: init.rs
  ↓
Task 9: main.rs
  ↓
Task 10: Integration tests
  ↓
Task 11: Polish
```

---

## What This Plan Does NOT Cover (Phase 2)

- TUI (onboarding and main view)
- Miller columns
- Dialog overlays (search, confirm, help, ignore, profile, git log, undo, app link)
- Fuzzy search widget
- Suspend/resume for $EDITOR in TUI context
- Dirty indicator in status bar
- Any `ratatui`/`crossterm` code
