#!/bin/sh
# setup-test-data.sh — create fake dotfile configs for testing roost
#
# Creates a set of realistic-looking dotfile configs in standard locations
# so you can test roost's add, sync, profile, restore, and doctor commands
# without risking your real configs.
#
# Usage:
#   sh setup-test-data.sh          # create all test data
#   sh setup-test-data.sh clean    # remove all test data
#
# All created files/dirs are tagged with a comment so they can be identified
# and cleaned up safely.

set -eu

TEST_TAG="# ROOST-TEST-DATA — safe to delete"

XDG_CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}"
XDG_DATA="${XDG_DATA_HOME:-$HOME/.local/share}"

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

created=""

ensure_dir() {
    mkdir -p "$1"
}

create_file() {
    ensure_dir "$(dirname "$1")"
    printf "%s\n%s\n" "$TEST_TAG" "$2" > "$1"
    created="$created $1"
}

clean() {
    log "Cleaning test data..."

    rm -rf "$XDG_CONFIG/fakeapp" 2>/dev/null
    rm -rf "$XDG_CONFIG/starship" 2>/dev/null
    rm -rf "$XDG_CONFIG/alacritty" 2>/dev/null
    rm -rf "$XDG_CONFIG/bat" 2>/dev/null
    rm -rf "$XDG_CONFIG/git" 2>/dev/null
    rm -rf "$XDG_CONFIG/zellij" 2>/dev/null
    rm -f "$HOME/.tmux.conf" 2>/dev/null
    rm -f "$HOME/.bashrc.d/roost-test.sh" 2>/dev/null
    rmdir "$HOME/.bashrc.d" 2>/dev/null || true
    rm -f "$HOME/.zshenv.local" 2>/dev/null
    rm -rf "$XDG_DATA/fakeapp" 2>/dev/null

    log "Done."
}

log() {
    printf "${CYAN}  → %s${NC}\n" "$1"
}

ok() {
    printf "${GREEN}  ✓ %s${NC}\n" "$1"
}

if [ "${1:-}" = "clean" ]; then
    clean
    exit 0
fi

log "Creating test dotfile data..."

create_file "$XDG_CONFIG/fakeapp/config.toml" 'theme = "dark"
font_size = 14
max_tabs = 10

[plugins]
demo = true

[server]
host = "127.0.0.1"
port = 8080'

create_file "$XDG_CONFIG/fakeapp/extra.yaml" 'database:
  host: localhost
  port: 5432
  name: fakeapp_db'

create_file "$XDG_CONFIG/starship.toml" 'add_newline = false
format = "$directory$git_branch$character"

[character]
success_symbol = "[❯](bold green)"
error_symbol = "[❯](bold red)"

[directory]
truncation_length = 3'

create_file "$XDG_CONFIG/alacritty/alacritty.yml" 'font:
  normal:
    family: JetBrains Mono
    size: 12.0

colors:
  primary:
    background: "0x1e1e2e"
    foreground: "0xcdd6f4"

window:
  padding:
    x: 5
    y: 5'

create_file "$XDG_CONFIG/bat/config" 'theme = "Nord"
map-syntax = ["*.conf:INI", "Pipfile:toml"]
pager = "less -FRS"'

create_file "$XDG_CONFIG/git/config" '[user]
    name = Test User
    email = test@example.com

[core]
    editor = vim
    autocrlf = input

[pull]
    rebase = true'

create_file "$XDG_CONFIG/git/ignore" '*.swp
*.swo
.DS_Store
Thumbs.db
__pycache__/

create_file "$XDG_CONFIG/zellij/config.kdl" 'default_layout "compact"
default_mode "locked"
mouse_mode false

pane_frames false
default_shell "zsh"

plugins {
    tab-bar location="bottom"
}'

create_file "$HOME/.tmux.conf" 'set -g prefix C-a
set -g mouse on
set -g base-index 1
set -g status-position bottom

set-option -g status-style "bg=#1e1e2e,fg=#cdd6f4"
set-option -g window-status-current-style "fg=#89b4fa,bold"

bind | split-window -h -c "#{pane_current_path}"
bind - split-window -v -c "#{pane_current_path}"'

create_file "$HOME/.bashrc.d/roost-test.sh" 'export ROOST_TEST="yes"
alias rt="roost test"
export PATH="$HOME/.local/bin:$PATH"'

create_file "$HOME/.zshenv.local" 'export ZDOTDIR="$HOME/.config/zsh"
export ROOST_TEST="yes"'

ensure_dir "$XDG_DATA/fakeapp"
create_file "$XDG_DATA/fakeapp/cache.db.sql" '-- test schema
CREATE TABLE test_items (
    id    INTEGER PRIMARY KEY,
    name  TEXT NOT NULL,
    value TEXT
);'

create_file "$XDG_DATA/fakeapp/logs/.gitkeep" ""

echo ""
ok "Created $(echo "$created" | wc -w | tr -d ' ') test files"
echo ""
printf "${CYAN}Test data created:${NC}\n"
for f in $created; do
    printf "  %s\n" "$f"
done
echo ""
printf "${CYAN}To test roost:${NC}\n"
printf "  1. cd ~/roost && ./target/release/roost init\n"
printf "  2. Add apps when prompted (look for fakeapp, starship, alacritty, etc.)\n"
printf "  3. ./target/release/roost status\n"
printf "  4. ./target/release/roost doctor\n"
printf "  5. ./target/release/roost add ~/.config/zellij\n"
printf "  6. ./target/release/roost sync\n"
echo ""
printf "${CYAN}To clean up:${NC}\n"
printf "  sh scripts/vm-setup/setup-test-data.sh clean\n"
