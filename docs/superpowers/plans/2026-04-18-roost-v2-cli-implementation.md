# Roost v2 Rebuild — Phase 1 (CLI) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a fully functional CLI dotfiles manager with 17 subcommands, backed by testable modules.

**Architecture:** Layered modules with no circular deps. `app/` is the leaf (data models + config). `scanner/`, `linker/`, `git/` depend on `app/`. `main.rs` and `init.rs` orchestrate them. Code-ownership mode: user stubs each module, agent fills mechanical parts.

**Tech Stack:** Rust edition 2024, clap (CLI), serde + toml (config), color-eyre (errors), dialoguer (init wizard), ratatui + crossterm (deferred to phase 2).

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
    mod.rs             -- SharedAppConfig, LocalAppConfig, Application, Profile, AppStorageType
                        -- TOML load/save, backward compat migration, config validation
    tests.rs           -- Unit tests for config load/save, tilde paths, migrations

  os_detect.rs         -- Runtime OS/arch detection

  scanner/
    mod.rs             -- scan directories, confidence scoring, DiscoveredItem
    tests.rs           -- Unit tests for scoring, filtering, ignore patterns

  linker/
    mod.rs             -- ingest, restore, unlink, ensure_links, switch_profile, import_from, copy_to
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

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`
- Create: `src/data/mod.rs`
- Create: `src/data/known_apps.txt` (copy from `MVP/data/`)
- Create: `src/data/known_dotfiles.txt` (copy from `MVP/data/`)
- Create: `.gitignore`

**What and why:** Set up the Rust project with all dependencies, the data module (embedded known app/dotfile lists), and a minimal `main.rs` that compiles. This is the foundation everything else builds on.

- [ ] **Step 1: Initialize Cargo project**

Run `cargo init` in `roost-v1.2/`, then replace `Cargo.toml` with:

```toml
[package]
name = "roost"
version = "0.2.0"
edition = "2024"
license = "MIT"

[dependencies]
ratatui = "0.30"
crossterm = "0.29"
color-eyre = "0.6"
dialoguer = "0.12"
dirs = "6"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
clap = { version = "4", features = ["derive"] }

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
tempfile = "3"
```

- [ ] **Step 2: Copy data files and create data module**

Copy `MVP/data/known_apps.txt` and `MVP/data/known_dotfiles.txt` into `src/data/`. Create `src/data/mod.rs` that uses `include_str!` to embed both files and parses them into `HashSet<&'static str>` (ignoring comment lines starting with `#` and blank lines). Expose as `pub fn known_apps() -> HashSet<&'static str>` and `pub fn known_dotfiles() -> HashSet<&'static str>`.

- [ ] **Step 3: Create minimal lib.rs**

```rust
pub mod data;
// More modules will be added as we build them.
```

- [ ] **Step 4: Create minimal main.rs**

```rust
fn main() {
    println!("roost v0.2.0");
}
```

- [ ] **Step 5: Create .gitignore**

```
/target
```

- [ ] **Step 6: Verify it compiles and data loads**

Run: `cargo build`
Run: `cargo test` (should pass with 0 tests)
Run: `cargo run` (should print "roost v0.2.0")

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: project scaffolding with embedded data files"
```

---

## Task 2: Data Models & Config (`app/`)

**Files:**
- Create: `src/app/mod.rs`
- Create: `src/app/tests.rs`
- Modify: `src/lib.rs` (add `pub mod app`)

**What and why:** The leaf module. All other modules depend on these types. Defines `SharedAppConfig`, `LocalAppConfig`, `Application`, `Profile`, `AppStorageType`, `OsInfo`. Handles TOML serialization/deserialization, backward compat migration, config validation (cycle detection, referential integrity).

**User stubs:**
- All struct/enum definitions with serde derives
- Function signatures: `load_shared(path) -> Result<SharedAppConfig>`, `save_shared(path, &SharedAppConfig) -> Result<()>`, `load_local(path) -> Result<LocalAppConfig>`, `save_local(path, &LocalAppConfig) -> Result<()>`
- Validation function signatures: `validate_config(&SharedAppConfig) -> Result<()>`
- Migration function signature: `migrate_shared(raw_toml: &str) -> String`
- Mark function bodies with `// agent: fill this`

**Agent fills:**
- TOML parse/serialize logic inside the load/save functions
- Backward compat migration (old `apps` list → table, old `link_path` → `link_paths`)
- Validation logic (cycle detection in app_sources, apps referenced by profiles exist in apps map, ignored apps not in active profiles)
- Custom serde for `AppStorageType` (serialize as lowercase string "dir"/"file")

**Unit tests:**
- Round-trip: create config, save, load, assert equal
- Backward compat: load old-format TOML string, verify migration
- Validation: cycle detection rejects A→B→A, missing app reference rejects
- Edge cases: empty config, config with only ignored patterns

- [ ] **Step 1: User stubs all types and function signatures in `src/app/mod.rs`**
- [ ] **Step 2: Agent fills implementation**
- [ ] **Step 3: User stubs test file `src/app/tests.rs`**
- [ ] **Step 4: Agent fills test implementations**
- [ ] **Step 5: Run `cargo test --lib app` — all tests pass**
- [ ] **Step 6: Commit**

```bash
git add src/app/ src/lib.rs
git commit -m "feat: data models, config load/save, backward compat"
```

- [ ] **Step 7: Recap — user walks through what was built**

---

## Task 3: OS Detection (`os_detect.rs`)

**Files:**
- Create: `src/os_detect.rs`
- Modify: `src/lib.rs` (add `pub mod os_detect`)

**What and why:** Small standalone module. Detects OS and arch at runtime for `LocalAppConfig.os_info`. Used during init and when validating config compatibility.

**Contents:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsInfo {
    pub os: String,    // "macos", "linux", "windows"
    pub arch: String,  // "aarch64", "x86_64"
}

pub fn detect() -> OsInfo { ... }
```

Uses `cfg!(target_os)` and `cfg!(target_arch)` for compile-time detection. No external deps needed.

- [ ] **Step 1: User stubs OsInfo struct and detect() signature**
- [ ] **Step 2: Agent fills detect() implementation**
- [ ] **Step 3: Verify `cargo test` passes, `cargo build` succeeds**
- [ ] **Step 4: Commit**

```bash
git add src/os_detect.rs src/lib.rs
git commit -m "feat: OS detection module"
```

---

## Task 4: Scanner (`scanner/`)

**Files:**
- Create: `src/scanner/mod.rs`
- Create: `src/scanner/tests.rs`
- Modify: `src/lib.rs` (add `pub mod scanner`)

**What and why:** Discovers candidate dotfiles/app configs on the filesystem. Used during `roost init` (app selection) and `roost add` (importing a path). Returns scored results sorted by confidence.

**Key types:**
```rust
pub struct DiscoveredItem {
    pub path: PathBuf,       // absolute path to the item
    pub name: String,        // filename/dirname
    pub confidence: u32,     // higher = more likely a config
    pub item_type: ItemType, // Dir or File
}

pub enum ItemType { Dir, File }
```

**Key functions:**
```rust
pub fn scan_home(base: &Path, ignored: &HashSet<String>) -> Result<Vec<DiscoveredItem>>
```

Scans 5 directories (`~/.config`, `~/Library/Application Support`, `~/.local/bin`, `~/.ssh`, `$HOME`). Each item scored using `data::known_apps()` and `data::known_dotfiles()`. Filters by ignore patterns (exact match + suffix wildcard like `*.log`). Returns sorted by confidence descending.

**Unit tests:**
- Score known dotfile name correctly (200)
- Score known app dir correctly (150)
- Score dir with config children (100)
- Filter by ignore patterns
- Sort by confidence descending

- [ ] **Step 1: User stubs DiscoveredItem, ItemType, scan_home signature**
- [ ] **Step 2: Agent fills implementation**
- [ ] **Step 3: User stubs tests, agent fills test bodies**
- [ ] **Step 4: Run `cargo test --lib scanner` — all tests pass**
- [ ] **Step 5: Commit**

```bash
git add src/scanner/ src/lib.rs
git commit -m "feat: scanner with confidence scoring"
```

- [ ] **Step 6: Recap — user walks through scoring logic**

---

## Task 5: Linker (`linker/`)

**Files:**
- Create: `src/linker/mod.rs`
- Create: `src/linker/tests.rs`
- Modify: `src/lib.rs` (add `pub mod linker`)

**What and why:** The core value of roost. All symlink operations. Each function takes explicit paths (no hardcoded HOME). Backs up conflicts to per-app paths. Propagates all errors.

**Key functions:**
```rust
pub fn ingest(original: &Path, profile_dir: &Path, app_name: &str, storage_type: AppStorageType) -> Result<()>
pub fn restore(app_name: &str, profile_dir: &Path, original: &Path) -> Result<()>
pub fn unlink(app_name: &str, profile_dir: &Path, original: &Path) -> Result<()>
pub fn ensure_links(config: &SharedAppConfig, local: &LocalAppConfig, roost_dir: &Path) -> Result<Vec<String>>
pub fn switch_profile(old: &str, new: &str, config: &SharedAppConfig, local: &mut LocalAppConfig, roost_dir: &Path) -> Result<()>
pub fn import_from(app: &str, source_profile: &str, target_profile: &str, roost_dir: &Path) -> Result<()>
pub fn copy_to(app: &str, source_profile: &str, target_profile: &str, roost_dir: &Path) -> Result<()>
```

**Backup strategy:** `~/.roost/.backups/<timestamp>-<app>/` — per-app, timestamped, no clobbering.

**Cycle detection:** `import_from` checks for existing symlink chains and rejects cycles. Also validated at config load time (Task 2).

**ensure_links returns:** Vec of strings describing what was fixed (for `roost doctor` output).

**Unit tests** (using temp directories):
- Ingest: file moved to roost dir, symlink created at original
- Ingest: dir moved to roost dir, symlink created at original
- Restore: symlink at original points to roost dir
- Unlink: symlink removed, files moved back to original
- Ensure links: missing symlinks created
- Ensure links: conflict backed up before overwrite
- Switch profile: old symlinks removed, new ones created
- Import from: symlink chain created
- Import from: cycle detected and rejected
- Copy to: independent copy created

- [ ] **Step 1: User stubs all function signatures in `src/linker/mod.rs`**
- [ ] **Step 2: Agent fills implementations one function at a time (user reviews each)**
- [ ] **Step 3: User stubs test file, agent fills test bodies**
- [ ] **Step 4: Run `cargo test --lib linker` — all tests pass**
- [ ] **Step 5: Commit**

```bash
git add src/linker/ src/lib.rs
git commit -m "feat: linker — all symlink operations"
```

- [ ] **Step 6: Recap — this is the most important module. User walks through the full flow.**

---

## Task 6: Git Module (`git/`)

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

**Files:**
- Create: `src/logo.rs`
- Create: `src/pager.rs`
- Modify: `src/lib.rs` (add both modules)

**What and why:** Two small standalone modules.

**logo.rs:** A `pub const ROOST_LOGO: &str` with the ASCII art from the spec. Used at the end of `roost init`.

**pager.rs:** A `pub fn open(text: &str) -> Result<()>` function that pipes text through `$PAGER` (defaulting to `less`). Used for `roost diff` and `roost log` output. Suspend/resume pattern: create temp file, spawn pager process, wait for exit.

- [ ] **Step 1: User creates logo.rs with the ASCII art constant**
- [ ] **Step 2: User stubs pager::open, agent fills implementation**
- [ ] **Step 3: Verify `cargo build` succeeds**
- [ ] **Step 4: Commit**

```bash
git add src/logo.rs src/pager.rs src/lib.rs
git commit -m "feat: logo and pager utilities"
```

---

## Task 8: Init Wizard (`init.rs`)

**Files:**
- Create: `src/init.rs`
- Modify: `src/lib.rs` (add `pub mod init`)

**What and why:** The 14-step onboarding flow. Uses `dialoguer` for interactive CLI prompts. Orchestrates `app/`, `scanner/`, `linker/`, `git/`.

**Flow:**
1. Resolve `ROOST_DIR` (env var or `~/.roost`)
2. Check state: already initialized / partial / empty
3. Optional: configure git remote
4. Prompt for profile name (default: hostname via `gethostname::gethostname()` or `hostname` command)
5. Load or create config
6. Select ignore patterns (MultiSelect with 16 defaults)
7. Write `local.toml` and `.gitignore`
8. Run scanner, present discovered apps via MultiSelect
9. Ingest selected apps (calling `linker::ingest` for each)
10. Build `roost.toml` with apps, profiles, guessed `primary_config`
11. Call `linker::ensure_links` verification
12. Initial `git::save` commit
13. Print `ROOST_LOGO`
14. If remote configured, push

**Custom dialoguer theme** (`roost_theme()`):
- `?` (cyan, bold) — prompt prefix
- `✓` (green, bold) — success prefix
- `✗` (red, bold) — error prefix
- `›` (cyan, bold) — active item prefix
- `✓` (green, bold) — checked prefix
- `○` (white) — unchecked prefix
- Separator: 60 `─` chars

**Key function:**
```rust
pub fn run_wizard() -> Result<()>
```

This is the most complex orchestration function. It calls into all other modules but contains no business logic itself.

- [ ] **Step 1: User stubs init.rs with function signatures and roost_theme()**
- [ ] **Step 2: Agent fills implementation step by step (user reviews each step)**
- [ ] **Step 3: Manual test — run `cargo run -- init` in a temp directory**
- [ ] **Step 4: Commit**

```bash
git add src/init.rs src/lib.rs
git commit -m "feat: init wizard with dialoguer prompts"
```

- [ ] **Step 5: Recap — user walks through the full init flow**

---

## Task 9: CLI Dispatch (`main.rs`)

**Files:**
- Modify: `src/main.rs`

**What and why:** Thin clap dispatch layer. Parses args and calls the right backend function. No business logic here.

**Clap structure:**
```rust
#[derive(Parser)]
#[command(name = "roost", version, about = "A terminal-based dotfile manager")]
enum Cli {
    Init,
    Add { path: PathBuf },
    Remove { app: String },
    Status,
    Sync,
    Save { message: Option<String> },
    Profile { command: ProfileCommand },
    Diff,
    Log { n: Option<usize> },
    Undo { n: Option<usize> },
    Rollback { hash: String },
    Restore { app: String },
    Remote { url: Option<String> },
    Doctor,
    Adopt,
    Where { app: String },
}

#[derive(Subcommand)]
enum ProfileCommand {
    List,
    Switch { name: String },
    Add { name: String },
    Delete { name: String },
    Rename { old: String, new: String },
}
```

**Each match arm:**
1. Resolve `ROOST_DIR`
2. Load configs (shared + local)
3. Call the appropriate backend function
4. Print result / error
5. Exit with appropriate code

**Color-eyre setup** in main: `color_eyre::install()?` at the top of main.

**Status output format:**
```
★ nvim    linked    ~/.config/nvim → ~/.roost/laptop/nvim
  zsh      linked    ~/.zshrc → ~/.roost/laptop/misc/.zshrc
  git      broken    ~/.config/git (symlink target missing)
  tmux     missing   ~/.tmux.conf (not symlinked)
```

**Where output:**
```
nvim → ~/.roost/laptop/nvim
  primary: init.lua
  storage: directory
  profiles: laptop, shared (source)
```

- [ ] **Step 1: User stubs the Cli enum and main match**
- [ ] **Step 2: Agent fills each subcommand handler**
- [ ] **Step 3: Manual test — run each subcommand**
- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: CLI dispatch with all subcommands"
```

- [ ] **Step 5: Recap — user walks through the full CLI surface**

---

## Task 10: Integration Tests

**Files:**
- Create: `tests/init.rs`
- Create: `tests/add.rs`
- Create: `tests/remove.rs`
- Create: `tests/status.rs`
- Create: `tests/sync.rs`
- Create: `tests/save.rs`
- Create: `tests/profile.rs`
- Create: `tests/diff.rs`
- Create: `tests/log.rs`
- Create: `tests/undo.rs`
- Create: `tests/rollback.rs`
- Create: `tests/restore.rs`
- Create: `tests/remote.rs`
- Create: `tests/doctor.rs`
- Create: `tests/adopt.rs`
- Create: `tests/where.rs`

**What and why:** End-to-end tests using `assert_cmd` subprocesses. Each test sets up a temp directory as `ROOST_DIR`, runs `roost` commands, and asserts output/exit codes/state.

**Test pattern:**
```rust
use assert_cmd::Command;
use tempfile::TempDir;

#[test]
fn roost_init_creates_config() {
    let tmp = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("roost").unwrap();
    cmd.env("ROOST_DIR", tmp.path())
       .arg("init")
       // ... pipe input for dialoguer prompts or use --non-interactive flag
       .assert()
       .success();
    
    assert!(tmp.path().join("roost.toml").exists());
    assert!(tmp.path().join("local.toml").exists());
    assert!(tmp.path().join(".gitignore").exists());
}
```

**Consideration:** Dialoguer prompts are interactive. For testing, either:
- Add a `--non-interactive` flag to `roost init` that accepts all defaults
- Or pipe input via stdin

This is a design decision to make during implementation.

**Tests per file (roughly):**
- `init.rs`: creates config, creates git repo, rejects re-init, resumes from partial
- `add.rs`: adds file, adds directory, rejects non-existent, adds to correct profile
- `remove.rs`: removes app, restores files, rejects unknown app
- `status.rs`: shows linked, shows broken, shows missing
- `sync.rs`: syncs clean repo, detects conflicts (requires mock or skip)
- `save.rs`: commits changes, no-op when clean
- `profile.rs`: list, switch, add, delete, rename — all 5 subcommands
- `diff.rs`: shows uncommitted changes, empty when clean
- `log.rs`: shows commits, respects -n limit
- `undo.rs`: undoes 1 commit, undoes N commits, rejects clean repo
- `rollback.rs`: resets to hash, rejects invalid hash
- `restore.rs`: restores app files, creates symlink
- `remote.rs`: shows remote, sets remote
- `doctor.rs`: reports broken links, reports inconsistencies, reports clean
- `adopt.rs`: adopts orphaned apps, skips already-managed
- `where.rs`: shows path for dir app, shows path for file app, rejects unknown

- [ ] **Step 1: User stubs test files with test function signatures**
- [ ] **Step 2: Agent fills test implementations (batch per file)**
- [ ] **Step 3: Run `cargo test` — all integration tests pass**
- [ ] **Step 4: Commit**

```bash
git add tests/
git commit -m "test: integration tests for all subcommands"
```

- [ ] **Step 5: Recap — user walks through the test strategy**

---

## Task 11: Polish & Edge Cases

**Files:**
- Modify: various files as needed

**What and why:** Address edge cases that surface during integration testing.

- Error messages: ensure all errors are human-readable (color-eyre helps here, but verify messages are actionable)
- Missing `git` binary: detect early, surface clear error ("git is required for sync/log/diff/undo/rollback")
- Missing `$EDITOR`: default to `vi`, surface warning if not found
- Empty profiles: handle profile with no apps gracefully
- Corrupted config: load returns actionable error with file path
- Permission errors on symlink creation: surface with instructions

- [ ] **Step 1: Run full test suite, identify failures**
- [ ] **Step 2: Fix edge cases**
- [ ] **Step 3: Run `cargo test` — all tests pass**
- [ ] **Step 4: Run `cargo clippy` — no warnings**
- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "fix: polish edge cases and error messages"
```

---

## Dependency Graph (execution order)

```
Task 1: Scaffolding
  ↓
Task 2: app/ (data models + config)
  ↓
Task 3: os_detect
  ↓
Task 4: scanner/ (depends on data/)
  ↓
Task 5: linker/ (depends on app/)
  ↓
Task 6: git/
  ↓
Task 7: logo + pager
  ↓
Task 8: init.rs (depends on app/, scanner/, linker/, git/)
  ↓
Task 9: main.rs (depends on everything)
  ↓
Task 10: Integration tests
  ↓
Task 11: Polish
```

Tasks 3, 7 can run in parallel after Task 2. Tasks 4, 5, 6 can partially overlap (each is independent). Task 8 requires 4+5+6 complete. Task 9 requires 8. Task 10 requires 9.

---

## What This Plan Does NOT Cover (Phase 2)

- TUI (onboarding and main view)
- Miller columns
- Dialog overlays (search, confirm, help, ignore, profile, git log, undo, app link)
- Fuzzy search widget
- Suspend/resume for $EDITOR in TUI context
- Dirty indicator in status bar
- Any `ratatui`/`crossterm` code
