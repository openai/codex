all: test

build:
	@cargo build --all-features

doc:
	@cargo doc --all-features

test:
	@cargo test
	@cargo test --all-features
	@cargo test --no-default-features
	@cargo test --no-default-features --features bytes

.PHONY: wasi-test
wasi-test:
	@cargo test --all-features --target=wasm32-wasip1 -- --nocapture

format:
	@rustup component add rustfmt 2> /dev/null
	@cargo fmt --all

format-check:
	@rustup component add rustfmt 2> /dev/null
	@cargo fmt --all -- --check

lint:
	@rustup component add clippy 2> /dev/null
	@cargo clippy

.PHONY: all doc test format format-check lint
