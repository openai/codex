SHELL := /bin/bash
MAKEFLAGS += --no-print-directory

DOCKER_COMPOSE ?= docker compose
DOCKER_SERVICE ?= codex-dev
COMPOSE_BAKE ?= true
RUN_DIR ?= /workspace/codex/.cache/codex-playground

.PHONY: help setup test build run test_and_run shell
.SILENT:

help:
	@echo "Targets:"
	@echo "  make setup           Build Docker image and warm Rust deps once"
	@echo "  make test            Run codex-cli tests in Docker"
	@echo "  make build           Build Codex in Docker (no host export)"
	@echo "  make run             Build, then run Codex in Docker (supports RUN_DIR='...')"
	@echo "  make test_and_run    Test, then build and run Codex (supports RUN_DIR='...')"
	@echo "  make shell           Open interactive shell in Docker dev container"

setup:
	start_s=$$(date +%s); \
	tmp=$$(mktemp); \
	mkdir -p .cache/codex-home; \
	if COMPOSE_BAKE=$(COMPOSE_BAKE) $(DOCKER_COMPOSE) build $(DOCKER_SERVICE) >$$tmp 2>&1 && \
		$(DOCKER_COMPOSE) run --rm --user root $(DOCKER_SERVICE) /bin/bash -lc 'set -euo pipefail; \
			mkdir -p /home/ubuntu/.cargo /home/ubuntu/.rustup /home/ubuntu/.codex; \
			chown -R ubuntu:ubuntu /home/ubuntu/.cargo /home/ubuntu/.rustup /home/ubuntu/.codex' >>$$tmp 2>&1 && \
		$(DOCKER_COMPOSE) run --rm $(DOCKER_SERVICE) /bin/bash -lc 'set -euo pipefail; \
			cd /workspace/codex/codex-rs; \
			just install' >>$$tmp 2>&1; then \
		end_s=$$(date +%s); \
		secs=$$(awk "BEGIN { printf \"%.2f\", $$end_s-$$start_s }"); \
		echo "Setup: OK ($${secs}s)"; \
	else \
		cat $$tmp; \
		rm -f $$tmp; \
		exit 1; \
	fi; \
	rm -f $$tmp

test:
	start_s=$$(date +%s); \
	tmp=$$(mktemp); \
	if $(DOCKER_COMPOSE) run --rm $(DOCKER_SERVICE) /bin/bash -lc 'set -euo pipefail; \
		cd /workspace/codex/codex-rs; \
		cargo test -p codex-cli' >$$tmp 2>&1; then \
		end_s=$$(date +%s); \
		secs=$$(awk "BEGIN { printf \"%.2f\", $$end_s-$$start_s }"); \
		echo "Test: OK ($${secs}s)"; \
	else \
		cat $$tmp; \
		rm -f $$tmp; \
		exit 1; \
	fi; \
	rm -f $$tmp

build:
	start_s=$$(date +%s); \
	tmp=$$(mktemp); \
	if $(DOCKER_COMPOSE) run --rm $(DOCKER_SERVICE) /bin/bash -lc 'set -euo pipefail; \
		cd /workspace/codex/codex-rs; \
		cargo build --bin codex' >$$tmp 2>&1; then :; else \
		cat $$tmp; rm -f $$tmp; exit 1; \
	fi; \
	end_s=$$(date +%s); \
	secs=$$(awk "BEGIN { printf \"%.2f\", $$end_s-$$start_s }"); \
	echo "Build: OK ($${secs}s)"; \
	echo "Ready: make run"; \
	rm -f $$tmp

run: build
	$(DOCKER_COMPOSE) run --rm $(DOCKER_SERVICE) /bin/bash -lc "set -euo pipefail; mkdir -p '$(RUN_DIR)'; cd '$(RUN_DIR)'; /workspace/codex/codex-rs/target/debug/codex"

test_and_run: test
	$(MAKE) run RUN_DIR='$(RUN_DIR)'

shell:
	$(DOCKER_COMPOSE) run --rm $(DOCKER_SERVICE)
