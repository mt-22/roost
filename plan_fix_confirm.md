# Fix: Set Primary (and Remove App) Confirm Dialogs Close TUI Instead of Performing Action

## Bug Description

When pressing `p` in the Files panel to set a primary config, the TUI closes instead of saving the setting. The same bug affects `x` in the Apps panel (remove app). The primary config is also not saved.

## Root Cause

`handle_confirm` in `event.rs` has a catch-all at the end:
```rust
return match action {
    ConfirmAction::Confirm => vec![Action::Quit],
    ConfirmAction::Discard => vec![Action::Quit],
};
```

This means ANY confirm dialog with `ConfirmAction::Confirm` returns `Action::Quit`, including "Set Primary" and "Remove App".

## Fix

1. Add `pending_action: Option<Action>` to `MainViewState`
2. When `p` sets the confirm dialog, also set `pending_action = Some(Action::SetPrimary { app, path })`
3. When `x` sets the confirm dialog, also set `pending_action = Some(Action::RemoveApp(app))`
4. In `handle_confirm`, when confirmed:
   - `if let Some(action) = state.pending_action.take() { return vec![action]; }`
   - Then check `rollback_pending` marker
   - Then fall back to `Action::Quit`
5. On cancel, also `state.pending_action = None` to clear it

## Verification

- `cargo test` passes
- `p` key sets primary config without closing TUI
- `x` key (when fully implemented) will remove app without closing TUI
- `q` quit and `Esc` quit still work correctly
- Git log `r` rollback still works via `rollback_pending` marker

## Files
- `src/tui/main_view/state.rs` — add `pending_action` field
- `src/tui/main_view/event.rs` — set `pending_action` for `p`/`x`, consume it in `handle_confirm`
