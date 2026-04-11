#!/bin/sh
# setup-windows.sh — install dependencies and build roost on Windows via WSL
#
# This script sets up a WSL environment for building roost. If you're running
# native Windows PowerShell, this will guide you to install WSL first.
#
# Usage (from WSL or Git Bash):
#   export ROOST_REPO_URL=https://github.com/mt-22/roost.git
#   sh setup-windows.sh

set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
COMMON="$SCRIPT_DIR/setup-common.sh"

if [ ! -f "$COMMON" ]; then
    echo "ERROR: setup-common.sh not found at $COMMON"
    echo "Download it alongside this script or run from the roost repo."
    exit 1
fi

. "$COMMON"

detect_wsl() {
    case "$(uname -r)" in
        *Microsoft*|*microsoft*)
            log_ok "Running under WSL"
            ;;
        *WSL*|*wsl*)
            log_ok "Running under WSL2"
            ;;
        MINGW*|MSYS*)
            log_warn "Running under MSYS2/Git Bash — limited symlink support"
            ;;
        *)
            log_warn "Not running under WSL"
            log_warn "For best results on Windows, use WSL2:"
            log_warn "  wsl --install"
            log_warn ""
            log_warn "Continuing anyway..."
            ;;
    esac
}

install_system_deps() {
    log_step "Installing system dependencies"

    if command -v apt-get >/dev/null 2>&1; then
        sudo apt-get update -qq
        sudo apt-get install -y --no-install-recommends \
            build-essential \
            pkg-config \
            libssl-dev \
            git \
            curl \
            file
        log_ok "Dependencies installed via apt"
    elif command -v dnf >/dev/null 2>&1; then
        log_warn "Fedora-based WSL detected — running dnf install"
        sudo dnf install -y gcc gcc-c++ make pkg-config openssl-devel git curl file
        log_ok "Dependencies installed via dnf"
    elif command -v pacman >/dev/null 2>&1; then
        sudo pacman -Syu --noconfirm gcc pkg-config openssl git curl file
        log_ok "Dependencies installed via pacman"
    else
        log_err "Unknown package manager. Install gcc, pkg-config, openssl-dev, and git manually."
        exit 1
    fi
}

detect_wsl
install_system_deps
run_setup
