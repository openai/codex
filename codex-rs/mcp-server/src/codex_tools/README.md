# Codex Tools Module

This directory contains Codex-specific MCP tool definitions for sub-agent delegation.

## Overview

Codex Tools provide a set of MCP (Model Context Protocol) tools that allow sub-agents to interact with Codex capabilities safely and efficiently.

## Tool Categories

### 1. Safe (Read-Only) Tools

These tools are safe to use without requiring additional permissions:

- **`read_file`** - Read files from the workspace
- **`grep`** - Search for patterns in files using regex
- **`codebase_search`** - Semantic code search using AI

### 2. Write Tools

These tools require write permissions:

- **`apply_patch`** - Apply unified diff patches to files

### 3. Shell Tools

These tools require shell execution permissions:

- **`shell`** - Execute shell commands (restricted, requires approval)

## Module Structure

```
codex_tools/
├── mod.rs              # Main module definition
├── read_file.rs        # Read file tool
├── grep.rs             # Grep search tool
├── codebase_search.rs  # Semantic search tool
├── apply_patch.rs      # Patch application tool
├── shell.rs            # Shell command tool
└── README.md           # This file
```

## Usage

### Getting Safe Tools

```rust
use codex_mcp_server::codex_tools::CodexMcpTool;

let safe_tools = CodexMcpTool::safe_tools();
// Returns: [read_file, grep, codebase_search]
```

### Getting All Tools

```rust
let all_tools = CodexMcpTool::all_tools();
// Returns: [read_file, grep, codebase_search, apply_patch, shell]
```

## Tool Schema

Each tool follows the MCP tool schema:

```rust
pub struct CodexMcpTool {
    pub name: String,           // Tool identifier (e.g., "codex_read_file")
    pub description: String,    // Human-readable description
    pub input_schema: Value,    // JSON Schema for input validation
}
```

## Security Model

- **Read-Only Tools**: No approval required, safe by default
- **Write Tools**: Require sandbox mode `workspace-write` or higher
- **Shell Tools**: Require approval policy `on-request` or stricter

## Adding New Tools

To add a new tool:

1. Create a new file in this directory (e.g., `new_tool.rs`)
2. Implement the tool method on `CodexMcpTool`
3. Add the module declaration to `mod.rs`
4. Add the tool to either `safe_tools()` or `all_tools()`

Example:

```rust
// new_tool.rs
use super::CodexMcpTool;

impl CodexMcpTool {
    pub fn new_tool() -> Self {
        Self {
            name: "codex_new_tool".to_string(),
            description: "Description of the new tool".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "param": {
                        "type": "string",
                        "description": "Parameter description"
                    }
                },
                "required": ["param"]
            }),
        }
    }
}
```

## Testing

Run unit tests:

```bash
cd codex-rs/mcp-server
cargo test codex_tools
```

## Version

- **Version**: 0.48.0
- **Last Updated**: 2025-10-15

