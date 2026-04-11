#!/bin/sh
# setup-nixos.sh — install dependencies and build roost on NixOS
#
# On NixOS, system packages come from configuration.nix. This script
# uses nix-shell to provide a temporary environment with build deps.
#
# Usage:
#   export ROOST_REPO_URL=https://github.com/mt-22/roost.git
#   sh setup-nixos.sh
#
# Or, for a persistent dev shell, add to your configuration.nix:
#   environment.systemPackages = with pkgs; [ rustc cargo gcc openssl git ];

set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
COMMON="$SCRIPT_DIR/setup-common.sh"

if [ ! -f "$COMMON" ]; then
    echo "ERROR: setup-common.sh not found at $COMMON"
    echo "Download it alongside this script or run from the roost repo."
    exit 1
fi

. "$COMMON"

install_system_deps() {
    if command -v git >/dev/null 2>&1 && command -v gcc >/dev/null 2>&1; then
        log_ok "git and gcc already available"
        return 0
    fi

    log_step "NixOS detected — installing build tools via nix-shell"

    if ! command -v nix-shell >/dev/null 2>&1; then
        log_err "nix-shell not found. Install Nix first:"
        log_err "  sh <(curl -L https://nixos.org/nix/install) --daemon"
        exit 1
    fi

    log_warn "These packages are available in the nix-shell only."
    log_warn "For persistent access, add them to your configuration.nix."
}

NIXPKGS="https://github.com/NixOS/nixpkgs/archive/nixos-unstable.tar.gz"

build_roost_nix() {
    log_step "Building roost via nix-shell"

    nix-shell \
        --packages "with import (fetchTarball $NIXPKGS) {}; [ gcc pkg-config openssl git rustc cargo ]" \
        --run "
            export CARGO_HOME=\"\$HOME/.cargo\"
            export RUSTUP_HOME=\"\$HOME/.rustup\"
            cargo build --release --manifest-path \"$INSTALL_DIR/Cargo.toml\"
        "

    BINARY="$INSTALL_DIR/target/release/roost"
    if [ -f "$BINARY" ]; then
        log_ok "Built: $BINARY"
        printf "\n${CYAN}To use roost:${NC}\n"
        printf "  1. Run: nix-shell -p gcc pkg-config openssl --run '%s/target/release/roost init'\n" "$INSTALL_DIR"
        printf "  2. Or add roost to your system configuration.nix\n"
    else
        log_err "Build failed"
        exit 1
    fi
}

install_system_deps

detect_os
install_rust
require_cmd git
clone_roost

if [ "${OS_ID}" = "nixos" ]; then
    build_roost_nix
else
    build_roost
fi

printf "\n${GREEN}${BOLD}Setup complete!${NC}\n"
