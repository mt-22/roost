# Building Roost

This is a Rust project (edition 2024) that builds with standard Cargo.

## Prerequisites

- **Rust 1.85 or later** — required for Rust 2024 edition
- **Git** — required for roost's sync functionality (but not for building itself)
- **`less`** (optional) — used as the default pager for `roost diff`

Install Rust via [rustup](https://rustup.rs/) if needed.

## Building

```bash
# Debug build (faster compile, larger binary, no optimizations)
cargo build

# Release build (optimized binary, smaller and faster)
cargo build --release
```

The release binary will be at `target/release/roost`.

## Running from Source

```bash
cargo run
```

Useful for development. Launches the TUI if roost is initialized, or prints a usage hint.

### Testing with a Custom Roost Directory

Set `ROOST_DIR` to point to a temporary or non-default location:

```bash
ROOST_DIR=/tmp/test-roost cargo run
```

This avoids affecting your real `~/.roost/` directory during development.

## Installing Locally

```bash
cargo install --path .
```

Installs the `roost` binary to `~/.cargo/bin/` (which should be in your `$PATH` if you installed Rust via rustup).

To uninstall:

```bash
cargo uninstall roost
```

## Linting

```bash
# Format check
cargo fmt --check

# Auto-format
cargo fmt

# Lint
cargo clippy

# Lint with all warnings
cargo clippy -- -W clippy::all
```

## Cross-Compilation

Roost uses platform-specific symlink code (`#[cfg(unix)]` and `#[cfg(windows)]`). To build for a different target:

```bash
# Install the target
rustup target add x86_64-unknown-linux-gnu

# Build
cargo build --target x86_64-unknown-linux-gnu --release
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `ratatui` | Terminal UI framework |
| `crossterm` | Cross-platform terminal manipulation |
| `color-eyre` | Error handling with colored backtraces |
| `dialoguer` | Interactive CLI prompts (init wizard) |
| `dirs` | Platform-aware directory paths |
| `serde` | Serialization/deserialization |
| `toml` | TOML parsing and writing |

All dependencies are permissively licensed (MIT or Apache-2.0).
