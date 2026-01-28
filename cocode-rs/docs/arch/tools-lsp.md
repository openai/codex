# LSP Tool (Phase 6)

> Architecture reference: [tools.md](./tools.md)
>
> **Status:** Planned for Phase 6 implementation

This document describes the LSP (Language Server Protocol) tool, which provides code intelligence capabilities through LSP server integration.

---

## Overview

The LSP tool enables the agent to perform code-aware operations like go-to-definition, hover information, and find-references by communicating with language servers.

## LSP Tool Definition

```rust
pub struct LSPTool {
    manager: Arc<LspServerManager>,
}

#[async_trait]
impl Tool for LSPTool {
    fn name(&self) -> &str { "LSP" }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition::full(
            "LSP",
            "Perform LSP operations: go-to-definition, hover, find-references.",
            json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["goto_definition", "hover", "find_references"],
                        "description": "LSP operation to perform"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file"
                    },
                    "line": {
                        "type": "integer",
                        "description": "Line number (1-indexed)"
                    },
                    "column": {
                        "type": "integer",
                        "description": "Column number (1-indexed)"
                    }
                },
                "required": ["operation", "file_path", "line", "column"]
            })
        )
    }

    fn is_read_only(&self) -> bool { true }

    fn is_concurrency_safe(&self, _input: &Value) -> bool { true }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolContext,
        _tool_use_id: &str,
        _metadata: ToolMetadata,
        _progress: Option<ProgressCallback>,
    ) -> Result<ToolOutput, ToolError> {
        let args: LspArgs = serde_json::from_value(input)?;

        let server = self.manager.get_server_for_file(&args.file_path).await?;

        match args.operation.as_str() {
            "goto_definition" => {
                let locations = server.goto_definition(
                    &args.file_path,
                    args.line,
                    args.column,
                ).await?;
                Ok(ToolOutput::success(format_locations(&locations)))
            }
            "hover" => {
                let hover = server.hover(
                    &args.file_path,
                    args.line,
                    args.column,
                ).await?;
                Ok(ToolOutput::success(hover.contents))
            }
            "find_references" => {
                let refs = server.find_references(
                    &args.file_path,
                    args.line,
                    args.column,
                ).await?;
                Ok(ToolOutput::success(format_references(&refs)))
            }
            _ => Ok(ToolOutput::error(format!("Unknown operation: {}", args.operation)))
        }
    }
}
```

## LspServerManager

The `LspServerManager` handles language server lifecycle and routing requests to the appropriate server based on file type.

```rust
/// LSP server manager
pub struct LspServerManager {
    servers: HashMap<String, Arc<LspServer>>,
    config: LspConfig,
}

impl LspServerManager {
    /// Get or start LSP server for a file
    pub async fn get_server_for_file(&self, path: &str) -> Result<Arc<LspServer>, LspError> {
        let language = detect_language(path)?;
        if let Some(server) = self.servers.get(&language) {
            return Ok(server.clone());
        }
        self.start_server(&language).await
    }
}
```

## Tool Properties

| Property | Value | Description |
|----------|-------|-------------|
| Read-Only | Yes | Does not modify files |
| Concurrency | Safe | Can run in parallel with other tools |
| Max Result Size | N/A | No specific limit |

## Supported Operations

| Operation | Description | Output |
|-----------|-------------|--------|
| `goto_definition` | Navigate to symbol definition | List of file:line:column locations |
| `hover` | Get type/documentation info | Markdown content |
| `find_references` | Find all symbol usages | List of file:line:column references |

## Integration with Edit Tool

The LSP tool integrates with the Edit tool for real-time feedback. After file edits, the Edit tool notifies the LSP server about changes:

```rust
// Post-edit LSP notification (if LSP server is connected)
if let Some(lsp) = ctx.lsp_client() {
    lsp.notify_did_change(&path, &new_content).await;
}
```

This enables:
- Immediate diagnostics refresh after edits
- Symbol updates without requiring file save
- Real-time error detection

## Registration

The LSP tool is registered optionally when an LSP server manager is available:

```rust
// In register_all_tools
// LSP (optional, if server manager available)
// registry.register(LSPTool::new(lsp_manager));
```

## Phase 6 Implementation Notes

For Phase 6 implementation, consider:

1. **Language Server Discovery**: Auto-detect installed language servers
2. **Server Lifecycle**: Handle server startup, shutdown, and crashes gracefully
3. **File Synchronization**: Keep LSP servers in sync with file changes
4. **Workspace Configuration**: Support workspace-specific LSP settings
5. **Diagnostics Integration**: Surface LSP diagnostics to the agent

See the main [tools.md](./tools.md) architecture document for how this tool fits into the overall tool system.
