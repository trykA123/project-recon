# Runs the source-only RECON quality gate; no install, deployment, or library upload.
.PHONY: check
check:
	cargo fmt --all -- --check
	cargo clippy --locked --all-targets -- -D warnings
	cargo test --locked --no-fail-fast
	cargo build --locked --release
	cargo audit --deny warnings
	@echo "RECON gate PASS"
