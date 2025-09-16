# Codex Build Integration

This document describes the implementation of the Codex integration build system that enables both @CODEX and @NOVA components to operate simultaneously in full resonance mode.

## Overview

The implementation includes:

1. **Turbo-based Monorepo Build System** - Enables parallel building of all packages
2. **Full Resonance Mode Configuration** - Environment settings for optimal Codex integration  
3. **Automated Launch System** - Single script to bootstrap and build everything
4. **Local and Cloud Codex Integration** - Scripts for both development modes

## Key Components

### 1. Turbo Configuration (`turbo.json`)

Defines build tasks for all workspace packages with dependency management:
- `build` - Builds all packages with proper dependency ordering
- `test` - Runs tests after builds complete
- `lint` - Code quality checks
- `codex:local` - Local Codex bootstrap
- `codex:cloud` - Cloud Codex setup
- `codex:build` - Combined Codex + build workflow

### 2. Environment Configuration (`.env`)

Sets up full resonance mode with optimal settings:
- `CODEX_RESONANCE=full` - Enables simultaneous @CODEX and @NOVA operation
- Local development configuration
- Performance tuning parameters
- Component enablement flags

### 3. Bootstrap Scripts

**scripts/codex_local.sh**
- Sets up local Codex development environment
- Builds Rust components in development mode
- Prepares Node.js components
- Validates toolchain requirements

**scripts/codex_cloud_setup.sh**  
- Configures cloud-based Codex integration
- Tests connectivity to cloud endpoints
- Validates API credentials
- Sets up cloud-specific features

### 4. Launch System (`launch_everything.sh`)

Comprehensive orchestration script that:
- Validates environment and toolchain
- Runs appropriate Codex bootstrap (local/cloud)
- Executes parallel builds via Turbo
- Provides build status and next steps

## Usage

### Quick Start
```bash
# Run everything in parallel with full resonance
pnpm turbo run build

# Or use the comprehensive launch script
./launch_everything.sh
```

### Individual Commands
```bash
# Local development setup
./scripts/codex_local.sh

# Cloud integration setup  
./scripts/codex_cloud_setup.sh

# Run specific build tasks
pnpm turbo run test
pnpm turbo run lint
```

### Rust CLI Access
```bash
# From codex-rs directory
just codex              # Run CLI
just tui               # Run TUI
just exec              # Run exec command
```

## Configuration Options

Environment variables in `.env`:

- `CODEX_MODE` - Operation mode (local/cloud)
- `CODEX_RESONANCE` - Resonance level (full for simultaneous operation)
- `CODEX_BUILD_PARALLEL` - Enable parallel builds
- `CODEX_DEBUG` - Debug mode for development
- `CODEX_LOCAL_PORT` - Local development port
- Performance and memory settings

## Architecture

The system enables:
1. **Simultaneous Operation** - Both @CODEX and @NOVA can run concurrently
2. **Full Resonance Mode** - Optimal integration between components
3. **Parallel Builds** - Fast development iteration
4. **Environment Flexibility** - Support for local and cloud configurations
5. **Robust Error Handling** - Graceful degradation when optional dependencies fail

## Verification

The implementation has been tested to:
- ✅ Build all packages successfully via Turbo
- ✅ Support parallel execution
- ✅ Handle network failures gracefully
- ✅ Provide functional Rust CLI
- ✅ Cache builds efficiently
- ✅ Bootstrap local development environment