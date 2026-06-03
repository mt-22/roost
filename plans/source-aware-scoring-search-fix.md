# Fix: Source-Aware Confidence Scoring + Search Engine Extraction

## Problem Statement

Two issues to fix:

1. **Confidence scores ignore source directory.** Items in `~/Library/Application Support` get the same score as items in `~/.config`. The user wants only known apps auto-selected from non-primary dirs, and the source directory should affect confidence.

2. **Fuzzy search overlay (`/`) not responding.** The search overlay code exists but pressing `/` doesn't activate it. Additionally, the search logic needs to be extracted into a reusable module for the main TUI.

---

## 1. Source-Aware Confidence Scoring

### Current Behavior
- `score_item()` only considers name and item type
- Known apps = 150 everywhere
- Dirs with config children = 100 everywhere
- The `scan_sources()` function filters at >= 100

### Desired Behavior
- Source directory should modify the final confidence
- `~/.config`: normal scoring (known apps 150, config dirs 100)
- `$HOME`: normal scoring for known dotfiles (200) and known apps (150)
- `~/Library/Application Support`: known apps stay 150, everything else penalized by 50
- `~/.local/bin`, `~/.ssh`: known apps stay 150, everything else penalized by 25

### Implementation

Add a `source_modifier` function in `scanner/mod.rs`:

```rust
fn source_modifier(path: &Path) -> i32 {
    let path_str = path.to_string_lossy();
    if path_str.contains("Library/Application Support") {
        -50
    } else if path_str.ends_with(".local/bin") || path_str.ends_with(".ssh") {
        -25
    } else {
        0
    }
}
```

In `scan_sources()`, apply the modifier to each item's confidence:
```rust
let modifier = source_modifier(&dir);
for mut item in scan_directory(&dir, ignored) {
    item.confidence = (item.confidence as i32 + modifier).max(0) as u32;
    // ... dedupe and filter
}
```

### Result
- Known apps in any dir: 150 (auto-selected, shown)
- Config dirs in `~/.config`: 100 (shown, not auto-selected)
- Config dirs in `~/Library/Application Support`: 50 (hidden by >= 100 filter)
- This means ONLY known apps appear from `~/Library/Application Support`

---

## 2. Extract Search Engine to Reusable Module

### New File: `src/tui/search/mod.rs`

Create a self-contained fuzzy search module:

```rust
pub struct SearchEngine {
    query: String,
    filtered_indices: Vec<usize>,
    cursor: usize,
}

impl SearchEngine {
    pub fn new() -> Self { ... }
    pub fn query(&self) -> &str { ... }
    pub fn is_active(&self) -> bool { ... }
    pub fn push_char(&mut self, c: char) { ... }
    pub fn backspace(&mut self) { ... }
    pub fn clear(&mut self) { ... }
    pub fn move_up(&mut self) { ... }
    pub fn move_down(&mut self, max: usize) { ... }
    pub fn filtered_indices(&self) -> &[usize] { ... }
    pub fn cursor(&self) -> usize { ... }
    pub fn filter<T: AsRef<str>>(&mut self, items: &[T]) { ... }
}
```

### Update `init_tui.rs`
- Replace inline search state with `SearchEngine`
- Import from `crate::tui::search::SearchEngine`

---

## 3. Fix Search Overlay Activation

### Possible Causes
1. crossterm not detecting `/` on macOS in some terminals
2. Event polling timeout too short
3. Search overlay renders but is visually subtle

### Fixes
1. **Add alternative search key:** Also bind `s` to activate search
2. **Increase event poll timeout:** Change from 100ms to 250ms to reduce CPU usage and ensure events are captured
3. **Make overlay more visible:**
   - Increase height to 5 rows (show query + match count + hint)
   - Add bright yellow border
   - Show "N matches" indicator
   - Add hint text: "Esc to close, Enter to confirm"

### Search Overlay Layout
```
┌─ Search ───────────────────────────┐
│ query_here                         │
│                                    │
│ 12 matches    Esc=close            │
└────────────────────────────────────┘
```

---

## 4. File Changes

| File | Change |
|------|--------|
| `src/scanner/mod.rs` | Add `source_modifier()`, update `scan_sources()` to apply modifiers |
| `src/scanner/tests.rs` | Add tests for source-aware scoring |
| `src/tui/search/mod.rs` | **New file** — extracted search engine |
| `src/lib.rs` | Add `pub mod tui;` |
| `src/tui/mod.rs` | **New file** — re-export search module |
| `src/init_tui.rs` | Use `SearchEngine`, fix search activation, add `s` key, improve overlay |

---

## 5. Testing

### Unit Tests
- `scanner::tests::source_modifier_application_support`: verify -50 penalty
- `scanner::tests::source_modifier_local_bin`: verify -25 penalty
- `scanner::tests::source_modifier_config`: verify no penalty
- `scanner::tests::scan_sources_filters_app_support_noise`: verify only known apps appear from App Support
- `tui::search::tests::filter_finds_matches`: basic filtering
- `tui::search::tests::filter_empty_query_returns_all`: empty query = all items
- `tui::search::tests::cursor_bounds`: cursor stays within filtered results

### Manual Verification
- Run `roost init` in a test directory
- Verify `~/.config` items appear normally
- Verify `~/Library/Application Support` only shows known apps
- Verify `/` and `s` both activate search
- Verify search filters results in real-time

---

## 6. Implementation Order

1. Create `src/tui/search/mod.rs` with `SearchEngine`
2. Update `src/lib.rs` and create `src/tui/mod.rs`
3. Update `src/scanner/mod.rs` with source modifiers
4. Update `src/init_tui.rs` to use `SearchEngine` and fix activation
5. Write tests
6. Run full test suite
