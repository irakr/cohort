# Cohort dev tasks. The hub is a normal cargo binary; the app is a Tauri 2
# desktop shell around the Vite frontend.

.PHONY: dev-hub dev-app types test test-hub test-app db-reset

dev-hub:
	cargo run -p cohort-hub

dev-app:
	cd app && npx tauri dev

# Regenerate TypeScript bindings from the Rust wire types (committed).
# The ts-export feature gates ts-rs's generated export tests, so a plain
# `cargo test` never writes TypeScript files.
types:
	TS_RS_EXPORT_DIR=../../app/src/api/types cargo test -p cohort-hub -p cohort-agent --features cohort-hub/ts-export,cohort-agent/ts-export export_bindings
	cd app/src/api/types && rm -f index.ts && (for f in *.ts; do echo "export type * from \"./$${f%.ts}\";"; done) > index.tmp && mv index.tmp index.ts

test-hub:
	cargo test -p cohort-hub -p cohort-agent

test-app:
	cd app && npm test

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
