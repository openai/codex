//! LSP tool for language server protocol operations.
//!
//! Provides IDE-like features through LSP: go to definition, find references,
//! hover documentation, document symbols, workspace symbols, and more.

use super::prompts;
use crate::context::ToolContext;
use crate::error::Result;
use crate::error::tool_error;
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_lsp::SymbolKind;
use cocode_protocol::ConcurrencySafety;
use cocode_protocol::ToolOutput;
use serde_json::Value;

/// Tool for LSP operations.
///
/// This is a read-only, concurrency-safe tool that provides language
/// intelligence features through Language Server Protocol.
pub struct LspTool;

impl LspTool {
    /// Create a new LSP tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for LspTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for LspTool {
    fn name(&self) -> &str {
        "Lsp"
    }

    fn description(&self) -> &str {
        prompts::LSP_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "description": "The LSP operation to perform",
                    "enum": [
                        "goToDefinition",
                        "findReferences",
                        "hover",
                        "documentSymbol",
                        "workspaceSymbol",
                        "goToImplementation",
                        "goToTypeDefinition",
                        "goToDeclaration",
                        "getCallHierarchy",
                        "getDiagnostics"
                    ]
                },
                "filePath": {
                    "type": "string",
                    "description": "The absolute path to the file"
                },
                "symbolName": {
                    "type": "string",
                    "description": "The name of the symbol to query (AI-friendly)"
                },
                "symbolKind": {
                    "type": "string",
                    "description": "The kind of symbol (e.g., 'function', 'struct', 'trait')"
                },
                "line": {
                    "type": "integer",
                    "description": "0-indexed line number for position-based queries"
                },
                "character": {
                    "type": "integer",
                    "description": "0-indexed character offset for position-based queries"
                },
                "query": {
                    "type": "string",
                    "description": "Search query for workspaceSymbol operation"
                },
                "includeDeclaration": {
                    "type": "boolean",
                    "description": "Include declaration in references (default: true)",
                    "default": true
                },
                "direction": {
                    "type": "string",
                    "description": "Direction for call hierarchy: 'incoming' or 'outgoing'",
                    "enum": ["incoming", "outgoing"]
                }
            },
            "required": ["operation"]
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::Safe
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn feature_gate(&self) -> Option<cocode_protocol::Feature> {
        Some(cocode_protocol::Feature::Lsp)
    }

    async fn execute(&self, input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        let manager = ctx.lsp_manager.as_ref().ok_or_else(|| {
            tool_error::ExecutionFailedSnafu {
                message: "LSP feature not enabled. No LSP server manager available.",
            }
            .build()
        })?;

        let operation = input["operation"].as_str().ok_or_else(|| {
            tool_error::InvalidInputSnafu {
                message: "operation must be a string",
            }
            .build()
        })?;

        // Most operations require a file path
        let file_path = input["filePath"].as_str();

        // Parse symbol name and kind for symbol-based queries
        let symbol_name = input["symbolName"].as_str();
        let symbol_kind = input["symbolKind"]
            .as_str()
            .and_then(SymbolKind::from_str_loose);

        // Parse position for position-based queries
        let line = input["line"].as_i64().map(|n| n as u32);
        let character = input["character"].as_i64().map(|n| n as u32);

        let result = match operation {
            "goToDefinition" => {
                let path = require_file_path(file_path)?;
                let path = ctx.resolve_path(path);
                let client = manager
                    .get_client(&path)
                    .await
                    .map_err(lsp_err_to_tool_err)?;

                let locations = if let Some(symbol) = symbol_name {
                    client
                        .definition(&path, symbol, symbol_kind)
                        .await
                        .map_err(lsp_err_to_tool_err)?
                } else if let (Some(l), Some(c)) = (line, character) {
                    let position = cocode_lsp::lsp_types_reexport::Position::new(l, c);
                    client
                        .definition_at_position(&path, position)
                        .await
                        .map_err(lsp_err_to_tool_err)?
                } else {
                    return Err(tool_error::InvalidInputSnafu {
                        message: "goToDefinition requires symbolName or line+character",
                    }
                    .build());
                };

                format_locations(&locations)
            }

            "findReferences" => {
                let path = require_file_path(file_path)?;
                let path = ctx.resolve_path(path);
                let client = manager
                    .get_client(&path)
                    .await
                    .map_err(lsp_err_to_tool_err)?;
                let include_declaration = input["includeDeclaration"].as_bool().unwrap_or(true);

                let locations = if let Some(symbol) = symbol_name {
                    client
                        .references(&path, symbol, symbol_kind, include_declaration)
                        .await
                        .map_err(lsp_err_to_tool_err)?
                } else if let (Some(l), Some(c)) = (line, character) {
                    let position = cocode_lsp::lsp_types_reexport::Position::new(l, c);
                    client
                        .references_at_position(&path, position, include_declaration)
                        .await
                        .map_err(lsp_err_to_tool_err)?
                } else {
                    return Err(tool_error::InvalidInputSnafu {
                        message: "findReferences requires symbolName or line+character",
                    }
                    .build());
                };

                format_locations(&locations)
            }

            "hover" => {
                let path = require_file_path(file_path)?;
                let path = ctx.resolve_path(path);
                let client = manager
                    .get_client(&path)
                    .await
                    .map_err(lsp_err_to_tool_err)?;

                let hover_result = if let Some(symbol) = symbol_name {
                    client
                        .hover(&path, symbol, symbol_kind)
                        .await
                        .map_err(lsp_err_to_tool_err)?
                } else if let (Some(l), Some(c)) = (line, character) {
                    let position = cocode_lsp::lsp_types_reexport::Position::new(l, c);
                    client
                        .hover_at_position(&path, position)
                        .await
                        .map_err(lsp_err_to_tool_err)?
                } else {
                    return Err(tool_error::InvalidInputSnafu {
                        message: "hover requires symbolName or line+character",
                    }
                    .build());
                };

                hover_result.unwrap_or_else(|| "No hover information available".to_string())
            }

            "documentSymbol" => {
                let path = require_file_path(file_path)?;
                let path = ctx.resolve_path(path);
                let client = manager
                    .get_client(&path)
                    .await
                    .map_err(lsp_err_to_tool_err)?;

                let symbols = client
                    .document_symbols(&path)
                    .await
                    .map_err(lsp_err_to_tool_err)?;

                format_document_symbols(&symbols)
            }

            "workspaceSymbol" => {
                let query = input["query"].as_str().unwrap_or("");
                // For workspace symbol, we need any file to get a client
                let path = file_path
                    .map(|p| ctx.resolve_path(p))
                    .unwrap_or(ctx.cwd.clone());
                let client = manager
                    .get_client(&path)
                    .await
                    .map_err(lsp_err_to_tool_err)?;

                let symbols = client
                    .workspace_symbol(query)
                    .await
                    .map_err(lsp_err_to_tool_err)?;

                format_workspace_symbols(&symbols)
            }

            "goToImplementation" => {
                let path = require_file_path(file_path)?;
                let path = ctx.resolve_path(path);
                let client = manager
                    .get_client(&path)
                    .await
                    .map_err(lsp_err_to_tool_err)?;

                let locations = if let Some(symbol) = symbol_name {
                    client
                        .implementation(&path, symbol, symbol_kind)
                        .await
                        .map_err(lsp_err_to_tool_err)?
                } else if let (Some(l), Some(c)) = (line, character) {
                    let position = cocode_lsp::lsp_types_reexport::Position::new(l, c);
                    client
                        .implementation_at_position(&path, position)
                        .await
                        .map_err(lsp_err_to_tool_err)?
                } else {
                    return Err(tool_error::InvalidInputSnafu {
                        message: "goToImplementation requires symbolName or line+character",
                    }
                    .build());
                };

                format_locations(&locations)
            }

            "goToTypeDefinition" => {
                let path = require_file_path(file_path)?;
                let path = ctx.resolve_path(path);
                let client = manager
                    .get_client(&path)
                    .await
                    .map_err(lsp_err_to_tool_err)?;

                let locations = if let Some(symbol) = symbol_name {
                    client
                        .type_definition(&path, symbol, symbol_kind)
                        .await
                        .map_err(lsp_err_to_tool_err)?
                } else if let (Some(l), Some(c)) = (line, character) {
                    let position = cocode_lsp::lsp_types_reexport::Position::new(l, c);
                    client
                        .type_definition_at_position(&path, position)
                        .await
                        .map_err(lsp_err_to_tool_err)?
                } else {
                    return Err(tool_error::InvalidInputSnafu {
                        message: "goToTypeDefinition requires symbolName or line+character",
                    }
                    .build());
                };

                format_locations(&locations)
            }

            "goToDeclaration" => {
                let path = require_file_path(file_path)?;
                let path = ctx.resolve_path(path);
                let client = manager
                    .get_client(&path)
                    .await
                    .map_err(lsp_err_to_tool_err)?;

                let locations = if let Some(symbol) = symbol_name {
                    client
                        .declaration(&path, symbol, symbol_kind)
                        .await
                        .map_err(lsp_err_to_tool_err)?
                } else if let (Some(l), Some(c)) = (line, character) {
                    let position = cocode_lsp::lsp_types_reexport::Position::new(l, c);
                    client
                        .declaration_at_position(&path, position)
                        .await
                        .map_err(lsp_err_to_tool_err)?
                } else {
                    return Err(tool_error::InvalidInputSnafu {
                        message: "goToDeclaration requires symbolName or line+character",
                    }
                    .build());
                };

                format_locations(&locations)
            }

            "getCallHierarchy" => {
                let path = require_file_path(file_path)?;
                let path = ctx.resolve_path(path);
                let client = manager
                    .get_client(&path)
                    .await
                    .map_err(lsp_err_to_tool_err)?;
                let direction = input["direction"].as_str().unwrap_or("incoming");

                let items = if let Some(symbol) = symbol_name {
                    client
                        .prepare_call_hierarchy(&path, symbol, symbol_kind)
                        .await
                        .map_err(lsp_err_to_tool_err)?
                } else if let (Some(l), Some(c)) = (line, character) {
                    let position = cocode_lsp::lsp_types_reexport::Position::new(l, c);
                    client
                        .prepare_call_hierarchy_at_position(&path, position)
                        .await
                        .map_err(lsp_err_to_tool_err)?
                } else {
                    return Err(tool_error::InvalidInputSnafu {
                        message: "getCallHierarchy requires symbolName or line+character",
                    }
                    .build());
                };

                if items.is_empty() {
                    "No call hierarchy available for this symbol".to_string()
                } else {
                    let item = items.into_iter().next().unwrap();
                    let item_name = item.name.clone();

                    match direction {
                        "incoming" => {
                            let calls = client
                                .incoming_calls(item)
                                .await
                                .map_err(lsp_err_to_tool_err)?;
                            format_incoming_calls(&item_name, &calls)
                        }
                        "outgoing" => {
                            let calls = client
                                .outgoing_calls(item)
                                .await
                                .map_err(lsp_err_to_tool_err)?;
                            format_outgoing_calls(&item_name, &calls)
                        }
                        _ => {
                            return Err(tool_error::InvalidInputSnafu {
                                message: "direction must be 'incoming' or 'outgoing'",
                            }
                            .build());
                        }
                    }
                }
            }

            "getDiagnostics" => {
                let path = require_file_path(file_path)?;
                let path = ctx.resolve_path(path);

                // Get diagnostics from the manager's diagnostics store
                let diagnostics = manager.diagnostics();
                let file_diagnostics = diagnostics.get_file(&path).await;

                if file_diagnostics.is_empty() {
                    "No diagnostics for this file".to_string()
                } else {
                    format_diagnostics(&file_diagnostics)
                }
            }

            _ => {
                return Err(tool_error::InvalidInputSnafu {
                    message: format!("Unknown operation: {operation}"),
                }
                .build());
            }
        };

        Ok(ToolOutput::text(result))
    }
}

fn require_file_path(file_path: Option<&str>) -> Result<&str> {
    file_path.ok_or_else(|| {
        tool_error::InvalidInputSnafu {
            message: "filePath is required for this operation",
        }
        .build()
    })
}

fn lsp_err_to_tool_err(err: cocode_lsp::LspErr) -> crate::error::ToolError {
    tool_error::ExecutionFailedSnafu {
        message: err.to_string(),
    }
    .build()
}

fn format_locations(locations: &[cocode_lsp::Location]) -> String {
    if locations.is_empty() {
        return "No results found".to_string();
    }

    let mut output = String::new();
    output.push_str(&format!("Found {} location(s):\n\n", locations.len()));

    for (i, loc) in locations.iter().enumerate() {
        let path = url_to_path(&loc.uri);
        let line = loc.range.start.line + 1; // Convert to 1-indexed
        let col = loc.range.start.character + 1;
        output.push_str(&format!("{}. {}:{}:{}\n", i + 1, path, line, col));
    }

    output
}

fn format_document_symbols(symbols: &[cocode_lsp::symbols::ResolvedSymbol]) -> String {
    if symbols.is_empty() {
        return "No symbols found in this file".to_string();
    }

    let mut output = String::new();
    output.push_str(&format!("Found {} symbol(s):\n\n", symbols.len()));

    for sym in symbols {
        let kind_name = sym.kind.display_name();
        let line = sym.position.line + 1; // Convert to 1-indexed
        output.push_str(&format!("- {} {} (line {})\n", kind_name, sym.name, line));
    }

    output
}

fn format_workspace_symbols(symbols: &[cocode_lsp::SymbolInformation]) -> String {
    if symbols.is_empty() {
        return "No symbols found matching query".to_string();
    }

    let mut output = String::new();
    output.push_str(&format!("Found {} symbol(s):\n\n", symbols.len()));

    for sym in symbols {
        let path = url_to_path(&sym.location.uri);
        let line = sym.location.range.start.line + 1; // Convert to 1-indexed
        let kind_str = format!("{:?}", sym.kind).to_lowercase();
        output.push_str(&format!(
            "- {} {} ({}:{})\n",
            kind_str, sym.name, path, line
        ));
    }

    output
}

fn format_incoming_calls(target: &str, calls: &[cocode_lsp::CallHierarchyIncomingCall]) -> String {
    if calls.is_empty() {
        return format!("No incoming calls to '{target}'");
    }

    let mut output = String::new();
    output.push_str(&format!(
        "Incoming calls to '{}' ({} caller(s)):\n\n",
        target,
        calls.len()
    ));

    for call in calls {
        let path = url_to_path(&call.from.uri);
        let line = call.from.selection_range.start.line + 1;
        output.push_str(&format!("- {} ({}:{})\n", call.from.name, path, line));
    }

    output
}

fn format_outgoing_calls(source: &str, calls: &[cocode_lsp::CallHierarchyOutgoingCall]) -> String {
    if calls.is_empty() {
        return format!("No outgoing calls from '{source}'");
    }

    let mut output = String::new();
    output.push_str(&format!(
        "Outgoing calls from '{}' ({} callee(s)):\n\n",
        source,
        calls.len()
    ));

    for call in calls {
        let path = url_to_path(&call.to.uri);
        let line = call.to.selection_range.start.line + 1;
        output.push_str(&format!("- {} ({}:{})\n", call.to.name, path, line));
    }

    output
}

fn format_diagnostics(diagnostics: &[cocode_lsp::DiagnosticEntry]) -> String {
    let mut output = String::new();
    output.push_str(&format!("Found {} diagnostic(s):\n\n", diagnostics.len()));

    for diag in diagnostics {
        let severity = match diag.severity {
            cocode_lsp::DiagnosticSeverityLevel::Error => "ERROR",
            cocode_lsp::DiagnosticSeverityLevel::Warning => "WARN",
            cocode_lsp::DiagnosticSeverityLevel::Info => "INFO",
            cocode_lsp::DiagnosticSeverityLevel::Hint => "HINT",
        };
        output.push_str(&format!(
            "[{}] Line {}: {}\n",
            severity, diag.line, diag.message
        ));
    }

    output
}

fn url_to_path(url: &cocode_lsp::lsp_types_reexport::Url) -> String {
    url.to_file_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_context() -> ToolContext {
        ToolContext::new("call-1", "session-1", PathBuf::from("/tmp"))
    }

    #[test]
    fn test_tool_properties() {
        let tool = LspTool::new();
        assert_eq!(tool.name(), "Lsp");
        assert!(tool.is_concurrent_safe());
        assert!(tool.is_read_only());
    }

    #[test]
    fn test_feature_gate() {
        let tool = LspTool::new();
        assert_eq!(tool.feature_gate(), Some(cocode_protocol::Feature::Lsp));
    }

    #[tokio::test]
    async fn test_execute_without_manager() {
        let tool = LspTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "operation": "goToDefinition",
            "filePath": "/test/file.rs",
            "symbolName": "Config"
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("LSP feature not enabled"));
    }

    #[tokio::test]
    async fn test_validation_missing_operation() {
        let tool = LspTool::new();

        let input = serde_json::json!({
            "filePath": "/test/file.rs"
        });

        let result = tool.validate(&input).await;
        assert!(matches!(
            result,
            cocode_protocol::ValidationResult::Invalid { .. }
        ));
    }

    #[test]
    fn test_format_locations_empty() {
        let result = format_locations(&[]);
        assert_eq!(result, "No results found");
    }

    #[test]
    fn test_format_document_symbols_empty() {
        let result = format_document_symbols(&[]);
        assert_eq!(result, "No symbols found in this file");
    }
}
