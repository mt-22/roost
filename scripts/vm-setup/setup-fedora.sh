#!/bin/sh
# setup-fedora.sh — install dependencies and build roost on Fedora/RHEL
#
# Usage:
#   export ROOST_REPO_URL=https://github.com/mt-22/roost.git
#   sh setup-fedora.sh

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
    log_step "Installing system dependencies (dnf)"

    sudo dnf install -y \
        gcc \
        gcc-c++ \
        make \
        pkg-config \
        openssl-devel \
        git \
        curl \
        file

    log_ok "System dependencies installed"
}

install_system_deps
run_setup
