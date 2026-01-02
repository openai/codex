# CORA Integration for Codex

## What This Is

CORA (Cognitive Orchestration Runtime Architecture) integrated into OpenAI Codex fork as MCP server.

## Structure
```
cora-mcp/               # MCP server implementation
  src/index.ts          # Server entry point
  bin/mcp-server.js     # Compiled binary
```

## Setup
```bash
# 1. Build CORA MCP
cd cora-mcp
npm install
npm run build

# 2. Configure Codex
mkdir -p ~/.codex
cat >> ~/.codex/config.toml << 'END'

[[mcp.servers]]
name = "cora"
command = "node"
args = ["/full/path/to/cora-mcp/bin/mcp-server.js"]
END

# 3. Build Codex
cd codex-rs
cargo build --release
```

## Usage
```bash
# Launch Codex
./codex-rs/target/release/codex

# Use CORA tool
> Use cora_learn tool with module "0.1"
```

## Status

- ✅ MCP server created
- ⏳ Codex Rust build needed
- ⏳ CORA Python script integration
- ⏳ End-to-end testing

## Next Steps

1. Build Codex binary
2. Copy CORA script to project root
3. Test MCP integration
4. Wire CRT professor UI
