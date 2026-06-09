# Changelog

## [0.2.4] — 2026-06-09

### Added

- **`roost remove --all`** — Removes all apps from the current profile and restores original config files

### Testing

- Integration tests for `remove --all`

## [0.2.3] — 2026-06-07

### Fixed

- **Codebase audit fixes** — General cleanup and correctness improvements

## [0.2.2] — 2026-06-07

### Changed

- **Unified mutation layer** — CLI and TUI now both route domain operations through `src/ops.rs` for a single source of truth

### Fixed

- **`remove_app` profile cleanup** — Removes the profile-level roost directory after unlinking an app
- **`switch_profile` backups** — Properly handles existing conflicts and directories during profile switching
- **`delete_profile` disk cleanup** — Removes the profile directory from disk when deleting a profile
- **`safe_rollback` hard failures** — Fails hard on critical repair errors instead of silently continuing
- **`remove_app` scope** — Only cleans the current profile, not all profiles

## [0.2.1] — 2026-06-07

### Added

- **Import-from and copy-to for profiles** — Cross-profile app operations to import or copy apps between profiles

### Changed

- **Hardened safety documentation** — Updated README to stop over-promising and clarify actual capabilities
- **Cross-platform support status** — Clarified OS compatibility claims in documentation

## [0.2.0] — 2026-06-06

### Added

- **Full TUI** — Complete daily-use terminal interface with app panel, Miller column file browser, fuzzy search, and keyboard-driven navigation
- **App selection TUI** — Interactive onboarding and add-app flows with multi-select, search, and directory browsing
- **Git sync** — `sync` command and TUI action with push, pull, and conflict detection
- **Safe rollback** — Selective git checkout preserves apps not present at the target commit; creates forward commit instead of destructive reset
- **Structural merge** — Field-level conflict resolution for `primary_config`, `ignore`, profile/app deletions, and ignored patterns
- **Git log viewer** — History browser within TUI with inline diff viewer and rollback trigger
- **Ignore system** — Global ignore patterns (scanner filtering + `.gitignore`) and per-app patterns (`.gitignore` scoped paths)
- **Responsive Miller columns** — Three modes: 3-column ≥100w, 2-column ≥55w, vertical stack <55w
- **File preview** — Inline content preview with binary detection in Miller columns
- **Primary config highlight** — `★` marker in Miller columns on primary config file; cursor auto-focuses on it
- **Fuzzy search engine** — Reusable `FuzzyEngine` with configurable scoring
- **Cross-profile symlink support** — Import/paste apps between profiles with `←` source markers
- **Cross-platform symlinks** — `#[cfg]` branches for Unix and Windows
- **Config validation** — Cycle detection, unknown app/profile checks on config load
- **Atomic config writes** — Concurrency-safe temp-file-then-rename for `roost.toml`, `local.toml`, and `.gitignore`
- **Path traversal validation** — `validate_path_in_home()` helper ensuring symlink targets stay within `~`
- **App name sanitization** — Replaces `/`, `\`, `\0` with `_` in app names derived from filenames
- **Suspend/resume infrastructure** — Generic helper to drop alternate screen for `$EDITOR`, `$PAGER`, and `git`
- **Exit banner** — Random talking rooster ASCII art banner on TUI quit
- **README** — Install guide, CLI reference, and TUI keybindings
- **Crates.io publishing** — Published as `roost-dot` on crates.io

### Changed

- **Profile switch now calls `linker::switch_profile()`** — symlinks update on disk, not just `local.toml`
- **`sync()` pushes after successful rebase** — `git push origin main` included in sync flow
- **`ensure_links()` called after sync** — both CLI and TUI re-establish symlinks post-sync
- **Confirm dialog renders on top** — fixes z-order bug where profile/ignore dialogs hid confirm overlays
- **Panic hook / Ctrl-C handler consolidated** — single `tui::init()` using `OnceLock` replaces 3 duplicate registrations
- **Quit keybinding** — Removed `Esc` as quit keybinding; use only `q`
- **Crate name** — Renamed crate to `roost-dot` on crates.io (binary name stays `roost`)

### Fixed

- **Fuzzy search in main TUI** — Query sync with `FuzzyEngine`, filter persistence, j/k routing through engine, filtered rendering
- **`Action::RemoveApp`** — Mirrors CLI `cmd_remove`: unlinks symlinks, removes from configs, saves atomically, auto-commits
- **Cross-profile source marker** — `←` indicator rendered for linked apps (was dead code)
- **Help text** — Corrected keybind documentation: `s`=Save, `S`=Sync, `r` only in git log, panel-specific keys
- **Hash slicing** — `&hash[..7]` replaced with safe `hash[..hash.len().min(7)]` in 3 locations
- **Status message persistence** — No longer cleared on every keypress; persists for overlay keys
- **Narrow terminal panic** — `saturating_sub` used for all width calculations; minimum 40x12 enforcement
- **Profile deletion confirm** — y/n confirmation before deleting profile
- **Ignore removal confirm** — y/n confirmation before removing ignore pattern
- **`j`/`k` in text input dialogs** — No longer consumed for navigation in ignore Add and profile Create modes
- **`rebase --continue` error propagation** — `git::sync()` surfaces failures instead of returning false `Clean`
- **Temp backup clobbering** — Backups go to `.backups/` inside roost directory
- **Git identity in tests** — All test helpers set `user.name`/`user.email`
- **Single-file app view in Miller columns** — Fixed display for apps with only one file
- **Status bar overflow** — Fixed overflow issues in status bar rendering
- **Git conflict UX** — Improved user experience during merge conflicts
- **Ctrl+C in raw mode** — Safe TUI exit when pressing Ctrl+C in raw terminal mode
- **Single-file app filter preservation** — Preserves filter when clearing app search in single-file apps
- **First push to new remote** — `sync` handles first push to a new remote correctly
- **Remote URL updates** — `roost remote` allows updating existing remote URL

### Testing

- **184 tests total** (112 unit + 71 integration + 1 doctest)
- Integration tests for: diff, ignore, restore, rollback, adopt, list, save, where, sync, profile, status, completions, doctor, git_commands

## [0.1.0] — 2026-02-01

- Initial release — basic CLI dotfile management with profiles and git sync
