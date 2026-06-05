# Plan: Make `o` Key Global + Highlight Primary Config in Files Panel

## Goals
1. Pressing `o` anywhere in the TUI (Apps panel or Files panel) should open the primary config for the selected app
2. The primary config file should be visually highlighted in the Miller columns (Files panel), not just the app name in the Apps panel

## Analysis

### Current State
- `o` is gated to `Focus::AppsPanel` only in `event.rs`
- `★` marker in apps panel shows which app has a primary config, but the actual file isn't highlighted in the miller columns
- `sync_miller_to_selected_app` sets the miller root to the app directory, but doesn't position the cursor on the primary config file

## Changes

### 1. `src/tui/main_view/event.rs` — Make `o` global
Remove the `if state.focus == Focus::AppsPanel` gate from the `o` key handler so it works from any panel:
```rust
KeyCode::Char('o') => {
    if let Some(app) = state.selected_app().cloned() {
        if let Some(path) = state
            .shared
            .apps
            .get(&app)
            .and_then(|a| a.primary_config.as_ref())
        {
            return vec![Action::OpenEditor(path.clone())];
        }
    }
    vec![Action::SetStatus("No primary config for this app".to_string())]
}
```

### 2. `src/tui/main_view/state.rs` — Sync miller cursor to primary config
When `sync_miller_to_selected_app` is called, also check if the app has a `primary_config` and set the miller cursor to that file. The miller needs to navigate into subdirectories if needed.

### 3. `src/miller.rs` — Add primary config highlighting
- Add `primary_config: Option<PathBuf>` to `MillerColumns`
- When rendering, check if an entry's path matches `primary_config` and apply a `★` prefix + highlight style
- Update `MillerColumns::set_root` to accept the primary config path
- Add a helper `set_primary_config` to update it when the app changes

### 4. `src/tui/main_view/ui.rs` — Pass primary config to miller rendering
In `render_files_panel`, get the selected app's primary config and set it on the miller before rendering.

### 5. `src/tui/main_view/mod.rs` — Sync cursor when app changes
When `sync_miller_to_selected_app` is called, also set the primary config on the miller. If the primary config is in a subdirectory, navigate down to it.

## Implementation Order
1. Make `o` global in `event.rs`
2. Add `primary_config` field to `MillerColumns`
3. Update `render_entries` to highlight primary config
4. Update `sync_miller_to_selected_app` to set cursor position
5. Update `ui.rs` to pass primary config

## Verification
- `cargo test` passes
- `o` opens primary config from both panels
- Primary config file is highlighted with `★` in miller columns
- Cursor focuses on primary config when switching apps

## Risks
- `sync_miller_to_selected_app` navigating into subdirectories might break if the path doesn't exist
- The `★` prefix uses 2 chars, so we need to adjust truncation math

## Out of Scope
- No changes to the `p` key behavior
- No changes to `guess_primary_configs`
