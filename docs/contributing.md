# Contributing to Roost

## Development Setup

See [building.md](building.md) for prerequisites and build instructions.

## Code Style

- Run `cargo fmt` before committing. The project has no custom rustfmt config — follow the default Rust formatting conventions.
- Run `cargo clippy` and address warnings. The project has no custom clippy config — follow the default lint rules.
- Prefer `color_eyre::Result<T>` as the return type for fallible functions.
- Use `eyre!()` for error messages with context.

## Testing

See [testing.md](testing.md) for the full testing guide.

- All changes should include tests. Run `cargo test` before submitting.
- Integration tests go in `tests/` (one file per command or feature area).
- Unit tests go in `#[cfg(test)]` modules within the source file.
- Use the `TestRoost` fixture for integration test setup.

## Commit Messages

Use [conventional commits](https://www.conventionalcommits.org/):

```
feat: add roost rollback command
fix: handle missing profile directory in switch
test: add edge cases for cross-profile removal
docs: update README with multi-device workflow
refactor: extract dialog rendering into helper
```

Prefixes:
- `feat:` — New feature
- `fix:` — Bug fix
- `test:` — Adding or updating tests
- `docs:` — Documentation changes
- `refactor:` — Code restructuring without behavior change
- `chore:` — Maintenance tasks (dependencies, tooling)

## Project Structure

```
src/
├── main.rs           # CLI entry point and command routing
├── app.rs            # Data models, config loading/saving, profile CRUD
├── init.rs           # roost init wizard
├── git.rs            # Git operations (shell out to git CLI)
├── linker.rs         # Symlink operations (ingest, restore, unlink, ensure_links)
├── scanner.rs        # App discovery, confidence scoring, file collection
├── os_detect.rs      # Runtime OS detection
├── pager.rs          # Pager integration
├── lib.rs            # Library root (re-exports)
├── data/             # Static data files (known_apps.txt, known_dotfiles.txt)
└── tui/
    ├── mod.rs        # Onboarding TUI entry point
    ├── event.rs      # Onboarding event handling
    ├── state.rs      # Onboarding state
    ├── ui.rs         # Onboarding rendering
    ├── search.rs     # Fuzzy search widget
    └── main_view/    # Main TUI (day-to-day management)
        ├── mod.rs    # Main view entry point
        ├── event.rs  # Main view event handling
        ├── state.rs  # Main view state + dialogs
        └── ui.rs     # Main view rendering

tests/                # Integration tests (one file per command)
```
