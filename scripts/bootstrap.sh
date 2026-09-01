#!/usr/bin/env bash
# Cohort machine bootstrap: installs every prerequisite at the pinned
# versions and prepares the repo for `make dev-hub` / `make dev-app`.
#
# Supported: macOS, Debian/Ubuntu. Safe to re-run; every step checks
# before it installs. Needs network; Linux steps use sudo for apt.
#
# Pinned by the repo, applied here:
# - Rust toolchain     rust-toolchain.toml
# - Node version       app/.nvmrc (via nvm)
# - Rust dependencies  Cargo.lock (cargo --locked)
# - npm dependencies   app/package-lock.json (npm ci)

set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
NVM_VERSION="v0.40.1"
NPM_MAJOR=11

say() { printf '\n== %s\n' "$*"; }

cd "$REPO_DIR"

# --- OS detection and system packages ---------------------------------------
OS="$(uname -s)"
case "$OS" in
Darwin)
    say "macOS detected"
    if ! xcode-select -p >/dev/null 2>&1; then
        say "Installing Xcode command line tools (a dialog may appear)"
        xcode-select --install || true
        echo "Re-run this script after the tools finish installing."
        exit 1
    fi
    ;;
Linux)
    if ! command -v apt-get >/dev/null 2>&1; then
        echo "Unsupported Linux distribution (apt-get not found)." >&2
        exit 1
    fi
    say "Debian/Ubuntu detected - installing system packages (sudo)"
    sudo apt-get update -qq
    sudo apt-get install -y \
        libwebkit2gtk-4.1-dev build-essential curl wget file \
        libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev \
        wmctrl imagemagick
    ;;
*)
    echo "Unsupported OS: $OS" >&2
    exit 1
    ;;
esac

# --- Rust (rustup + the toolchain pinned in rust-toolchain.toml) ------------
if ! command -v rustup >/dev/null 2>&1; then
    say "Installing rustup"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --default-toolchain none
fi
# shellcheck disable=SC1091
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
say "Installing the pinned Rust toolchain"
rustup show active-toolchain

# --- Node (nvm + the version pinned in app/.nvmrc) --------------------------
export NVM_DIR="$HOME/.nvm"
if [ ! -s "$NVM_DIR/nvm.sh" ]; then
    say "Installing nvm $NVM_VERSION"
    curl -o- "https://raw.githubusercontent.com/nvm-sh/nvm/$NVM_VERSION/install.sh" | bash
fi
# shellcheck disable=SC1091
. "$NVM_DIR/nvm.sh"
say "Installing the pinned Node version"
# No subshell here: the activation must hold for the rest of this script.
NODE_PIN="$(cat app/.nvmrc)"
nvm install "$NODE_PIN"
nvm use "$NODE_PIN"

# npm: same major everywhere (also avoids the npm 10.9 arborist bug).
if [ "$(npm --version | cut -d. -f1)" -lt "$NPM_MAJOR" ]; then
    say "Upgrading npm to $NPM_MAJOR.x"
    npm install -g "npm@$NPM_MAJOR"
fi

# --- Locked project dependencies --------------------------------------------
say "Installing npm dependencies from the lockfile"
(cd app && npm ci)

say "Fetching Rust dependencies from the lockfile"
cargo fetch --locked

# --- Summary ----------------------------------------------------------------
say "Done. Versions on this machine:"
echo "  rustc  $(rustc --version | cut -d' ' -f2)"
echo "  cargo  $(cargo --version | cut -d' ' -f2)"
echo "  node   $(node --version)"
echo "  npm    $(npm --version)"
echo
echo "Next: 'make dev-hub' and 'make dev-app' (see README.md)."
echo "Note: open a new terminal so nvm/cargo are on PATH there too."
