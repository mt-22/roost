[![Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org/) [![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

```text
                  ,.
                 (\(\)
 ,_              ;  o >
  {`-.          /  (_)
  `={\`-._____/`   |
   `-{ /    -=`\   |
    `={  -= = _/   /
       `\  .-'   /`               _ __ ___   ___  ___| |_ 
        {`-,__.'=                | '__/ _ \ / _ \/ __| __|
         ||                      | | | (_) | (_) \__ \ |_ 
         | \                     |_|  \___/ \___/|___/\__|
 --------/\/\---------------------------------------------
```

# `roost` — per-device dotfile profiles, symlink-backed, git-synced

Roost is a terminal-based dotfile manager that moves your application configuration files into a central directory (`~/.roost/`), organized by device-specific **profiles**, and creates symlinks back to their original locations. Optionally sync everything via Git for multi-device setups.

## Features

- Per-device profiles — different machines get different config sets
- Symlink-based — apps write to their standard locations, edits are reflected instantly
- Interactive TUI and CLI — browse and manage configs in a terminal interface, or script with subcommands
- Git sync — push/pull your entire dotfile store to a remote repository
- Cross-profile sharing — link apps from one profile into another (zero-copy) or copy them independently
- Built-in diagnostics — `roost doctor` checks for broken symlinks, config inconsistencies, orphaned files
- Git history — `diff`, `log`, `undo`, and `rollback` commands for config change management
- Fuzzy search — quickly find apps and files in the TUI
- Cross-platform — works on macOS, Linux, and Windows (with symlink support)
- Confidence-scored app detection — automatically identifies 120+ known applications during setup

## Prerequisites

- Rust 1.85 or later (edition 2024)
- `git` CLI

## Installation

From source:

```sh
git clone <repo-url>
cd roost
cargo install --path .
```

## Quick Start

1. `roost init` — Interactive setup wizard: choose a profile name, set up git remote (optional), select apps to manage
2. After init, your configs are symlinked into `~/.roost/<profile>/`
3. `roost` — Launch the TUI to browse apps, edit configs, manage profiles
4. `roost sync` — Push changes to your remote repository
5. On another device, `roost init` with the same remote, and your configs are available

## CLI Commands

| Command | Description |
|---------|-------------|
| `roost init` | Initialize roost (interactive setup wizard) |
| `roost` | Launch the TUI (default when no command given) |
| `roost add <path>` | Ingest a config path into the active profile |
| `roost remove <app>` | Stop managing an app, restore original files |
| `roost restore` | Repair all symlinks |
| `roost status` | Show managed apps and link status |
| `roost where <app>` | Print where an app's files live in `~/.roost/` |
| `roost sync` | Stage, commit, pull (rebase), and push |
| `roost diff` | Show uncommitted changes |
| `roost log` | Show recent commits |
| `roost undo [n]` | Undo the last n commit(s) (destructive) |
| `roost rollback <hash>` | Reset to a specific commit (destructive) |
| `roost doctor` | Run diagnostics on config and symlinks |
| `roost remote` | Show the current git remote URL |
| `roost remote set <url>` | Set the git remote URL |
| `roost profile add <name>` | Create a new profile (clones active profile's apps) |
| `roost profile add <name> --empty` | Create an empty profile |
| `roost profile list` | List all profiles (active marked with `*`) |
| `roost profile switch <name>` | Switch active profile (relinks symlinks) |
| `roost profile delete <name>` | Delete a profile (restores files, auto-commits) |
| `roost profile rename <old> <new>` | Rename a profile (moves files, updates refs) |

## TUI Keybindings

### Navigation

| Key | Action |
|-----|--------|
| `j` / `Down` | Move down |
| `k` / `Up` | Move up |
| `h` / `Left` | Go to parent (Miller columns) / switch focus to Apps |
| `l` / `Right` | Enter directory / open file / switch focus to Files |
| `Tab` | Toggle focus between Apps and Files panels |
| `/` | Fuzzy search apps or files |
| `q` / `Esc` | Quit |

### Actions

| Key | Action |
|-----|--------|
| `e` / `Enter` | Open file in `$EDITOR` |
| `o` | Open primary config of selected app |
| `p` | Set primary config for current file |
| `a` | Add a new app |
| `x` | Remove selected app |
| `s` | Sync with remote |
| `d` | Show working tree diff |
| `g` | View git log |
| `u` | Undo last commit |
| `f` | Import app from another profile (symlink) |
| `m` | Paste app into another profile (copy) |
| `i` | Manage ignore patterns |
| `P` | Profile dialog (switch / create / delete) |
| `?` | Help (searchable keybind reference) |

## How It Works

`~/.roost/` is the home of all managed configs.

- `roost.toml` — shared, git-tracked config (profiles, apps, ignore patterns, remote URL)
- `local.toml` — device-local, git-ignored (active profile, OS info, per-device link paths)
- Profile directories (`~/.roost/<profile>/`) hold the actual config files

During setup, the original config file is moved into `~/.roost/<profile>/` and a symlink is created at the original location pointing to the roost copy. Apps continue reading and writing to their standard paths — all changes are stored inside the roost directory.

```
~/.roost/
├── roost.toml          # Shared config (git-tracked)
├── local.toml          # Device-local config (git-ignored)
├── laptop/             # Profile directory
│   ├── nvim/           # App configs (directories)
│   │   └── init.lua
│   └── misc/           # Standalone files
│       └── .gitconfig
└── desktop/
    └── ...
```

## Multi-Device Setup

### Shared profile

**Device A (desktop):** Run `roost init`, create profile "shared" with common apps (zsh, git, tmux, nvim), set up git remote, `roost sync`.

**Device B (laptop):** Run `roost init` with the same git remote — pulls the shared profile. Create a new "laptop" profile for device-specific configs. Add the laptop's device-specific configs. Then use the TUI to share common apps:

- **Link from profile** (press `f` in the Apps panel): Select the "shared" profile, then pick an app (e.g., nvim). This creates a symlink chain so both profiles read from the same files:
  ```
  ~/.config/nvim → ~/.roost/laptop/nvim → ~/.roost/shared/nvim
  ```
  Edits from either device are visible everywhere — perfect for configs you want identical across machines.

- **Paste into profile** (press `m` in the Apps panel): Select the "shared" profile, then pick an app. This creates an independent copy so each profile can diverge. Ideal for apps that need device-specific tweaks.

**Sync:** On each device, `roost sync` pushes/pulls changes. The symlink chains are stored in config, so they work on any device after a pull.

```
                    ┌─ laptop/nvim → shared/nvim  (symlink, shared config)
~/.config/nvim ─────┤
                    └─ desktop/nvim (independent copy)

~/.config/zshrc ──── shared/misc/.zshrc  (linked from both profiles)
```

## Contributing

Contributions are welcome. Please see:

- [docs/building.md](docs/building.md) — development setup
- [docs/testing.md](docs/testing.md) — testing guidelines
- [docs/contributing.md](docs/contributing.md) — contribution standards

## License

MIT — see [LICENSE](LICENSE)
