# Plan: Roost Main TUI + Backend Hardening

## Overview

Replace the `roost` no-arg placeholder with a full daily-use TUI, then close remaining backend gaps and integration test coverage. Ordered for maximum testability and minimal risk.

---

## Stream 1: Suspend/Resume Infrastructure

**Goal:** Generic helper to leave alternate screen, run an external command, and restore the TUI.

**Why first:** Main TUI needs this for `$EDITOR`, `$PAGER`, and `git` operations before any rendering code can call them.

### Shell Overlay vs. Drop-to-Terminal

The user asked whether we should render git inside the TUI via a "shell overlay" that forwards I/O between the user and a real shell. Let's evaluate both approaches:

| Approach | Pros | Cons |
|----------|------|------|
| **Drop-to-terminal** (SPEC choice) | Simple, robust, interactive git (credentials, merge conflicts, `$EDITOR`) just works, matches SPEC | Jarring context switch, TUI disappears temporarily |
| **Shell overlay in TUI** | Visually integrated, no context switch | Requires PTY management (platform-dependent, complex), interactive git still needs a real terminal anyway, significantly more code |

**Decision:** Follow the SPEC's suspend/resume approach for the MVP. Drop to the real terminal for all external tools. A shell overlay is a valid future enhancement but requires PTY plumbing and does not solve the interactive git problem — credentials, rebase conflicts, and commit-message `$EDITOR` all need a real TTY. We will keep the door open by making `suspend_and_run` generic enough that a future overlay could replace it for read-only commands.

### Implementation

**Files to create/modify:**
- `src/tui/suspend.rs` — `suspend_and_run<F, T>(f: F) -> Result<T>` helper
  - Disables raw mode, leaves alternate screen, runs closure, restores on return
  - Handles panics during suspension (restore terminal before unwinding)
  - Returns the closure's result so callers can handle errors

**Usage pattern:**
```rust
// From main TUI — open file in $EDITOR
let path = ...;
suspend_and_run(|| {
    std::process::Command::new(std::env::var("EDITOR").unwrap_or("vi"))
        .arg(&path)
        .status()
})?;

// From diff viewer — pipe to $PAGER
suspend_and_run(|| {
    pager::open(&diff_output)
})?;

// From sync action
suspend_and_run(|| {
    git::sync(&roost_dir, &preference)
})?;
```

**Checkpoint:** A test that spawns `echo hello` via the helper and asserts it returns `Ok(())`.

---

## Stream 2: Main TUI Core (`tui/main_view/`)

**Goal:** The primary interface launched by `roost` with no args.

**Architecture:** Elm-style per SPEC — separate `state.rs`, `event.rs`, `ui.rs` + `mod.rs` entry point. This is a departure from the monolithic `init_tui.rs`, but it keeps the main view (which will be large) maintainable.

### 2a: `tui/main_view/mod.rs`
- `pub fn run(roost_dir: PathBuf, shared: SharedAppConfig, local: LocalAppConfig) -> Result<()>`
- Terminal setup/restore (extracted pattern from `init_tui.rs`)
- Panic hook + Ctrl+C handler (global `SHOULD_EXIT`)
- Main event loop: `draw` → `poll` → `handle_event` → `process_actions`
- `pending_auto_commit` batching: after event processing, if `pending_auto_commit` is `Some(msg)`, call `git::save()`

### 2b: `tui/main_view/state.rs`
```
struct MainViewState {
    roost_dir: PathBuf,
    shared: SharedAppConfig,
    local: LocalAppConfig,
    
    // Panels
    focus: Focus,           // AppsPanel | FilesPanel
    app_cursor: usize,
    app_scroll: usize,
    miller: MillerColumns,  // initialized to active profile dir
    
    // Overlays (flat Option priority stack)
    confirm_dialog: Option<ConfirmDialog>,
    search: Option<SearchState>,       // fuzzy search overlay
    help_dialog: Option<HelpState>,
    profile_dialog: Option<ProfileState>,
    ignore_dialog: Option<IgnoreState>,
    git_log_dialog: Option<GitLogState>,
    undo_dialog: Option<UndoState>,
    app_link_dialog: Option<AppLinkState>,
    diff_view: Option<DiffViewState>,
    
    // Meta
    status_message: Option<String>,
    pending_auto_commit: Option<String>,
    quit: bool,
}
```

**Key behaviors:**
- Left panel: list of apps in active profile, sorted alphabetically. `★` marks primary config set. `←source` marks cross-profile linked apps.
- Right panel: Miller columns rooted at `~/.roost/<active_profile>/`. For the selected app, show its files. `h/l` navigate miller columns.
- `Tab` switches focus between left apps panel and right miller panel.
- `j/k` navigate within the focused panel.
- `/` opens fuzzy search (searches app names when apps panel is focused, file names when files panel is focused).
- `q` / `Esc` quits (with confirm if pending_auto_commit or unsaved changes).

### 2c: `tui/main_view/event.rs`
- `fn handle_event(state: &mut MainViewState, key: KeyEvent) -> Vec<Action>`
- Returns `Vec<Action>` because one keypress can trigger multiple side effects (e.g., `x` → confirm dialog → on confirm → remove app + set pending_auto_commit)
- Dialog routing: check overlay `Option`s in priority order (confirm first, then search, then others). If any is `Some`, route key to that dialog's handler.
- `Action` enum: `Quit`, `SetStatus(String)`, `AutoCommit(String)`, `RemoveApp(String)`, `OpenEditor(PathBuf)`, `OpenPager(String)`, `Sync`, `SwitchProfile(String)`, etc.

### 2d: `tui/main_view/ui.rs`
- `fn render(state: &mut MainViewState, frame: &mut Frame)`
- Header: `roost · profile: <name>  N apps managed`
- Left apps panel (24 chars or 30% width): `List` widget with custom styling
- Right panel (remaining width): `MillerColumns` widget
- Status bar: context-sensitive keys. Different text when apps panel is focused vs files panel is focused.
- Dialog overlays: each dialog has its own `render_*` function, centered with `Clear` behind.

**Checkpoint:** `cargo run` with no args launches the TUI, shows the active profile name, app list, and miller columns. `q` quits cleanly. Tests pass.

---

## Stream 3: Dialog System (`tui/main_view/dialogs/`)

**Goal:** Implement the 7 missing dialog overlays on top of Main TUI core.

**Files to create:**
- `src/tui/main_view/dialogs/mod.rs` — re-exports
- `src/tui/main_view/dialogs/help.rs` — Searchable keybind reference. Static data + FuzzyEngine over keybind strings. Yellow/Cyan border.
- `src/tui/main_view/dialogs/ignore.rs` — Add/remove ignore patterns. Two modes (Tab to cycle): type to add, list to remove. Yellow border.
- `src/tui/main_view/dialogs/profile.rs` — Switch/create/delete profiles. Three modes (Tab): list and pick, type new name (+ choose [current]/[empty]), select and confirm delete. Yellow border for switch/create, Red for delete.
- `src/tui/main_view/dialogs/git_log.rs` — Git history browser. Uses `git::log(roost_dir, 50)` to populate a `List`. `r` on a commit opens rollback confirm (Red). Yellow border.
- `src/tui/main_view/dialogs/undo.rs` — Simple confirm: "Undo last commit?" Red border. On confirm, calls `git::undo(roost_dir, 1)`.
- `src/tui/main_view/dialogs/app_link.rs` — Import/paste multi-step wizard. Cyan border for import, Yellow for paste. Step 1: pick profile from list. Step 2: pick app from that profile.
- `src/tui/main_view/dialogs/diff_view.rs` — Captures `git::diff(roost_dir)` output, renders in scrollable view. Yellow border. `e` to open in `$PAGER` via suspend.

**Key behaviors:**
- Only one dialog active at a time (flat `Option` priority stack checked in fixed order)
- `Esc` cancels current dialog (or triggers quit confirm if at base state)
- `y/n` for confirm dialogs
- `Tab` cycles modes inside multi-mode dialogs (Ignore, Profile)
- `j/k` navigate lists within dialogs
- Typing filters/searchs where applicable

**Checkpoint:** All dialogs render correctly, handle input, and call the right backend functions. No dead code.

---

## Stream 4: Backend Hardening

### 4a: Tilde-path serde module
**Task:** Custom serde for `PathBuf` that serializes as `~/...` and deserializes using current device's home directory.

**Why:** SPEC requires `primary_config` and other paths in `roost.toml` to be portable across devices with different home directory paths.

**Approach:**
- `src/app/tilde_serde.rs` — `mod tilde_serde` with `serialize_path` and `deserialize_path` functions
- Use `dirs::home_dir()` to resolve `~` on deserialize
- Strip home prefix and prepend `~` on serialize
- Apply to `Application::primary_config` field via `#[serde(with = "tilde_serde")]`

**Tradeoff:** This changes the TOML output format. Existing `roost.toml` files without tildes will still load (absolute paths deserialize as-is), but newly saved files will use tildes. Backward-compatible.

### 4b: Config migration
**Task:** Implement `migrate_shared()` stub.

**Why:** Old `roost.toml` formats must load cleanly. SPEC mentions dual-format `apps` field and legacy `link_path` -> `link_paths`.

**Approach:**
- Parse raw TOML as `toml::Value`
- Detect old format: `apps` as array of strings instead of table
- Detect legacy `link_path` at root level
- Transform to new format in-memory before deserializing to `SharedAppConfig`

**Tradeoff:** Migration runs on every load. Minimal overhead since configs are small. Could cache a "migrated" flag in the file comment, but not worth the complexity.

### 4c: Git push in sync
**Task:** Add `git push origin main` after successful rebase in `git::sync()`.

**Why:** SPEC explicitly says "Auto-commit + `git pull --rebase` + `git push`". Current implementation fetches and rebases but never pushes.

**Approach:**
- After successful rebase (no conflicts), run `git push origin main`
- If push fails (e.g., non-fast-forward), surface error to caller
- `SyncResult` already has variants for conflicts; add a `PushFailed` variant if needed

### 4d: Concurrency protection
**Task:** Atomic config writes or file locking.

**Why:** Multiple roost processes (or CLI + TUI simultaneously) could corrupt `roost.toml` or `local.toml`.

**Approach:**
- Write to temp file (`roost.toml.tmp`), then `fs::rename()` to final path. This is atomic on POSIX and modern Windows.
- No file locking for MVP — atomic writes are sufficient for the common case.

**Tradeoff:** True file locking (`flock`) prevents concurrent reads too, which is overkill. Atomic rename is the standard approach for config files.

### 4e: File preview in Miller columns
**Task:** Inline content preview for files in the Miller column preview pane.

**Why:** SPEC says "File preview: inline content for files, children for directories." Current `MillerColumns` shows `(file)` for files.

**Approach:**
- In `miller.rs` preview pane: if hovered item is a file, read first N lines (e.g., 20) and render as gray text
- Skip binary files (check for null bytes in first 1KB)
- Cap line length to avoid horizontal overflow

**Tradeoff:** Reading file content on every cursor move could be slow for large files. Use a small read limit and cache the last previewed path.

**Checkpoint:** All backend changes compile, existing tests pass, new unit tests for tilde serde and migration pass.

---

## Stream 5: Integration Test Coverage

**Goal:** Fill the two remaining integration test gaps.

### 5a: `tests/sync.rs`
- Test `roost sync` in a temp directory with a configured remote
- Verify it commits uncommitted changes, pulls, rebases, and pushes
- Test conflict resolution preference behavior

**Challenge:** `sync` requires a real git remote. Use `git init --bare` in a second temp dir as the remote, then `git remote add origin <bare-dir>`.

### 5b: `tests/init.rs`
- Test `roost init` wizard with mock inputs
- Since `roost init` uses `dialoguer` (interactive prompts), we need to test the backend functions directly or use `std::process::Command` with piped stdin
- Alternatively, test `init_tui.rs` directly via its `run()` function with test data

**Tradeoff:** Testing interactive CLI prompts is hard. Focus on testing the post-init state: after running init (or its component functions), assert that `roost.toml`, `local.toml`, `.gitignore`, and git repo exist and have correct content.

**Checkpoint:** `cargo test` passes with 100+ total tests.

---

## Implementation Order Within Streams

For each stream, recommended order to maximize reviewability under code-ownership:

1. **Stream 1 (Suspend):** Write `suspend.rs` stub → I fill implementation → you review → tests
2. **Stream 2 (Main TUI):** 
   - You stub `main_view/mod.rs`, `state.rs`, `event.rs`, `ui.rs` (empty functions, struct definitions)
   - I implement one file at a time, starting with `state.rs` → `event.rs` → `ui.rs` → `mod.rs`
   - Recap after each file
3. **Stream 3 (Dialogs):**
   - You pick one dialog to stub first (e.g., `help.rs` — simplest)
   - I implement it, you review, then move to next
4. **Stream 4 (Backend):**
   - These are smaller, independent tasks. Can be done in parallel with Stream 2/3 if desired.
5. **Stream 5 (Tests):**
   - Write after features stabilize, or alongside each stream for TDD

---

## Open Questions Before Starting

1. **TUI file structure:** Elm-style 4-file split (`mod.rs`, `state.rs`, `event.rs`, `ui.rs`) for `main_view/`, or monolithic like `init_tui.rs`?
2. **Dialog state:** Flat `Option<T>` fields (simpler, matches init_tui.rs) or state machine enum (type-safe)?
3. **Suspend scope:** All external tools at once, or `$EDITOR` first then `$PAGER`/`git`?
4. **Mode preference:** Default (you stub, I fill) or Guided (I explain shape, you direct scope, I write mechanical parts) for each stream?
