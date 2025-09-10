# Build

## Prerequisites
- Node.js 22+
- pnpm 9+
- Rust toolchain (rustup, cargo)

## Steps
1. Clone the repository
   ```bash
   git clone https://github.com/openai/codex.git
   cd codex
   ```
2. Install JavaScript dependencies
   ```bash
   pnpm install
   ```
3. Build Rust workspace
   ```bash
   cd codex-rs
   cargo build
   ```
4. Run tests
   ```bash
   cargo test
   ```
