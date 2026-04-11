# VM Setup Scripts

Install roost from source on a fresh VM and create test dotfile data.

## Quick Start

```sh
export ROOST_REPO_URL=https://github.com/mt-22/roost.git
sh setup-ubuntu.sh        # Ubuntu/Debian
sh setup-fedora.sh        # Fedora/RHEL
sh setup-nixos.sh         # NixOS
sh setup-windows.sh       # WSL on Windows
```

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `ROOST_REPO_URL` | *(required)* | Git clone URL |
| `ROOST_BRANCH` | `main` | Branch to checkout |
| `INSTALL_DIR` | `~/roost` | Clone destination |
| `SKIP_CLONE` | `0` | Set to `1` to skip git clone |

## Test Data

Create fake dotfiles for testing roost's add/sync/restore/doctor flow:

```sh
sh setup-test-data.sh       # create test dotfiles
sh setup-test-data.sh clean # remove them
```

Creates configs for: fakeapp, starship, alacritty, bat, git, zellij, tmux, and more under `~/.config/` and `~/`.

## What Each Script Does

1. Installs system packages (compiler, openssl, git, curl)
2. Installs Rust via rustup (skips if already installed)
3. Clones the roost repo
4. Builds `roost` in release mode

The binary ends up at `$INSTALL_DIR/target/release/roost`.
