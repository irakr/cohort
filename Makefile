# Cohort dev tasks. The hub is a normal cargo binary; the app is a Tauri 2
# desktop shell around the Vite frontend.
#
# Reproducible versions everywhere:
# - rust-toolchain.toml pins the exact Rust toolchain (rustup honors it)
# - Cargo.lock and app/package-lock.json pin every dependency; cargo runs
#   with --locked and npm installs with `npm ci`, so a drifted lockfile
#   fails loudly instead of silently updating

.PHONY: bootstrap setup dev-hub dev-app types test test-hub test-app db-reset

# Recipes run under bash, and any target that needs Node loads nvm first
# when node is not already on PATH (make's non-interactive shell does not
# read .bashrc, so an nvm-installed Node would otherwise be invisible).
SHELL := /usr/bin/env bash
NODE_ENV = if ! command -v npm >/dev/null 2>&1 && [ -s "$$HOME/.nvm/nvm.sh" ]; then source "$$HOME/.nvm/nvm.sh" >/dev/null; fi

# Fresh machine: installs system packages, rustup, nvm, the pinned
# toolchain/Node, and all locked dependencies. Idempotent.
bootstrap:
	./scripts/bootstrap.sh

# Lighter re-sync when the tools already exist (e.g. after a pull).
setup:
	rustup show active-toolchain
	@$(NODE_ENV); cd app && npm ci

dev-hub:
	cargo run --locked -p cohort-hub

dev-app:
	@$(NODE_ENV); cd app && npx tauri dev

# Regenerate TypeScript bindings from the Rust wire types (committed).
# The ts-export feature gates ts-rs's generated export tests, so a plain
# `cargo test` never writes TypeScript files.
types:
	TS_RS_EXPORT_DIR=../../app/src/api/types cargo test --locked -p cohort-hub -p cohort-agent --features cohort-hub/ts-export,cohort-agent/ts-export export_bindings
	cd app/src/api/types && rm -f index.ts && (for f in *.ts; do echo "export type * from \"./$${f%.ts}\";"; done) > index.tmp && mv index.tmp index.ts

test-hub:
	cargo test --locked -p cohort-hub -p cohort-agent -p cohort-dirs

test-app:
	@$(NODE_ENV); cd app && npm test

test: test-hub test-app

# The hub database lives in the cohort config namespace by default
# (macOS/Linux paths below); the repo-local file covers COHORT_DB overrides.
db-reset:
	rm -f cohort.db cohort.db-shm cohort.db-wal
	rm -f "$$HOME/Library/Application Support/cohort/config/cohort.db" \
	      "$$HOME/Library/Application Support/cohort/config/cohort.db-shm" \
	      "$$HOME/Library/Application Support/cohort/config/cohort.db-wal" \
	      "$$HOME/.config/cohort/config/cohort.db" \
	      "$$HOME/.config/cohort/config/cohort.db-shm" \
	      "$$HOME/.config/cohort/config/cohort.db-wal"
