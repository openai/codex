.PHONY: fix format test

fix:
	cd codex-rs && cargo clippy --fix --tests --allow-dirty

format:
	cd codex-rs && cargo fmt

test:
	cd codex-rs && cargo test --all-features

