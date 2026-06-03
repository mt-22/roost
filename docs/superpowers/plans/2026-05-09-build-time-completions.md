# Switch to Build-Time Shell Completions

## Goal
Remove the runtime `roost complete <shell>` command and replace it with build-time completion script generation via `build.rs`.

## Why
- Runtime completion commands are unusual for CLI tools.
- Build-time generation is the standard pattern (used by ripgrep, fd, starship, etc.).
- Completion scripts can be packaged with release artifacts or installed by package managers.

## Changes Required

### 1. Remove Runtime Completion Infrastructure
- **Remove from `src/main.rs`:**
  - Delete `Complete { shell: String }` variant from `Commands` enum.
  - Delete `cmd_complete()` function.
  - Remove `CommandFactory` from the `clap` import (no longer needed in main).
- **Remove from `Cargo.toml`:**
  - Remove `clap_complete` from `[dependencies]` (move to `[build-dependencies]`).
- **Remove `tests/completions.rs`:**
  - Delete the entire file (3 tests for runtime command).

### 2. Extract CLI Definition into a Shared Include File
To avoid duplicating the CLI definition between `build.rs` and the binary, the CLI structs will live in a shared include file consumed by both.

- **Create `src/cli_def.rs`:**
  - Contains the `Cli` struct, `Commands` enum, `ProfileCmd`, and `ProfileAction` enums.
  - Uses fully-qualified `clap::` paths so it compiles both inside the crate and inside `build.rs`.
- **Update `src/cli.rs`:**
  - Simply `include!("cli_def.rs")` to bring the definitions into the library crate.
- **Update `src/main.rs`:**
  - Import `Cli`, `Commands`, `ProfileAction`, and `ProfileCmd` from `roost::cli`.

### 3. Add Build-Time Generation
- **Create `build.rs`:**
  - Uses `include!("src/cli_def.rs")` to get the CLI definition without duplicating it.
  - Imports `clap::CommandFactory` and `clap_complete::{generate_to, Shell}`.
  - Generates completion scripts for `bash`, `zsh`, `fish`, and `powershell`.
  - Writes them to `OUT_DIR`.
- **Update `Cargo.toml`:**
  - Add `clap_complete = "4"` to `[build-dependencies]`.
  - Add `clap = { version = "4", features = ["derive"] }` to `[build-dependencies]` so `build.rs` can use the derive macros.

### 4. Verification
- Run `cargo build` to ensure `build.rs` executes without errors.
- Check that completion files are generated in the output directory.
- Run `cargo test` to confirm all remaining tests pass.

## Notes
- The `include!` approach keeps a single source of truth for the CLI definition. Both `build.rs` and `src/main.rs` consume the same file.
- This avoids the need to duplicate ~60 lines of CLI definition code.
