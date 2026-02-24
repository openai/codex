# HeliosCLI - Fork of OpenAI Codex CLI

## Overview

Forked from https://github.com/openai/codex for performance optimization and custom modifications.

## Setup

```bash
# Clone
git clone https://github.com/openai/codex.git helios-cli
cd helios-cli

# Add upstream for tracking
git remote add upstream https://github.com/openai/codex.git

# Verify remotes
git remote -v
# origin   https://github.com/openai/codex.git (fetch)
# origin   https://github.com/openai/codex.git (push)
# upstream https://github.com/openai/codex.git (fetch)
```

## Tracking Upstream

```bash
# Fetch upstream changes
git fetch upstream

# View branches
git branch -a

# Sync with upstream main
git fetch upstream
git checkout main
git merge upstream/main

# Create working branch
git checkout -b helios-optimization
```

## Directory Structure

```
helios-cli/
├── cli/              # CLI entry point
├── codex-rs/         # Rust implementation
├── packages/         # NPM packages  
├── docs/             # Documentation
└── scripts/          # Build/dev scripts
```

## Performance Branches

| Branch | Focus |
|--------|-------|
| helios-cpu-opt | CPU optimization |
| helios-lat-opt | Latency optimization |
| helios-mem-opt | Memory optimization |

## Benchmarking

```bash
# Run benchmarks
cargo bench -p codex-core

# Profile
cargo flamegraph --bin codex -- --help

# Compare with upstream
git fetch upstream
git diff main upstream/main
```

## Syncing

```bash
# Weekly sync
git fetch upstream
git checkout main
git merge upstream/main
git push origin main

# Sync specific branch
git fetch upstream
git checkout helios-optimization
git rebase upstream/main
```
