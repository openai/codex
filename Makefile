.PHONY: fix format test install

fix:
	cd codex-rs && cargo clippy --fix --all-features --tests --allow-dirty

format:
	cd codex-rs && cargo fmt

test:
	cd codex-rs && cargo test --all-features

install: 
	cd codex-rs && cargo fetch

