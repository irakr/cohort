# Cohort dev tasks. The hub is a normal cargo binary; the app is a Tauri 2
# desktop shell around the Vite frontend.

.PHONY: dev-hub dev-app types test test-hub test-app db-reset

dev-hub:
	cargo run -p cohort-hub

dev-app:
	cd app && npx tauri dev

# Regenerate TypeScript bindings from the Rust wire types (committed).
types:
	TS_RS_EXPORT_DIR=../../app/src/api/types cargo test -p cohort-hub -p cohort-agent export_bindings
	cd app/src/api/types && rm -f index.ts && (for f in *.ts; do echo "export type * from \"./$${f%.ts}\";"; done) > index.tmp && mv index.tmp index.ts

test-hub:
	cargo test -p cohort-hub -p cohort-agent

test-app:
	cd app && npm test

test: test-hub test-app

db-reset:
	rm -f cohort.db cohort.db-shm cohort.db-wal
