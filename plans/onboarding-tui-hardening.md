# Onboarding TUI Hardening Plan

## Goal
Harden the `roost init` onboarding flow to match the intended UX: a clean scan-results list with fuzzy search, proper key bindings, signal-safe cleanup, and better app discovery.

---

## 1. Expand `known_apps.txt`

Add commonly-missing apps that currently score 50 ("other dir") but should score 150 (known app).

**Additions:**
- `fastfetch`
- `neofetch`
- `paru`, `yay`
- `qutebrowser`
- `brave`
- `obsidian`
- `discord`
- `spotify`
- `signal-desktop`
- `zoom`
- `obs-studio`
- `blender`
- `docker`
- `yt-dlp`
- `code` (VSCode)
- `cursor`
- `zed`
- `warp`
- `com.raycast.macos` (macOS Application Support dir name)

> If any of these are already present, skip duplicates.

---

## 2. Confidence Filtering

- **Threshold: 100.** Any scan result with confidence < 100 is discarded before reaching the TUI.
- This means only **known dotfiles (200)**, **known apps (150)**, and **dirs with config children (100)** appear.
- Config files (80) and other dirs (50) are hidden.

**Implementation:** Apply filtering in `scanner::scan_directory` or in `init.rs` before passing to the TUI.

---

## 3. Scan Source Directories

Maintain a list of known config source locations. Check existence before scanning. Merge and deduplicate results.

**Sources to check:**
| Path | Platforms | Notes |
|------|-----------|-------|
| `~/.config` | Linux, macOS | Primary XDG config dir |
| `~/Library/Application Support` | macOS | macOS app support |
| `~/.local/bin` | Linux | Some tools store configs here |
| `~/.ssh` | All | SSH keys + config |
| `$HOME` | All | Dotfiles |

**Behavior:**
- Iterate the list.
- Skip any path that does not exist.
- Run `scanner::scan_directory` on each.
- Deduplicate by absolute path.
- Sort by confidence descending.
- Filter below 100.
- Pass unified `Vec<DiscoveredItem>` to the TUI.

---

## 4. Pre-Select Existing Apps on Re-Init

When `roost init` is run on an already-initialized repo (e.g., after a git pull):
- Load existing `roost.toml` apps.
- Pre-select any scan result whose name matches an already-managed app.
- Do **not** use confidence-based pre-selection in this case.
- If it is a *fresh* init (no existing config), fall back to pre-selecting items with confidence >= 150.

---

## 5. Onboarding TUI Improvements (`init_tui.rs`)

### 5a. Fuzzy Search Overlay (`/`)
- Press `/` to open a search overlay.
- Overlay: Yellow border, width ~40, centered, with `Clear` widget behind.
- Typing filters the scan list in real time.
- `Esc` or `Enter` closes search and returns focus to the list.
- `j/k` navigate within filtered results.

### 5b. Key Binding Changes
| Key | Current | New |
|-----|---------|-----|
| `Enter` | Finalize | No-op (or keep as alternative) |
| `w` | — | Finalize and return selections |
| `Space` | Toggle select | Toggle select |
| `Esc` | Cancel immediately | Open "Discard selections?" confirm dialog |
| `q` | Cancel | Cancel (same as Esc) |
| `j/k` | Navigate | Navigate |
| `Tab` | Switch tab | Switch tab |
| `/` | — | Open fuzzy search |

### 5c. Discard Confirmation on `Esc`
- When `Esc` or `q` is pressed, show a centered confirm dialog:
  - "Discard selections and exit?"
  - `y` = return empty Vec (cancel)
  - `n` or `Esc` = close dialog, resume

### 5d. `<C-c>` Signal Handling
- Install a `ctrlc` handler (or use `tokio::signal` if we were async; since we're blocking, use `ctrlc` crate or crossterm signal detection).
- On `Ctrl+C`:
  1. Restore terminal (leave alternate screen, disable raw mode).
  2. **Do NOT write configs.**
  3. Exit process with non-zero status.
- **Scope:** Within the TUI only. Dialoguer prompts before the TUI are handled by standard process termination (already safe).

### 5e. Symbol Updates
- Replace `☐ / ☑` checkboxes with `●` bullet for selected items.
- Unselected items show no bullet (just space).
- Cursor indicator: `»` before the active line.
- Use `bg(DarkGray) + bold` for cursor highlight (per SPEC palette).

### 5f. Status Bar
- Replace gray hint bar with SPEC-style status bar.
- Yellow keys: `j/k nav  Tab focus  / search  ␣ select  w confirm  Esc cancel`

### 5g. Layout Adjustments
- Scan Results panel: ~70% width.
- Right panel: ~30% width showing selected items with `●` bullets.
- This mirrors the SPEC onboarding layout sketch.

---

## 6. Testing

### Unit Tests
- `scanner::tests`: verify confidence filtering (items below 100 excluded).
- `init_tui`: test fuzzy matching logic independently.

### Integration Tests
- `tests/init.rs` (new file):
  - `init_creates_config_files`
  - `init_skips_existing_local_toml`
  - `init_scans_and_selects_apps` (mock TUI selection)
  - `init_reinit_preselects_existing_apps`

> Note: Testing the full TUI interactively is hard in subprocess tests. We may test `init.rs` logic by extracting the scan+merge+filter into a pure function, then testing that.

---

## 7. Implementation Order

1. **known_apps.txt** — quick win, no risk.
2. **Scanner source list + filtering** — modify `init.rs` to scan multiple dirs, dedupe, filter < 100.
3. **Pre-select logic** — adjust `init_tui::App::new` to accept `existing_apps: HashSet<String>`.
4. **Signal handling + cleanup** — add `ctrlc` handler and terminal restore on panic/signal.
5. **Discard confirmation dialog** — simple overlay in `init_tui.rs`.
6. **Fuzzy search overlay** — add search state, filter logic, overlay rendering.
7. **Key binding updates + symbols + status bar** — polish.
8. **Right panel for selected items** — layout change.
9. **Tests** — write unit and integration tests for new logic.

---

## 8. Dependencies

- `ctrlc` (for `C-c` handling in the TUI). Lightweight, widely used.
- No other new dependencies expected.

---

## Open Questions

- Should `~/.local/share` or `~/Library/Preferences` also be in the source list? (Currently omitted per "not super exhaustive.")
- Should the search overlay also work in the Browse (Miller) tab? (Proposed: no, only in Scan Results.)
