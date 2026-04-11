#!/bin/sh
# setup-ubuntu.sh — install dependencies and build roost on Ubuntu/Debian
#
# Usage:
#   export ROOST_REPO_URL=https://github.com/mt-22/roost.git
#   sh setup-ubuntu.sh
#
# Or non-interactively:
#   ROOST_REPO_URL=https://github.com/mt-22/roost.git sh setup-ubuntu.sh

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
    log_step "Installing system dependencies (apt)"

    sudo apt-get update -qq
    sudo apt-get install -y --no-install-recommends \
        build-essential \
        pkg-config \
        libssl-dev \
        git \
        curl \
        file

    log_ok "System dependencies installed"
}

install_system_deps
run_setup
