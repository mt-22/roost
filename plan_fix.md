# Fix: Ctrl-C Handler Conflict in Add App Flow

## Problem

When pressing `a` in the main TUI to add apps, `app_selector::run_selection_tui` is called from within `suspend_and_run`. This fails because:

1. The main TUI already registered a `ctrlc::set_handler` at startup
2. `app_selector.rs` tries to register its own `ctrlc::set_handler` inside `run_selection_tui`
3. The `ctrlc` crate returns an error: `Ctrl-C signal handler already registered`
4. Additionally, `app_selector.rs` has its own private `SHOULD_EXIT` static, which is never triggered by the main TUI's handler

## Solution

Make `run_selection_tui` accept an external `&AtomicBool` for exit signaling, so the main TUI can share its own `SHOULD_EXIT` flag with the app selector.

### Changes

1. **`src/app_selector.rs`**
   - Change `run_selection_tui` signature:
     ```rust
     pub fn run_selection_tui(
         scan_items: Vec<DiscoveredItem>,
         root_path: &Path,
         should_exit: &AtomicBool,
     ) -> Result<TuiResult>
     ```
   - Remove the local `static SHOULD_EXIT: AtomicBool` from `app_selector.rs`
   - Remove the `ctrlc::set_handler` call entirely from `app_selector.rs` (callers are responsible for setting it up)
   - In `run_app`, check `should_exit.load(Ordering::Relaxed)` instead of the local static
   - Remove the `ctrlc` dependency from `app_selector.rs` imports

2. **`src/init.rs`**
   - Create a local `AtomicBool` (e.g., `let should_exit = AtomicBool::new(false)`)
   - Register `ctrlc::set_handler` before calling `run_selection_tui`, storing the handler
   - Pass `&should_exit` to `app_selector::run_selection_tui`
   - The handler just sets the local `AtomicBool` to true

3. **`src/tui/main_view/mod.rs`**
   - Pass `&SHOULD_EXIT` to `app_selector::run_selection_tui` inside the `suspend_and_run` closure
   - No need to register a new handler (the main TUI's handler already updates `SHOULD_EXIT`)

4. **`src/tui/main_view/mod.rs`** (if not already done)
   - Ensure `SHOULD_EXIT` is accessible in the `suspend_and_run` closure (it is `static` at module scope, so it is)

## Verification

- `cargo test` passes
- `roost init` still works (registers its own handler + passes flag)
- Main TUI `a` key works without the Ctrl-C error
- Pressing Ctrl-C in the app selector (when launched from main TUI) exits cleanly because the main TUI's handler already sets `SHOULD_EXIT`

## Risks

- The `panic::set_hook` in `app_selector.rs` may still conflict with the main TUI's hook. However, since the main TUI is suspended (alternate screen left, raw mode disabled), the app selector takes over the terminal and its panic hook is appropriate. The main TUI's hook will be restored when `suspend_and_run` returns.

## Out of Scope

- No changes to `app_selector.rs` UI logic, search, or miller behavior
- No changes to the main TUI event loop or rendering
