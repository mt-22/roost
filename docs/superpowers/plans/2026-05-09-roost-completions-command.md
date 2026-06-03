# Add `roost completions <shell>` Command

## Goal
Add a `roost completions <shell>` command that prints shell completion scripts to stdout, enabling users to source them dynamically or redirect to a file.

## Why
- Auto-installation to disk is not 100% robust (shell detection, directory conventions, permissions).
- Printing to stdout is the de-facto standard (starship, zoxide, fzf, atuin).
- Works reliably for `cargo install` users and package manager installs alike.
- Users can either `eval "$(roost completions bash)"` or save to file manually.

## Changes Required

### 1. Restore Completion Infrastructure
- **Update `src/main.rs`:**
  - Re-add `Complete { shell: String }` variant to `Commands` enum (or rename to `Completions`).
  - Implement `cmd_completions(shell: String)` that parses the shell and calls `clap_complete::generate()` to stdout.
- **Update `Cargo.toml`:**
  - Re-add `clap_complete = "4"` to `[dependencies]` (remove from `[build-dependencies]` since build.rs no longer needs it).
- **Remove `build.rs`:**
  - Delete the file. Build-time generation is being replaced by runtime generation.
- **Simplify `src/cli_def.rs` / `src/cli.rs`:**
  - Since `build.rs` is gone, we can inline the CLI definition back into `src/cli.rs` normally (no `include!` needed).
  - Alternatively, keep `cli.rs` as a regular module file with the structs defined directly.

### 2. Command Design
```bash
roost completions bash    # prints bash completion script
roost completions zsh     # prints zsh completion script
roost completions fish    # prints fish completion script
roost completions powershell  # prints PowerShell completion script
```
Invalid shells produce a clear error message.

### 3. Tests
- **Create `tests/completions.rs`:**
  - Test that `roost completions bash` succeeds and contains "roost".
  - Test that `roost completions zsh` succeeds.
  - Test that `roost completions notashell` fails with a clear error.

### 4. Documentation
- Update README with usage examples:
  ```bash
  eval "$(roost completions bash)"
  roost completions zsh > ~/.zsh/completions/_roost
  ```

## Notes
- This approach is simpler than the build-time generation we just added.
- The tradeoff is that `cargo install` users must run the command to get completions, but they gain a guaranteed-in-sync completion script.
- Package managers can still ship pre-generated scripts if desired, but it's no longer the primary mechanism.
