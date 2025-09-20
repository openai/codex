#!/bin/bash
# Launch Everything Script
# Comprehensive build and launch system for Codex with full resonance mode

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Print colored output
print_status() {
    echo -e "${BLUE}[$(date +'%H:%M:%S')]${NC} $1"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_info() {
    echo -e "${CYAN}ℹ️  $1${NC}"
}

# Script header
echo -e "${PURPLE}"
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║                    🚀 LAUNCH EVERYTHING                      ║"
echo "║              Codex Full Resonance Build System              ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo -e "${NC}"

# Check if we're in the right directory
if [[ ! -f "package.json" ]] || [[ ! -d "codex-rs" ]]; then
    print_error "Please run this script from the root of the codex repository"
    exit 1
fi

# Load environment configuration
if [[ -f ".env" ]]; then
    print_status "Loading environment from .env"
    set -a
    source .env
    set +a
    print_success "Environment loaded"
else
    print_warning ".env file not found, using defaults"
fi

# Set defaults
export CODEX_MODE="${CODEX_MODE:-local}"
export CODEX_RESONANCE="${CODEX_RESONANCE:-full}"
export CODEX_BUILD_PARALLEL="${CODEX_BUILD_PARALLEL:-true}"

print_info "Configuration:"
print_info "  Mode: $CODEX_MODE"
print_info "  Resonance: $CODEX_RESONANCE"
print_info "  Parallel Build: $CODEX_BUILD_PARALLEL"

# Pre-flight checks
print_status "Running pre-flight checks..."

# Check required tools
missing_tools=()
command -v pnpm >/dev/null 2>&1 || missing_tools+=("pnpm")
command -v cargo >/dev/null 2>&1 || missing_tools+=("cargo")
command -v npm >/dev/null 2>&1 || missing_tools+=("npm")

if [[ ${#missing_tools[@]} -gt 0 ]]; then
    print_error "Missing required tools: ${missing_tools[*]}"
    print_info "Please install the missing tools and try again"
    exit 1
fi

print_success "All required tools available"

# Initialize Codex components based on mode
print_status "Initializing Codex components..."

if [[ "$CODEX_MODE" == "local" ]]; then
    print_status "Running local Codex bootstrap..."
    ./scripts/codex_local.sh
elif [[ "$CODEX_MODE" == "cloud" ]]; then
    print_status "Running cloud Codex setup..."
    ./scripts/codex_cloud_setup.sh
else
    print_warning "Unknown CODEX_MODE: $CODEX_MODE, proceeding with default setup"
fi

# Install dependencies
print_status "Installing dependencies..."
if pnpm install --no-frozen-lockfile; then
    print_success "Dependencies installed"
else
    print_warning "Some dependencies failed to install, continuing..."
fi

# Run the comprehensive build
print_status "Starting comprehensive build with Turbo..."

if [[ "$CODEX_BUILD_PARALLEL" == "true" ]]; then
    print_info "Running parallel build with full resonance..."
    if pnpm turbo run build --parallel; then
        print_success "Parallel build completed successfully"
    else
        print_error "Parallel build failed"
        exit 1
    fi
else
    print_info "Running sequential build..."
    if pnpm turbo run build; then
        print_success "Sequential build completed successfully"
    else
        print_error "Sequential build failed"
        exit 1
    fi
fi

# Run tests if available
print_status "Running tests..."
if pnpm turbo run test 2>/dev/null; then
    print_success "Tests completed successfully"
else
    print_warning "Tests not available or failed (continuing)"
fi

# Final verification
print_status "Verifying build artifacts..."

artifacts_ok=true

# Check Node.js build artifacts
if [[ -d "codex-cli/bin" ]] || [[ -d "codex-cli/dist" ]]; then
    print_success "Node.js artifacts found"
else
    print_warning "Node.js artifacts not found"
    artifacts_ok=false
fi

# Check Rust build artifacts
if [[ -d "codex-rs/target" ]]; then
    print_success "Rust artifacts found"
else
    print_warning "Rust artifacts not found"
    artifacts_ok=false
fi

# Summary
echo
echo -e "${PURPLE}╔══════════════════════════════════════════════════════════════╗${NC}"
if [[ "$artifacts_ok" == "true" ]]; then
    echo -e "${PURPLE}║${GREEN}                    🎉 BUILD SUCCESSFUL! 🎉                   ${PURPLE}║${NC}"
    echo -e "${PURPLE}║${NC}                                                              ${PURPLE}║${NC}"
    echo -e "${PURPLE}║${NC} Both @CODEX and @NOVA components are now online and ready   ${PURPLE}║${NC}"
    echo -e "${PURPLE}║${NC} for full resonance mode operation.                          ${PURPLE}║${NC}"
else
    echo -e "${PURPLE}║${YELLOW}                   ⚠️  BUILD COMPLETED ⚠️                    ${PURPLE}║${NC}"
    echo -e "${PURPLE}║${NC}                                                              ${PURPLE}║${NC}"
    echo -e "${PURPLE}║${NC} Build completed but some artifacts may be missing.          ${PURPLE}║${NC}"
    echo -e "${PURPLE}║${NC} Check the logs above for details.                           ${PURPLE}║${NC}"
fi
echo -e "${PURPLE}╚══════════════════════════════════════════════════════════════╝${NC}"

echo
print_info "Available commands:"
print_info "  pnpm turbo run build    - Rebuild all packages"
print_info "  pnpm turbo run test     - Run all tests"
print_info "  pnpm turbo run lint     - Lint all code"
print_info "  just codex              - Run Rust CLI (from codex-rs/)"
print_info "  just tui                - Run Rust TUI (from codex-rs/)"

echo
print_success "Launch Everything completed! 🚀"