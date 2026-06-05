# Fix: Primary Config Set to Directory Instead of File

## Problem

When pressing `p` (Set Primary) on a file inside a directory app, the primary config is stored as the **internal roost path** (e.g., `~/.roost/default/nvim/init.lua`). This causes two issues:

1. The `o` key handler resolves the path incorrectly via `link_paths.get(&app).cloned()`, which returns the **app directory** (`~/.config/nvim`) instead of the **file** (`~/.config/nvim/init.lua`)
2. The primary config should always be stored as the **original path** (the symlink target), not the internal roost path

Additionally, the `o` key handler's `link_paths` fallback is incorrect for file-level primary configs.

## Solution

### 1. Fix `SetPrimary` action in `process_action`

In `src/tui/main_view/mod.rs`, convert the internal roost path to the original path before storing as `primary_config`:

```rust
Action::SetPrimary { app, path } => {
    if let Some(app_entry) = state.shared.apps.get_mut(&app) {
        let resolved = if let Some(original_base) = state.local.link_paths.get(&app) {
            let app_dir = crate::app::profile_dir(&state.roost_dir, &state.local.active_profile)
                .join(&app);
            if path.starts_with(&app_dir) {
                if let Ok(rel) = path.strip_prefix(&app_dir) {
                    original_base.join(rel)
                } else {
                    path
                }
            } else {
                path
            }
        } else {
            path
        };
        app_entry.primary_config = Some(resolved);
        // ... save
    }
}
```

### 2. Fix `o` key handler in `event.rs`

Remove the `link_paths` fallback that incorrectly replaces the file path with the directory:

```rust
KeyCode::Char('o') if state.focus == Focus::AppsPanel => {
    if let Some(app) = state.selected_app().cloned() {
        if let Some(path) = state
            .shared
            .apps
            .get(&app)
            .and_then(|a| a.primary_config.clone())
        {
            return vec![Action::OpenEditor(path)];
        }
    }
    vec![Action::SetStatus("No primary config for this app".to_string())]
}
```

### 3. Fix `guess_primary_configs` for the Add App flow

When apps are added via `a` in the main TUI, `guess_primary_configs` is never called. Add it after the `SuspendForAddApp` action processes new apps:

```rust
if !added.is_empty() {
    // ... existing save code ...
    
    // Guess primary configs for newly added apps
    let _ = guess_primary_configs(&roost_dir, &profile_name, &mut state.shared, &state.local);
}
```

Extract `guess_primary_configs` from `init.rs` to a shared module (e.g., `app::guess_primary_configs`) so it can be used by both `init.rs` and `main_view/mod.rs`.

## Verification

- `cargo test` passes
- `roost init` with a directory app containing one file correctly sets `primary_config` to the file
- Main TUI `a` to add a directory app with one file correctly sets `primary_config` to the file
- `p` in Files panel on a file inside a directory app stores the correct original path
- `o` key opens the correct file (not the directory)

## Files
- `src/tui/main_view/mod.rs` — fix `SetPrimary` path resolution
- `src/tui/main_view/event.rs` — fix `o` key handler
- `src/init.rs` — extract `guess_primary_configs` to shared module
- `src/app.rs` or `src/app/mod.rs` — add shared `guess_primary_configs` function
