# Testing Roost

## Running Tests

```bash
# Run all tests
cargo test

# Run a specific test file
cargo test --test profile

# Run tests matching a name pattern
cargo test "add_directory"

# Run with output shown
cargo test -- --nocapture

# Run unit tests only (in src/)
cargo test --lib
```

The project has approximately 93 tests: 83 integration tests and 10 unit tests.

## Test Framework

- **Integration tests** (`tests/`): Use `assert_cmd` to run the `roost` binary as a subprocess, `predicates` for output matching, and `tempfile` for isolated directories.
- **Unit tests** (inline `#[cfg(test)]` modules): Direct function calls with standard `#[test]` attributes, primarily in `app.rs`, `linker.rs`, `scanner.rs`, `git.rs`, and `tui/search.rs`.

## Test Organization

```
tests/
├── helpers/
│   └── mod.rs          # Shared TestRoost fixture
├── add.rs              # roost add command
├── diff.rs             # roost diff command
├── doctor.rs           # roost doctor command
├── edge_cases.rs       # Cross-feature edge cases
├── help.rs             # Help output and unknown commands
├── log.rs              # roost log command
├── multi_profile.rs    # Cross-profile operations
├── profile.rs          # Profile CRUD commands
├── remote.rs           # roost remote command
├── remove.rs           # roost remove command
├── restore.rs          # roost restore command
├── rollback.rs         # roost rollback command
├── status.rs           # roost status command
├── sync.rs             # roost sync command
├── undo.rs             # roost undo command
├── where.rs            # roost where command
└── workflows.rs        # End-to-end workflow tests
```

## Test Helpers

The `TestRoost` fixture (`tests/helpers/mod.rs`) provides isolated environments:

```rust
use helpers::TestRoost;

// Creates a temp directory with home/, home/.config/, home/.roost/
let t = TestRoost::new();

// Write minimal roost.toml and local.toml
t.init_minimal();

// Get a pre-configured assert_cmd::Command with ROOST_DIR and HOME set
t.cmd()
    .arg("status")
    .assert()
    .success();
```

Key methods:
- `TestRoost::new()` — Creates temp home + roost directory structure
- `TestRoost::init_minimal()` — Writes minimal shared + local config
- `TestRoost::init_git()` — Initializes a git repo in the roost directory
- `TestRoost::cmd()` — Returns `assert_cmd::Command` with correct env vars
- `TestRoost::path(relative)` — Resolves paths relative to temp home

## How Integration Tests Work

Each test:
1. Creates an isolated temporary directory (never touches the real filesystem)
2. Sets `ROOST_DIR` and `HOME` environment variables via `TestRoost`
3. Runs the compiled `roost` binary as a subprocess
4. Asserts on:
   - Exit code (`.assert().success()` or `.assert().failure()`)
   - Standard output and stderr (`.stdout(predicate)`, `.stderr(predicate)`)
   - Filesystem state (symlinks, file existence, file contents)
   - Config file contents (parsed `roost.toml` and `local.toml`)

## When to Write Tests

**Every new CLI command** should have integration tests covering:
- Happy path (valid input, correct output)
- Error cases (missing args, not initialized, nonexistent resources)
- Edge cases relevant to the command

**Every new public function** in core modules (`app.rs`, `linker.rs`, `scanner.rs`, `git.rs`) should have unit tests covering:
- Normal operation
- Boundary conditions (empty inputs, max sizes)
- Error paths

**Edge cases** that deserve test coverage:
- Corrupted or empty config files
- Broken symlinks
- Missing directories
- Duplicate operations (adding the same app twice, switching to the active profile, etc.)
- Cross-profile interactions (sourced apps, shared apps)

## Best Practices

### Setup
- Always use `TestRoost::new()` and `init_minimal()` for setup. Don't duplicate the fixture logic.
- For tests that need git, call `t.init_git()` after `t.init_minimal()`.

### Structure
- Test both the happy path and error cases. Every command should have at least one "not initialized" test.
- Verify filesystem state, not just stdout. Check that symlinks exist, point to the right targets, and config files contain the expected data.
- Use `predicates::str::contains()` for flexible output matching rather than exact string comparison.

### Patterns
- Test idempotency where applicable. `roost restore` should be safe to run multiple times. `roost sync` with no changes should succeed without errors.
- For destructive operations (`remove`, `undo`, `rollback`), test both confirmation (`echo "y" | roost remove app`) and abort (no input or `echo "n"`).
- Keep tests independent. Each test should create its own `TestRoost` instance — no shared mutable state between tests.

### Naming
- Use descriptive test names: `fn add_existing_symlink_registers_on_active_profile()` rather than `fn test_add_3()`.

### Assertions
- Combine output assertions with filesystem checks when possible:
  ```rust
  t.cmd()
      .arg("add").arg("nvim")
      .assert()
      .success()
      .stdout(predicates::str::contains("Added 'nvim'"));

  // Also verify the symlink was created
  assert!(t.path(".config/nvim").is_symlink());
  ```
