# Plan: Reuse Selection TUI for Add App Flow

## Problem
The current `a` (Add App) flow in the main TUI uses a custom popup dialog (`src/tui/main_view/dialogs/add_app.rs`) that only supports single-file selection. The user wants to use the **full adoption TUI** from `src/init_tui.rs` (scan results + browse tabs, multi-select, search, confirm) instead. Since this TUI is now used for both onboarding (`roost init`) and adding apps from the main TUI, it needs to be renamed and generalized.

## Goal
1. Rename `src/init_tui.rs` to a generic name reflecting its dual use (onboarding + add-app)
2. When the user presses `a` in the main TUI, **suspend the main TUI** and launch the full selection TUI
3. The user can select multiple apps (from scan results or browse), then confirm
4. After confirm, the main TUI resumes and ingests all selected apps
5. The UI between the two TUIs should be balanced: same status bar style, `?` help overlay

## Rename
- `src/init_tui.rs` → `src/app_selector.rs` (generic selection TUI for picking apps)
- Update `src/lib.rs`: add `pub mod app_selector;` (or replace `pub mod init_tui;`)
- Update `src/init.rs`: `use crate::app_selector;` instead of `init_tui`
- Update `src/tui/main_view/mod.rs`: `use crate::app_selector;` for the Add App flow
- Update `AGENTS.md`: replace `init_tui.rs` references with `app_selector.rs`

## Changes

### 1. `src/app_selector.rs` (formerly `init_tui.rs`) — UI Balancing
- **Reorder status bar tooltips** to match main TUI priority: `? help` first, then `j/k nav`, `Tab focus`, `/ search`, `Space select`, `Enter confirm`, `Esc cancel`
- **Add `?` help overlay** — a searchable keybind reference (same style as `src/tui/main_view/dialogs/help.rs`)
- **Add `help_visible` flag** to `App` struct
- **Add `KeyCode::Char('?')` handler** that toggles the help overlay
- **Render help overlay** in `run_app()` draw loop (centered, bordered, scrollable, searchable)
- Keep `TuiResult` and `run_selection_tui` public API unchanged

### 2. `src/tui/main_view/event.rs` — Replace Add App Dialog
- Remove the `handle_add_app` function entirely
- In the base `handle_key` match for `KeyCode::Char('a')`, instead of opening `add_app_dialog`, return `Action::SuspendForAddApp`
- Remove the old `AddApp` single-app action and `AddAppFocus` type

### 3. `src/tui/main_view/mod.rs` — Handle New Action
- Add `Action::SuspendForAddApp` variant
- In the `process_action` match, handle `SuspendForAddApp`:
  - Call `crate::tui::suspend::suspend_and_run` closure
  - Inside the closure:
    - Scan for apps using `scanner::default_scan_sources` and `scanner::scan_sources`
    - Call `app_selector::run_selection_tui(scan_items, home_dir)`
    - Match on `TuiResult::Selected(items)` → for each item, call `linker::ingest` and add to shared config
    - Match on `TuiResult::Aborted` → do nothing
  - After the closure returns, set `pending_auto_commit = true` if any apps were ingested
  - Return `Ok(Continue)` to resume the main TUI

### 4. `src/tui/main_view/dialogs/add_app.rs` — Remove
- The entire file can be removed since the main TUI no longer uses a custom dialog

### 5. `src/tui/main_view/dialogs/mod.rs` — Remove Exports
- Remove `pub use add_app::*;`
- Remove `AddAppFocus`, `AddAppState` from the public API

### 6. `src/tui/main_view/state.rs` — Remove AddApp Dialog State
- Remove `add_app_dialog: Option<add_app::AddAppState>` from `MainViewState`
- Remove the `if add_app_dialog.is_some()` early-return in `focused_app()` and `focused_app_name()`

### 7. `src/tui/main_view/ui.rs` — Remove AddApp Dialog Rendering
- Remove the `if let Some(ref add_app) = state.add_app_dialog` block in `render_dialogs`
- Remove `render_add_app_dialog` function entirely
- Remove the `add_app` import from `dialogs`

### 8. `src/init.rs` — Update Import
- Change `use crate::init_tui;` to `use crate::app_selector;`
- Update all `init_tui::` calls to `app_selector::`

### 9. `src/lib.rs` — Update Module Declaration
- Replace `pub mod init_tui;` with `pub mod app_selector;`

### 10. `AGENTS.md` — Update Documentation
- Replace `init_tui.rs` references with `app_selector.rs` in the module map and current state assessment

## Key Design Decision
Instead of embedding the selection TUI as a dialog, we **suspend the main TUI** and run it standalone. This is the cleanest approach because:
- `app_selector` sets up its own terminal (alternate screen, raw mode)
- `suspend_and_run` already handles leaving/restoring the alternate screen
- No need to duplicate the 819-line logic inside the main TUI
- The user gets the exact same UI they see during `roost init`

## Risks
- `app_selector` uses `ctrlc::set_handler` which replaces the global handler. The main TUI's handler will be restored after `suspend_and_run` returns, but we should verify the `ctrlc` crate behavior (it replaces the old handler, so the main TUI handler will need to be re-registered when returning)
- `app_selector` uses `SHOULD_EXIT` static atomic. We should check if the main TUI also uses one and ensure they don't conflict.
- Panic hook: `app_selector` sets a panic hook. We should save/restore the main TUI's panic hook.
- The rename touches 4 source files and 1 doc file. Ensure all imports are updated.

## Verification
- After changes, `cargo test` should pass with **all existing tests unchanged**
- The main TUI should still compile and run
- `roost init` should still work (uses `app_selector::run_selection_tui`)
- The old `AddApp` dialog tests should be removed or updated

## Out of Scope
- The `app_selector.rs` scan results logic doesn't need to change (it's already tested and working)
- The `ConfirmDialog` in `app_selector.rs` doesn't need to change
- The `miller` widget doesn't need to change
