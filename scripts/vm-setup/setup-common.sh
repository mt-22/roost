#!/bin/sh
# setup-common.sh — shared logic for roost VM setup scripts
#
# Usage: source this from an OS-specific script, or run directly:
#   curl -sL https://raw.githubusercontent.com/mt-22/roost/main/scripts/vm-setup/setup-common.sh | sh
#
# Environment variables:
#   ROOST_REPO_URL  — git clone URL (default: auto-detected or prompts)
#   ROOST_BRANCH    — branch to checkout (default: main)
#   INSTALL_DIR     — where to clone (default: ~/roost)
#   SKIP_CLONE      — set to 1 to skip git clone (use existing repo)

set -eu

ROOST_REPO_URL="${ROOST_REPO_URL:-}"
ROOST_BRANCH="${ROOST_BRANCH:-main}"
INSTALL_DIR="${INSTALL_DIR:-$HOME/roost}"
SKIP_CLONE="${SKIP_CLONE:-0}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

log_step() {
    printf "\n${CYAN}${BOLD}==> %s${NC}\n" "$1"
}

log_ok() {
    printf "${GREEN}  ✓ %s${NC}\n" "$1"
}

log_warn() {
    printf "${YELLOW}  ⚠ %s${NC}\n" "$1"
}

log_err() {
    printf "${RED}  ✗ %s${NC}\n" "$1"
}

require_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        log_err "$1 is required but not installed"
        exit 1
    fi
}

detect_os() {
    OS_ID=""
    OS_VERSION=""
    OS_FAMILY=""

    case "$(uname -s)" in
        Linux)
            if [ -f /etc/os-release ]; then
                . /etc/os-release
                OS_ID="${ID}"
                OS_VERSION="${VERSION_ID:-}"
            elif [ -f /usr/lib/os-release ]; then
                . /usr/lib/os-release
                OS_ID="${ID}"
                OS_VERSION="${VERSION_ID:-}"
            else
                OS_ID="linux"
            fi
            OS_FAMILY="unix"
            ;;
        Darwin)
            OS_ID="macos"
            OS_VERSION="$(sw_vers -productVersion 2>/dev/null || echo '')"
            OS_FAMILY="unix"
            ;;
        MINGW*|MSYS*|CYGWIN*)
            OS_ID="windows"
            OS_FAMILY="windows"
            ;;
        *)
            OS_ID="$(uname -s | tr '[:upper:]' '[:lower:]')"
            OS_FAMILY="unix"
            ;;
    esac

    ARCH="$(uname -m 2>/dev/null || uname -p 2>/dev/null || echo 'unknown')"
    log_ok "Detected: ${OS_ID} ${OS_VERSION} (${ARCH})"
}

install_rust() {
    if command -v rustup >/dev/null 2>&1 && command -v cargo >/dev/null 2>&1; then
        log_ok "Rust already installed: $(rustc --version)"
        return 0
    fi

    log_step "Installing Rust via rustup"

    case "$OS_ID" in
        nixos)
            log_warn "On NixOS, prefer 'nix develop' or add rustup to your config"
            log_warn "Attempting rustup install — this may fail on NixOS"
            ;;
    esac

    if command -v rustup >/dev/null 2>&1; then
        rustup update
    else
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    fi

    . "$HOME/.cargo/env" 2>/dev/null || true

    if command -v rustc >/dev/null 2>&1; then
        log_ok "Rust installed: $(rustc --version)"
    else
        log_err "Rust installation failed"
        log_err "Make sure \$HOME/.cargo/bin is in your PATH"
        exit 1
    fi
}

clone_roost() {
    if [ "$SKIP_CLONE" = "1" ]; then
        if [ -d "$INSTALL_DIR/.git" ]; then
            log_ok "Using existing repo at $INSTALL_DIR"
            return 0
        else
            log_err "SKIP_CLONE=1 but $INSTALL_DIR is not a git repo"
            exit 1
        fi
    fi

    if [ -d "$INSTALL_DIR/.git" ]; then
        log_ok "Repo already exists at $INSTALL_DIR — pulling latest"
        git -C "$INSTALL_DIR" pull --ff-only || true
        git -C "$INSTALL_DIR" checkout "$ROOST_BRANCH" 2>/dev/null || true
        return 0
    fi

    if [ -z "$ROOST_REPO_URL" ]; then
        log_err "ROOST_REPO_URL is not set and no repo exists at $INSTALL_DIR"
        log_err "Set ROOST_REPO_URL, e.g.:"
        log_err "  export ROOST_REPO_URL=https://github.com/mt-22/roost.git"
        log_err "  sh setup-ubuntu.sh"
        exit 1
    fi

    log_step "Cloning roost from $ROOST_REPO_URL"
    git clone --branch "$ROOST_BRANCH" "$ROOST_REPO_URL" "$INSTALL_DIR"
    log_ok "Cloned to $INSTALL_DIR"
}

build_roost() {
    log_step "Building roost (release)"

    cargo build --release --manifest-path "$INSTALL_DIR/Cargo.toml"

    BINARY="$INSTALL_DIR/target/release/roost"
    if [ -f "$BINARY" ]; then
        log_ok "Built: $BINARY"
        printf "\n${CYAN}To use roost, either:${NC}\n"
        printf "  1. Add to PATH: export PATH=\"$INSTALL_DIR/target/release:\$PATH\"\n"
        printf "  2. Copy to a PATH dir: sudo cp %s /usr/local/bin/\n" "$BINARY"
    else
        log_err "Build succeeded but binary not found at $BINARY"
        exit 1
    fi
}

run_setup() {
    detect_os
    install_rust
    require_cmd git
    clone_roost
    build_roost

    printf "\n${GREEN}${BOLD}Setup complete!${NC}\n"
    printf "Run ${CYAN}%s/target/release/roost init${NC} to configure dotfiles.\n" "$INSTALL_DIR"
}

if [ "${0##*/}" = "setup-common.sh" ]; then
    run_setup
fi
