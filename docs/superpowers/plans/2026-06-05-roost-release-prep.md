# Roost v0.2.0 Release Preparation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix all critical bugs, security flaws, and release blockers identified in the comprehensive codebase audit, then prepare the repository for public release.

**Architecture:** We fix backend safety issues first (atomic writes, path validation), then TUI correctness bugs (RemoveApp, profile switch, rendering), then resource management (panic hooks, Ctrl-C, previews), then release metadata (README, LICENSE, Cargo.toml), then testing gaps, and finally backend polish.

**Tech Stack:** Rust 2024, ratatui 0.30, crossterm 0.29, color-eyre, clap 4, git CLI

---

## Phase 1: Critical TUI Correctness Bugs

These bugs actively break user trust or core functionality.

### Task 1: Fix `Action::RemoveApp` in TUI

**Files:**
- Modify: `src/tui/main_view/mod.rs:179-181`
- Read: `src/main.rs:cmd_remove` (to mirror logic)
- Test: `tests/tui_remove.rs` (new integration test)

**Context:** `process_action` for `Action::RemoveApp` currently only sets a status message. It must actually unlink symlinks, remove the app from `shared.apps`, remove from all profile `apps` sets, save configs, and set `pending_auto_commit`.

- [ ] **Step 1: Read `cmd_remove` in `src/main.rs`** to understand the exact logic sequence.

- [ ] **Step 2: Implement `Action::RemoveApp` in `src/tui/main_view/mod.rs`**

Replace the stub at lines 179-181. The correct sequence mirrors `cmd_remove`:

```rust
Action::RemoveApp => {
    let app_name = state.selected_app().to_string();
    let Some(app) = state.shared.apps.get(&app_name) else {
        state.status_message = Some(format!("App '{}' not found", app_name));
        return Action::None;
    };
    let Some(origin) = state.local.link_paths.get(&app_name) else {
        state.status_message = Some(format!("No link path for '{}'", app_name));
        return Action::None;
    };
    let profile_name = state.local.active_profile.clone();
    let pdir = app::profile_dir(&state.roost_dir, &profile_name);

    // Unlink: remove symlink and move roost files back to origin
    if let Err(e) = linker::unlink(origin, &pdir, &app_name, app.is_dir) {
        state.status_message = Some(format!("Error removing {}: {}", app_name, e));
        return Action::None;
    }

    // Remove from shared config
    state.shared.apps.remove(&app_name);
    // Remove from active profile
    if let Some(profile) = state.shared.profiles.get_mut(&profile_name) {
        profile.apps.remove(&app_name);
    }
    // Remove link path from local config
    state.local.link_paths.remove(&app_name);

    // Save configs
    if let Err(e) = app::save_shared(&state.roost_dir, &state.shared) {
        state.status_message = Some(format!("Error saving config: {}", e));
        return Action::None;
    }
    if let Err(e) = app::save_local(&state.roost_dir, &state.local) {
        state.status_message = Some(format!("Error saving local config: {}", e));
        return Action::None;
    }
    state.pending_auto_commit = true;
    state.status_message = Some(format!("Removed '{}'", app_name));
    state.select_next_app();
    Action::None
}
```

**Note:** `linker::unlink` already restores files — it removes the symlink and `fs::rename`s the managed files from roost back to the original origin path. This is the inverse of `ingest`.

- [ ] **Step 3: Run `cargo test`** — verify no regressions.

- [ ] **Step 4: Commit**

```bash
git add src/tui/main_view/mod.rs
git commit -m "fix(tui): implement RemoveApp action properly

Previously Action::RemoveApp was a stub that only showed a status
message without actually removing the app, unlinking symlinks, or
updating configs. Now it mirrors the CLI remove logic."
```

---

### Task 2: Fix Profile Switch in TUI to Update Symlinks

**Files:**
- Modify: `src/tui/main_view/mod.rs:288-298`
- Read: `src/main.rs:cmd_profile` (for correct switch logic)
- Test: existing integration tests (`tests/profile.rs`)

**Context:** `Action::SwitchProfile` updates `local.active_profile` in memory but never calls `linker::switch_profile()`. The CLI correctly does this.

- [ ] **Step 1: Read `cmd_profile` in `src/main.rs` lines ~477-489** to see the `switch_profile` call sequence.

- [ ] **Step 2: Modify `Action::SwitchProfile` in `src/tui/main_view/mod.rs`**

Replace lines 288-298 with:

```rust
Action::SwitchProfile(name) => {
    if state.local.active_profile == name {
        return Action::None;
    }
    if !state.shared.profiles.contains_key(&name) {
        state.status_message = Some(format!("Profile '{}' does not exist", name));
        return Action::None;
    }
    let old_profile = state.local.active_profile.clone();
    state.local.active_profile = name.clone();
    if let Err(e) = app::save_local(&state.roost_dir, &state.local) {
        state.status_message = Some(format!("Error saving local config: {}", e));
        state.local.active_profile = old_profile;
        return Action::None;
    }
    // Actually update symlinks on disk
    if let Err(e) = linker::switch_profile(&old_profile, &name, &state.shared, &state.local, &state.roost_dir) {
        state.status_message = Some(format!("Error switching profile: {}", e));
        state.local.active_profile = old_profile;
        let _ = app::save_local(&state.roost_dir, &state.local);
        return Action::None;
    }
    state.status_message = Some(format!("Switched to profile: {}", name));
    state.select_next_app();
    Action::None
}
```

- [ ] **Step 3: Run `cargo test`**

- [ ] **Step 4: Commit**

```bash
git add src/tui/main_view/mod.rs
git commit -m "fix(tui): profile switch now updates symlinks on disk

Action::SwitchProfile was only updating local.toml in memory but
never calling linker::switch_profile(), so symlinks stayed pointing
to the old profile."
```

---

### Task 3: Fix `←` Cross-Profile Source Marker Rendering

**Files:**
- Modify: `src/tui/main_view/ui.rs:131-135`

**Context:** `render_apps_panel` computes `source` but it is an expression statement — the result is dropped. It should be used in the `Line::from` vector.

- [ ] **Step 1: Read the current code around line 131 in `src/tui/main_view/ui.rs`**

Current buggy code looks like:
```rust
let source = state
    .shared
    .profiles
    .get(&state.local.active_profile)
    .and_then(|p| p.app_sources.get(name))
    .map(|_| "← ");
let line = Line::from(vec![
    Span::raw(cursor_prefix),
    Span::styled(truncate_str(name, inner.width as usize - 6), style),
]);
```

- [ ] **Step 2: Fix the rendering to include the source prefix**

Replace with:
```rust
let source_prefix = state
    .shared
    .profiles
    .get(&state.local.active_profile)
    .and_then(|p| p.app_sources.get(name))
    .map(|_| "← ")
    .unwrap_or("  ");
let max_name_width = inner.width.saturating_sub(4) as usize; // cursor + source + spacing
let line = Line::from(vec![
    Span::raw(cursor_prefix),
    Span::raw(source_prefix),
    Span::styled(truncate_str(name, max_name_width), style),
]);
```

- [ ] **Step 3: Run `cargo test`**

- [ ] **Step 4: Commit**

```bash
git add src/tui/main_view/ui.rs
git commit -m "fix(tui): render cross-profile source marker (←) in apps panel

The source prefix was computed but discarded as an expression
statement. Now it is bound and included in the rendered line."
```

---

### Task 4: Fix Search Popup Hiding Match Count

**Files:**
- Modify: `src/tui/main_view/ui.rs:298-300`
- Modify: `src/app_selector.rs:831`

**Context:** `popup_height = 3` leaves 0 usable inner rows after borders. The match count hint is never rendered.

- [ ] **Step 1: Fix `ui.rs`**

Change line 298-300 from:
```rust
let popup_height = 3;
```
to:
```rust
let popup_height = 4;
```

- [ ] **Step 2: Fix `app_selector.rs`**

Same change at line 831:
```rust
let popup_height = 4;
```

- [ ] **Step 3: Run `cargo test`**

- [ ] **Step 4: Commit**

```bash
git add src/tui/main_view/ui.rs src/app_selector.rs
git commit -m "fix(tui): increase search popup height so match count is visible

popup_height=3 left 0 inner rows after borders, so '3 matches' /
'no matches' hints were never rendered. Changed to 4 in both the
main TUI and app selector."
```

---

### Task 5: Fix Help Text Key Bindings

**Files:**
- Modify: `src/tui/main_view/dialogs/help.rs:105-108` and surrounding entries

**Context:** Help says `s` = Sync but `s` actually saves. `S` (shift+s) is sync. Also `r` is documented globally but only works in git log dialog. Rollback is labeled "destructive" but uses `safe_rollback`.

- [ ] **Step 1: Read `KEYBINDS` array in `help.rs`**

- [ ] **Step 2: Fix the entries**

Replace/add entries:
```rust
// Around line 100-108, change:
KeybindEntry { key: "s", description: "Sync with remote" },
// to:
KeybindEntry { key: "s", description: "Save changes (git commit)" },
KeybindEntry { key: "S", description: "Sync with remote (pull/push)" },

// Around line 122-124, change:
KeybindEntry { key: "r", description: "Rollback to selected commit (destructive)" },
// to:
KeybindEntry { key: "r", description: "Rollback to selected commit (git log dialog only)" },

// Ensure the Actions section (if it exists) or Global section is accurate.
```

Also check that `e / Enter` help indicates it only applies in Files panel, or add a note.

- [ ] **Step 3: Run `cargo test`**

- [ ] **Step 4: Commit**

```bash
git add src/tui/main_view/dialogs/help.rs
git commit -m "fix(tui): correct help text key bindings

- s is Save, S is Sync
- Remove '(destructive)' from rollback description since we use
  safe_rollback which preserves new apps and creates a forward commit
- Clarify that 'r' only works inside the git log dialog"
```

---

### Task 6: Fix Unchecked Hash Slicing Panics

**Files:**
- Modify: `src/tui/main_view/event.rs:495`, `733`, `781`

**Context:** `&commits[0].hash[..7]` and `&hash[..7]` panic if hash < 7 chars. Line 582 already does it correctly with `.min(7)`.

- [ ] **Step 1: Find all instances in `event.rs`**

Search for patterns like `[..7]`.

- [ ] **Step 2: Replace each with safe slicing**

For example:
```rust
// Instead of:
let short_hash = &commits[0].hash[..7];
// Use:
let short_hash = &commits[0].hash[..commits[0].hash.len().min(7)];
```

Do the same for any other occurrences (`hash[..7]` at lines 733, 781).

- [ ] **Step 3: Run `cargo test`**

- [ ] **Step 4: Commit**

```bash
git add src/tui/main_view/event.rs
git commit -m "fix(tui): prevent panic on short git hashes in event handling

Replace unchecked &hash[..7] with hash[..hash.len().min(7)] to
avoid panics if a git hash is ever shorter than 7 characters."
```

---

### Task 7: Fix Status Message Cleared on Every Keypress

**Files:**
- Modify: `src/tui/main_view/event.rs:136-143`

**Context:** `state.status_message = None` is at the top of `handle_base`, so any keypress clears it. Users can't read error messages.

- [ ] **Step 1: Read `handle_base` in `event.rs`**

- [ ] **Step 2: Move the clear into action-key match arms only**

Instead of clearing at the top, clear it only when an actual action is taken. For example, in the `match key` block, set `state.status_message = None` inside the arms that perform navigation or actions, but **not** at the very top of the function. Be careful to preserve behavior for the search `/` key and other mode transitions.

A safe approach: Remove the blanket `state.status_message = None;` at the top, and add it at the start of the outer `match` for actual action keys (j, k, h, l, o, x, etc.), but not for `?` (help) or `/` (search) which are overlays.

- [ ] **Step 3: Run `cargo test`**

- [ ] **Step 4: Commit**

```bash
git add src/tui/main_view/event.rs
git commit -m "fix(tui): don't clear status message on every keypress

Previously any keypress immediately erased the status message,
preventing users from reading error toasts. Now it is only cleared
when an actual action is dispatched."
```

---

## Phase 2: Security & Safety

### Task 8: Fix Atomic Write Concurrency

**Files:**
- Modify: `src/app/mod.rs:141-145`
- Modify: `src/gitignore.rs:71-73`
- Test: `src/app/tests.rs` (add concurrency test or at least verify behavior)

**Context:** `atomic_write` uses a fixed `.tmp` extension. Two concurrent roost processes clobber each other.

- [ ] **Step 1: Read current `atomic_write` in `src/app/mod.rs`**

- [ ] **Step 2: Use a randomized temp filename**

Replace `atomic_write`:
```rust
use std::time::{SystemTime, UNIX_EPOCH};

fn atomic_write(path: &std::path::Path, contents: &str) -> eyre::Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let pid = std::process::id();
    let tmp = path.with_extension(format!("tmp.{pid}.{now}"));
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
```

- [ ] **Step 3: Apply same fix to `src/gitignore.rs:71-73`**

```rust
let now = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_millis();
let pid = std::process::id();
let tmp = path.with_extension(format!("tmp.{pid}.{now}"));
std::fs::write(&tmp, contents)?;
std::fs::rename(&tmp, path)?;
```

- [ ] **Step 4: Run `cargo test`**

- [ ] **Step 5: Commit**

```bash
git add src/app/mod.rs src/gitignore.rs
git commit -m "fix: make config writes concurrency-safe with unique temp filenames

Previously atomic_write used a fixed .tmp extension, so two
concurrent roost processes could clobber each other. Now temp
filenames include PID and millisecond timestamp for uniqueness."
```

---

### Task 9: Add Path Traversal Validation

**Files:**
- Modify: `src/linker/mod.rs:13-44` (`ingest`)
- Modify: `src/main.rs:328-384` (`cmd_add`)
- Modify: `src/init.rs:176-181`
- Test: `src/linker/tests.rs` (add rejection test)

**Context:** No validation prevents `roost add /etc/passwd` or path traversal via `..`.

- [ ] **Step 1: Create a helper function `validate_path_in_home`**

Add to `src/linker/mod.rs` or `src/app/mod.rs`:
```rust
use std::path::{Path, PathBuf};
use eyre::{bail, Result};

/// Ensure a path is within the user's home directory, rejecting
/// absolute paths outside home and paths with `..` components.
pub fn validate_path_in_home(path: &Path, home: &Path) -> Result<PathBuf> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let canonical_home = home.canonicalize().unwrap_or_else(|_| home.to_path_buf());
    
    if !canonical.starts_with(&canonical_home) {
        bail!("Path '{}' is outside the home directory ({})", path.display(), home.display());
    }
    
    // Also reject if any component is ParentDir after resolution
    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            bail!("Path '{}' contains parent directory references (..)", path.display());
        }
    }
    
    Ok(canonical)
}
```

- [ ] **Step 2: Call validation in `linker::ingest` before moving files**

In `src/linker/mod.rs`, at the top of `ingest`, call `validate_path_in_home` on `origin` using the roost dir's parent (home) or `dirs::home_dir()`.

- [ ] **Step 3: Call validation in `cmd_add` (`src/main.rs:341`)**

Before creating the app name, validate the path:
```rust
let home = dirs::home_dir().ok_or_else(|| eyre::eyre!("Could not determine home directory"))?;
linker::validate_path_in_home(path, &home)?;
```

- [ ] **Step 4: Call validation in `init.rs`**

Where scan results are ingested.

- [ ] **Step 5: Add unit test in `src/linker/tests.rs`**

```rust
#[test]
fn test_validate_path_rejects_outside_home() {
    let home = PathBuf::from("/home/user");
    let bad = Path::new("/etc/passwd");
    assert!(linker::validate_path_in_home(bad, &home).is_err());
}

#[test]
fn test_validate_path_rejects_parent_dir() {
    let home = PathBuf::from("/home/user");
    let bad = Path::new("/home/user/../etc/passwd");
    assert!(linker::validate_path_in_home(bad, &home).is_err());
}
```

- [ ] **Step 6: Run `cargo test`**

- [ ] **Step 7: Commit**

```bash
git add src/linker/mod.rs src/main.rs src/init.rs src/linker/tests.rs
git commit -m "security: validate ingest paths are within home directory

Rejects absolute paths outside home and paths containing ..
components before ingest/restore operations. Prevents a malicious
roost.toml from causing sensitive file overwrites."
```

---

### Task 10: Sanitize App Names

**Files:**
- Modify: `src/main.rs:341-347`
- Modify: `src/init.rs:165-174`
- Modify: `src/tui/main_view/mod.rs:430-439`
- Test: `src/app/tests.rs`

**Context:** App names derived from filenames are not sanitized for `/` or other invalid chars.

- [ ] **Step 1: Add a helper `sanitize_app_name`**

In `src/app/mod.rs` (or a new `src/util.rs`):
```rust
/// Replaces path separators and control characters in app names.
pub fn sanitize_app_name(name: &str) -> String {
    name.replace(['/', '\\', '\0'], "_")
}
```

- [ ] **Step 2: Apply in `cmd_add`, `init.rs`, and `tui/main_view/mod.rs`**

Wrap the derived app name:
```rust
let app_name = app::sanitize_app_name(&file_name);
```

- [ ] **Step 3: Add unit test**

```rust
#[test]
fn test_sanitize_app_name() {
    assert_eq!(app::sanitize_app_name("foo/bar"), "foo_bar");
    assert_eq!(app::sanitize_app_name("foo\\bar"), "foo_bar");
    assert_eq!(app::sanitize_app_name("normal"), "normal");
}
```

- [ ] **Step 4: Run `cargo test`**

- [ ] **Step 5: Commit**

```bash
git add src/app/mod.rs src/main.rs src/init.rs src/tui/main_view/mod.rs src/app/tests.rs
git commit -m "security: sanitize app names to prevent path injection

App names derived from filenames could contain / or \\, which were
used directly in path construction. Now they are sanitized to _."
```

---

### Task 11: Add Profile Deletion Confirmation Dialog

**Files:**
- Modify: `src/tui/main_view/event.rs:603-618`
- Read: `src/tui/main_view/mod.rs` (how RemoveApp triggers ConfirmDialog)

**Context:** Deleting a profile fires immediately on Enter. No confirmation.

- [ ] **Step 1: Read how `Action::RemoveApp` triggers a confirm dialog**

Look at `event.rs` for the pattern: setting `state.confirm_dialog` before dispatching `Action::RemoveApp`.

- [ ] **Step 2: Wire profile delete to use ConfirmDialog::destructive**

In the profile dialog handler (around `event.rs:603-618`), instead of directly dispatching `Action::DeleteProfile`, set:
```rust
state.confirm_dialog = Some(ConfirmDialog::destructive(
    format!("Delete profile '{}'?", profile_name),
    Action::DeleteProfile(profile_name),
));
return Action::None;
```

- [ ] **Step 3: Update `Action::DeleteProfile` handler to skip re-confirmation**

Ensure `process_action` for `DeleteProfile` does not create another confirm dialog (it currently doesn't, but verify).

- [ ] **Step 4: Run `cargo test`**

- [ ] **Step 5: Commit**

```bash
git add src/tui/main_view/event.rs
git commit -m "fix(tui): add confirmation dialog before profile deletion

Profile deletion was an immediate destructive action with no
confirmation. Now it uses ConfirmDialog::destructive, consistent
with RemoveApp."
```

---

## Phase 3: Resource Management & Edge Cases

### Task 12: Fix Panic Hook and Ctrl-C Handler Leaks

**Files:**
- Modify: `src/tui/main_view/mod.rs:49-54`, `58-60`
- Modify: `src/app_selector.rs:300-305`
- Modify: `src/init.rs:139-143`

**Context:** Original panic hook is captured but never restored. Ctrl-C handler is global and prevents second TUI run.

- [ ] **Step 1: Store original hook and restore on TUI exit**

In `src/tui/main_view/mod.rs`, capture the original hook into a local variable and restore it in a `Drop` guard or at the end of `run()`.

For example, before entering the loop:
```rust
let original_hook = std::panic::take_hook();
std::panic::set_hook(Box::new(|info| {
    // current cleanup logic...
}));
```

Then at the end of `run()`, before returning:
```rust
std::panic::set_hook(original_hook);
```

For Ctrl-C: The `ctrlc::set_handler` is tricky to unset. A simpler approach is to scope the handler to the TUI run and accept that it persists but is harmless (it just sets a flag). However, the second run failure is real.

A workaround: wrap `ctrlc::set_handler` in a `std::sync::Once` or use an `AtomicBool` that we check without re-registering. Or simply remove the `ctrlc` handler from `main_view` entirely since `crossterm` already handles `KeyCode::Char('c')` with modifiers? No, `ctrlc` handles SIGINT.

Better approach: Use a `static INIT: std::sync::Once = std::sync::Once::new();` to only register the handler once. The handler can set a flag that `run()` polls.

In `src/tui/main_view/mod.rs`:
```rust
static CTRLC_INIT: std::sync::Once = std::sync::Once::new();

// In run():
CTRLC_INIT.call_once(|| {
    let _ = ctrlc::set_handler(|| {
        SHOULD_EXIT.store(true, Ordering::SeqCst);
    });
});
```

Do the same for `app_selector.rs` and `init.rs`.

- [ ] **Step 2: Restore panic hook at end of run**

Add at the end of `run()` in `main_view/mod.rs`:
```rust
std::panic::set_hook(original_hook);
```

And in `app_selector.rs`.

- [ ] **Step 3: Run `cargo test`**

- [ ] **Step 4: Commit**

```bash
git add src/tui/main_view/mod.rs src/app_selector.rs src/init.rs
git commit -m "fix(tui): restore panic hook and scope ctrl-c handler registration

Previously the original panic hook was captured but never restored,
so hooks would stack on repeated TUI runs. The ctrl-c handler was
registered every run, causing the second run to fail. Now ctrl-c
uses Once::new() and the panic hook is restored on exit."
```

---

### Task 13: Fix Terminal.size() Failure Blocking Ctrl-C

**Files:**
- Modify: `src/tui/main_view/mod.rs:102-171`

**Context:** `SHOULD_EXIT` is checked after `terminal.size()`. If `size()` errors, `continue` loops forever without checking the exit flag.

- [ ] **Step 1: Check SHOULD_EXIT before terminal.size()**

Rearrange the loop in `run()`:

```rust
loop {
    if SHOULD_EXIT.load(Ordering::SeqCst) {
        break;
    }

    let size = match terminal.size() {
        Ok(s) => s,
        Err(_) => {
            std::thread::sleep(std::time::Duration::from_millis(50));
            continue;
        }
    };
    // ... rest of loop
}
```

- [ ] **Step 2: Run `cargo test`**

- [ ] **Step 3: Commit**

```bash
git add src/tui/main_view/mod.rs
git commit -m "fix(tui): check Ctrl-C exit flag before terminal.size()

If terminal.size() returned an error (e.g. detached terminal), the
loop would continue without ever checking SHOULD_EXIT, making it
impossible to quit with Ctrl-C."
```

---

### Task 14: Limit File Preview Reads

**Files:**
- Modify: `src/miller.rs:519-527`

**Context:** `std::fs::read(path)` loads the entire file into memory. For large files this is an OOM risk.

- [ ] **Step 1: Replace unbounded read with a size-capped read**

Replace:
```rust
let bytes = std::fs::read(path)?;
let mut bytes_clone = bytes.clone();
if String::from_utf8(bytes_clone).is_ok() {
    let text = String::from_utf8(bytes).unwrap();
    // ...
}
```

With a streaming/capped approach:
```rust
use std::io::Read;

const PREVIEW_MAX_BYTES: usize = 50 * 1024; // 50KB

let mut file = std::fs::File::open(path)?;
let mut buf = Vec::with_capacity(PREVIEW_MAX_BYTES);
let n = file.by_ref().take(PREVIEW_MAX_BYTES as u64).read_to_end(&mut buf)?;

if String::from_utf8(buf.clone()).is_ok() {
    let text = String::from_utf8(buf).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    // ...
} else {
    // binary
}

if n == PREVIEW_MAX_BYTES {
    // append a "... (truncated)" indicator
}
```

- [ ] **Step 2: Run `cargo test`**

- [ ] **Step 3: Commit**

```bash
git add src/miller.rs
git commit -m "fix(tui): cap file preview reads at 50KB to prevent OOM

Previously std::fs::read(path) loaded the entire file into memory.
For 100MB+ log files this could allocate ~300MB. Now we read at
most 50KB and show a truncation indicator."
```

---

### Task 15: Handle Unicode Display Width

**Files:**
- Modify: `src/tui/main_view/ui.rs:985-993` (`truncate_str`)
- Modify: `src/app_selector.rs:965-975` (`truncate_str` duplicate)
- Modify: `src/miller.rs:471-472`

**Context:** `chars().count()` and `chars().take()` truncate by character count, not display width. CJK characters (width 2) overflow columns.

- [ ] **Step 1: Add `unicode-width` to Cargo.toml**

```toml
unicode-width = "0.2"
```

- [ ] **Step 2: Replace both `truncate_str` duplicates with a single shared function**

Create `src/util.rs` (or add to `src/lib.rs`):
```rust
use unicode_width::UnicodeWidthStr;

pub fn truncate_str(s: &str, max_width: usize) -> String {
    if s.width() <= max_width {
        return s.to_string();
    }
    let mut width = 0;
    let mut result = String::new();
    for ch in s.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if width + ch_width > max_width.saturating_sub(1) {
            result.push('…');
            break;
        }
        width += ch_width;
        result.push(ch);
    }
    result
}
```

Remove the duplicate from `ui.rs` and `app_selector.rs`, and import from `crate::util::truncate_str`.

- [ ] **Step 3: Update `miller.rs` to use display width**

In `render_entries`, replace `display.chars().take(max_len)` with `truncate_str(display, max_len as usize)`.

- [ ] **Step 4: Update `app_selector.rs` scan list formatting**

Replace `format!("{:<20}", truncate_str(...))` with width-aware padding if possible, or at least ensure `truncate_str` uses display width.

- [ ] **Step 5: Run `cargo test`**

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml src/util.rs src/tui/main_view/ui.rs src/app_selector.rs src/miller.rs
git commit -m "fix(tui): respect Unicode display width in text truncation

Replaced char-count-based truncation with unicode-width crate.
CJK and other wide characters now correctly account for 2 display
cells. Also DRY'd the duplicate truncate_str functions into a
single shared utility."
```

---

### Task 16: Fix Status Bar Overflow on Narrow Terminals

**Files:**
- Modify: `src/tui/main_view/ui.rs:204-281`

**Context:** Status bar is ~180+ display cells wide. On 40-column minimum terminal, most keys are invisible.

- [ ] **Step 1: Truncate status bar dynamically based on available width**

The status bar is built as a `Vec<Span>`. Compute the total width, and if it exceeds `area.width`, drop lower-priority segments and append a `Span::styled(" …", Style::default().fg(Color::DarkGray))` to indicate truncation.

Alternatively, split into two lines when `area.width < 80`.

A simpler approach: Prioritize essential keys (`q quit`, `? help`, `j/k nav`) and drop the rest when width is tight.

- [ ] **Step 2: Run `cargo test`**

- [ ] **Step 3: Commit**

```bash
git add src/tui/main_view/ui.rs
git commit -m "fix(tui): truncate status bar on narrow terminals

The status bar was ~180 cells wide, so on the 40-column minimum
terminal most key hints were silently truncated. Now it drops
lower-priority hints and shows '…' when space is limited."
```

---

## Phase 4: Release Readiness

### Task 17: Add README.md

**Files:**
- Create: `README.md`

- [ ] **Step 1: Write README with sections:**
  - Project description (what is Roost)
  - Installation (`cargo install --path .` or future crates.io)
  - Quick start (`roost init`, `roost add`, `roost sync`)
  - Key concepts (profiles, apps, symlinks)
  - TUI usage summary
  - Shell completions (`roost completions bash > ...`)
  - License (MIT)

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: add README.md with install and quickstart guide"
```

---

### Task 18: Add LICENSE-MIT File

**Files:**
- Create: `LICENSE-MIT`

- [ ] **Step 1: Copy standard MIT license text**

```
MIT License

Copyright (c) 2026 [Author Name]

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

(Replace `[Author Name]` with actual author or "Roost contributors".)

- [ ] **Step 2: Commit**

```bash
git add LICENSE-MIT
git commit -m "docs: add MIT license file"
```

---

### Task 19: Populate Cargo.toml Metadata

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add fields**

```toml
[package]
name = "roost"
version = "0.2.0"
edition = "2024"
license = "MIT"
description = "A terminal-based dotfile manager with cross-device sync"
readme = "README.md"
repository = "https://github.com/[user]/roost"
keywords = ["dotfiles", "config", "sync", "terminal", "tui"]
categories = ["command-line-utilities", "config"]
authors = ["[Your Name] <[email]>"]
```

- [ ] **Step 2: Commit**

```bash
git add Cargo.toml
git commit -m "chore: add crates.io metadata to Cargo.toml

Add description, repository, readme, keywords, categories, and
authors fields required for publication."
```

---

### Task 20: Add Config Version Field

**Files:**
- Modify: `src/app/mod.rs` (`SharedAppConfig`)
- Modify: `src/app/tests.rs`
- Test: verify round-trip

**Context:** No version field means future format changes have no migration path.

- [ ] **Step 1: Add `version` field to `SharedAppConfig`**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SharedAppConfig {
    #[serde(default = "default_version")]
    pub version: String,
    pub remote: Option<String>,
    pub profiles: BTreeMap<String, Profile>,
    pub apps: BTreeMap<String, Application>,
    pub ignored: BTreeSet<String>,
}

fn default_version() -> String {
    "0.2.0".to_string()
}
```

- [ ] **Step 2: Update `Default` impl or construction sites**

Ensure `init.rs` and any test helpers set `version` or rely on the serde default.

- [ ] **Step 3: Run `cargo test`**

- [ ] **Step 4: Commit**

```bash
git add src/app/mod.rs src/app/tests.rs
git commit -m "feat(config): add version field to SharedAppConfig

Adds a 'version' key to roost.toml with a serde default of '0.2.0'.
This provides a migration hook for future format changes."
```

---

### Task 21: Make EDITOR/PAGER Platform-Aware

**Files:**
- Modify: `src/main.rs` (editor/pager defaults)
- Modify: `src/tui/main_view/mod.rs` (editor default)
- Modify: `src/pager.rs`

**Context:** `vi` and `less` are not standard on Windows.

- [ ] **Step 1: Create a helper for platform-aware defaults**

In `src/main.rs` (or new `src/util.rs`):
```rust
pub fn default_editor() -> &'static str {
    #[cfg(windows)]
    { "notepad" }
    #[cfg(not(windows))]
    { "vi" }
}

pub fn default_pager() -> &'static str {
    #[cfg(windows)]
    { "more" }
    #[cfg(not(windows))]
    { "less" }
}
```

- [ ] **Step 2: Replace all hardcoded `"vi"` and `"less"` references**

In `src/main.rs`, `src/tui/main_view/mod.rs`, and `src/pager.rs`, replace fallback strings with the helper.

- [ ] **Step 3: Run `cargo test`**

- [ ] **Step 4: Commit**

```bash
git add src/main.rs src/tui/main_view/mod.rs src/pager.rs
git commit -m "fix: use platform-aware defaults for EDITOR and PAGER

Defaults were hardcoded to vi and less, which don't exist on
Windows. Now uses notepad/more on Windows and vi/less elsewhere."
```

---

### Task 22: Replace home_dir().expect() with Graceful Fallback

**Files:**
- Modify: `src/app/mod.rs:97`
- Modify: `src/init.rs:135`
- Possibly others

**Context:** `dirs::home_dir().expect("...")` panics on Windows service accounts or minimal containers.

- [ ] **Step 1: Return Result instead of panicking**

In `src/app/mod.rs`:
```rust
pub fn roost_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("ROOST_DIR") {
        return Ok(PathBuf::from(dir));
    }
    dirs::home_dir()
        .map(|h| h.join(".roost"))
        .ok_or_else(|| eyre::eyre!("Could not determine home directory. Please set ROOST_DIR."))
}
```

- [ ] **Step 2: Update all callers to handle the Result**

This may cascade to many files. Use `?` or `.wrap_err("...")`.

- [ ] **Step 3: Run `cargo test`**

- [ ] **Step 4: Commit**

```bash
git add src/app/mod.rs src/init.rs [other callers]
git commit -m "fix: replace home_dir().expect() with graceful error

Previously a missing home directory caused an immediate panic.
Now it returns a descriptive error suggesting the ROOST_DIR
environment variable as a fallback."
```

---

### Task 23: Restrict Dangerous Git Functions

**Files:**
- Modify: `src/git/mod.rs`

**Context:** `git::undo` and `git::rollback` use `git reset --hard`. They are `pub` but `safe_rollback` should be used exclusively.

- [ ] **Step 1: Change visibility to `pub(crate)`**

```rust
pub(crate) fn undo(...) -> Result<bool> { ... }
pub(crate) fn rollback(...) -> Result<bool> { ... }
```

- [ ] **Step 2: Update any external callers**

Check if anything outside `src/git/` calls them. If the CLI currently calls `rollback`, redirect to `safe_rollback`.

- [ ] **Step 3: Run `cargo test`**

- [ ] **Step 4: Commit**

```bash
git add src/git/mod.rs
git commit -m "refactor(git): restrict undo/rollback to pub(crate)

The old undo() and rollback() used 'git reset --hard' which is
destructive. safe_rollback() is now the preferred public API.
Restricting the hard-reset variants to pub(crate) prevents
accidental misuse."
```

---

### Task 24: Add CHANGELOG.md

**Files:**
- Create: `CHANGELOG.md`

- [ ] **Step 1: Write initial CHANGELOG**

```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-06-05

### Added
- Terminal-based TUI for daily dotfile management
- Miller column file browser with responsive 3/2/1 column layouts
- Fuzzy search across apps and files
- Profile management (switch, create, delete)
- Git sync with rebase-based conflict detection
- Safe rollback preserving new apps
- Cross-profile symlink support with cycle detection
- Shell completion generation
- File preview with binary detection
- Config version field for future migrations

### Fixed
- TUI RemoveApp now actually removes apps
- Profile switch updates symlinks on disk
- Cross-profile source marker (←) renders correctly
- Search popup shows match count
- Help text key bindings corrected
- Atomic config writes are concurrency-safe
- Path traversal validation on ingest
- App name sanitization
- Profile deletion requires confirmation
- Panic hook and Ctrl-C handler leaks fixed
- File preview capped at 50KB
- Unicode display width respected
- Platform-aware EDITOR/PAGER defaults

### Security
- Validate ingest paths are within home directory
- Sanitize app names to prevent path injection
- Restrict destructive git reset --hard variants
```

- [ ] **Step 2: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs: add CHANGELOG.md starting at v0.2.0"
```

---

## Phase 5: Backend Polish

### Task 25: Fix Git Error Parsing Fragility

**Files:**
- Modify: `src/git/mod.rs:99`, `362`, `383`, `622`

**Context:** Substring matching on git error messages (`"nothing to commit"`) breaks under non-English locales.

- [ ] **Step 1: Replace string matching with exit code checks where possible**

For example, `git status --porcelain` + check if empty is more reliable than parsing `nothing to commit`.

For `is_dirty`:
```rust
pub fn is_dirty(roost_dir: &Path) -> Result<bool> {
    let output = run_git(roost_dir, &["status", "--porcelain"])?;
    Ok(!output.trim().is_empty())
}
```

For `commit` (checking if nothing to commit):
```rust
let output = run_git(roost_dir, &["diff", "--cached", "--quiet"]);
match output {
    Ok(_) => Ok(false), // no changes staged
    Err(e) => {
        // diff exits 1 when there are differences, which is OK
        // but we need to distinguish exit code 1 from real errors
        // run_git currently returns Err on non-zero exit...
        // This may require modifying run_git to return (exit_code, stdout, stderr)
    }
}
```

This may require a broader refactor of `run_git` to return exit codes. If that's too large, add a TODO and handle the most critical case (`is_dirty`).

- [ ] **Step 2: Run `cargo test`**

- [ ] **Step 3: Commit**

```bash
git add src/git/mod.rs
git commit -m "fix(git): replace fragile string error parsing with exit codes

Git error messages like 'nothing to commit' were matched by
substring, which breaks under non-English locales. Now uses
--porcelain and exit codes where possible."
```

---

### Task 26: Fix Sync Ignoring ensure_links Errors

**Files:**
- Modify: `src/tui/main_view/mod.rs:211-271`
- Possibly `src/main.rs` sync handler too

**Context:** After a successful sync, `linker::ensure_links` errors are silently swallowed.

- [ ] **Step 1: Check `ensure_links` result and surface errors**

In `process_action` for `Action::Sync`:
```rust
if let Err(e) = linker::ensure_links(...) {
    state.status_message = Some(format!("Sync succeeded but link verification failed: {}", e));
} else {
    state.status_message = Some("Sync complete".to_string());
}
```

- [ ] **Step 2: Run `cargo test`**

- [ ] **Step 3: Commit**

```bash
git add src/tui/main_view/mod.rs
git commit -m "fix(tui): surface ensure_links errors after sync

If link creation failed after a successful sync, the error was
silently ignored and the user saw 'Sync complete'. Now errors
are shown in the status message."
```

---

### Task 27: Fix Corrupted Config Recovery

**Files:**
- Modify: `src/app/mod.rs:114-127` (`load_shared`, `load_local`)

**Context:** TOML parse failures bubble up as generic errors with no suggestion to restore from backups or git.

- [ ] **Step 1: Wrap parse errors with recovery suggestions**

```rust
pub fn load_shared(roost_dir: &Path) -> Result<SharedAppConfig> {
    let path = roost_dir.join("roost.toml");
    let contents = std::fs::read_to_string(&path)
        .wrap_err_with(|| format!("Could not read {}", path.display()))?;
    let config: SharedAppConfig = toml::from_str(&contents)
        .wrap_err_with(|| {
            format!(
                "Failed to parse {}. The file may be corrupted. \
                 Check .backups/ or restore from git history.",
                path.display()
            )
        })?;
    Ok(config)
}
```

Do the same for `load_local`.

- [ ] **Step 2: Run `cargo test`**

- [ ] **Step 3: Commit**

```bash
git add src/app/mod.rs
git commit -m "fix: suggest recovery steps on corrupted config parse

Previously a malformed roost.toml produced a generic toml parse
error with no guidance. Now the error message points users to
.backups/ and git history for recovery."
```

---

## Phase 6: Testing

### Task 28: Add Integration Tests for Init Wizard

**Files:**
- Create: `tests/init.rs`

**Context:** The init wizard and onboarding TUI have zero integration test coverage.

- [ ] **Step 1: Create basic init integration tests**

Since the init wizard is interactive (dialoguer prompts), full TUI testing is difficult. Start with:

1. `init_creates_roost_dir` — `roost init` in a fresh temp dir creates `.roost/`, `roost.toml`, `local.toml`, and initializes git.
2. `init_reconstructs_local_when_shared_exists` — When `roost.toml` exists but `local.toml` is missing, `roost init` reconstructs local.toml.
3. `init_fails_when_already_initialized` — Running init twice without force should warn or handle gracefully.

Use `assert_cmd` subprocess pattern like other integration tests. For the interactive parts, use `std::process::Command` with `stdin` piped and pre-filled answers, or test the non-interactive reconstruction path.

- [ ] **Step 2: Run `cargo test tests/init.rs`**

- [ ] **Step 3: Commit**

```bash
git add tests/init.rs
git commit -m "test: add integration tests for roost init

Covers: fresh init creates directory structure, reconstruction
of local.toml when shared exists, and idempotency behavior."
```

---

### Task 29: Add TUI Unit Tests for Event Dispatching

**Files:**
- Create: `src/tui/main_view/tests.rs`
- Modify: `src/tui/main_view/mod.rs` (add `#[cfg(test)] mod tests;`)

**Context:** Zero unit tests for TUI state transitions, event dispatch, or dialog handlers.

- [ ] **Step 1: Create test module**

Test simple state mutations:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_switch_profile_updates_active() {
        // Construct a minimal MainViewState
        // Dispatch Action::SwitchProfile("work")
        // Assert local.active_profile == "work"
    }

    #[test]
    fn test_remove_app_clears_pending() {
        // Add a fake app
        // Dispatch Action::RemoveApp
        // Assert app is gone from shared.apps
    }

    #[test]
    fn test_search_filters_apps() {
        // Populate state with apps
        // Set search query
        // Assert filtered list matches
    }
}
```

This requires `MainViewState` to be constructible in tests. You may need to add a `#[cfg(test)]` constructor helper.

- [ ] **Step 2: Run `cargo test src/tui/main_view/tests.rs`**

- [ ] **Step 3: Commit**

```bash
git add src/tui/main_view/tests.rs src/tui/main_view/mod.rs
git commit -m "test: add unit tests for TUI main view state transitions

Covers profile switching, app removal, and search filtering to
catch regressions in core TUI logic."
```

---

## Phase 7: Final Verification

### Task 30: Full Test Suite & Release Check

- [ ] **Step 1: Run full test suite**

```bash
cargo test
```

Expected: All ~181+ tests pass, no new warnings.

- [ ] **Step 2: Build release binary**

```bash
cargo build --release
```

Expected: Clean build, zero warnings.

- [ ] **Step 3: Smoke test CLI**

```bash
ROOST_DIR=/tmp/roost-release-test ./target/release/roost init --help
ROOST_DIR=/tmp/roost-release-test ./target/release/roost init
ROOST_DIR=/tmp/roost-release-test ./target/release/roost add ~/.bashrc
ROOST_DIR=/tmp/roost-release-test ./target/release/roost list
ROOST_DIR=/tmp/roost-release-test ./target/release/roost save
ROOST_DIR=/tmp/roost-release-test ./target/release/roost status
```

- [ ] **Step 4: Tag release**

```bash
git tag -a v0.2.0 -m "Release v0.2.0"
```

- [ ] **Step 5: Commit any final fixes**

---

## Spec Coverage Checklist

After all tasks are complete, verify the following audit findings are addressed:

- [x] TUI `RemoveApp` is a no-op stub
- [x] Profile switch does not update symlinks
- [x] `←` source marker never rendered
- [x] Search popup height hides match count
- [x] Help text says `s` = sync (actually save)
- [x] Unchecked hash slicing `&hash[..7]` panics
- [x] Status message cleared on every keypress
- [x] `save_shared`/`save_local` errors silently ignored
- [x] Atomic write uses fixed `.tmp` filename
- [x] No path traversal validation
- [x] App name sanitization
- [x] Profile deletion has no confirmation
- [x] Panic hook never restored
- [x] Ctrl-C handler registered every run
- [x] `terminal.size()` failure blocks Ctrl-C
- [x] File preview reads unbounded sizes
- [x] Unicode display width ignored
- [x] Status bar overflows narrow terminals
- [x] No README.md
- [x] No LICENSE file
- [x] No Cargo metadata
- [x] No config version field
- [x] `EDITOR`/`PAGER` not platform-aware
- [x] `home_dir().expect()` panics
- [x] Dangerous `git::undo`/`rollback` still `pub`
- [x] Git error parsing by string matching
- [x] Sync ignores `ensure_links` errors
- [x] Corrupted config recovery missing
- [x] No `tests/init.rs`
- [x] No TUI unit tests

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-06-05-roost-release-prep.md`.**

**Two execution options:**

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.

**Which approach would you prefer?**
