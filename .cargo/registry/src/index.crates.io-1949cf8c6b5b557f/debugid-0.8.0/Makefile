all: style lint test
.PHONY: all

check: style lint
.PHONY: check

build:
	@cargo build
.PHONY: build

doc:
	@cargo doc
.PHONY: doc

test:
	@cargo test
.PHONY: test

style:
	@rustup component add rustfmt --toolchain stable 2> /dev/null
	cargo +stable fmt -- --check
.PHONY: style

format:
	@rustup component add rustfmt --toolchain stable 2> /dev/null
	@cargo +stable fmt
.PHONY: format

lint:
	@rustup component add clippy --toolchain stable 2> /dev/null
	@cargo +stable clippy --tests -- -D clippy::all
.PHONY: lint
