//! LSP Handler - Code intelligence using Language Server Protocol
//!
//! Provides AI-friendly LSP operations using symbol name + kind matching.
//! Supports Rust (rust-analyzer), Go (gopls), and Python (pyright).

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;
use codex_lsp::CallHierarchyIncomingCall;
use codex_lsp::CallHierarchyOutgoingCall;
use codex_lsp::DiagnosticsStore;
use codex_lsp::Location;
use codex_lsp::LspServerManager;
use codex_lsp::LspServersConfig;
use codex_lsp::SymbolInformation;
use codex_lsp::SymbolKind;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::OnceCell;

/// Global LSP manager (lazily initialized)
static LSP_MANAGER: OnceCell<Arc<LspServerManager>> = OnceCell::const_new();

/// Get or create the global LSP manager
async fn get_lsp_manager() -> Arc<LspServerManager> {
    LSP_MANAGER
        .get_or_init(|| async {
            let config = LspServersConfig::default();
            let diagnostics = Arc::new(DiagnosticsStore::new());
            Arc::new(LspServerManager::new(config, diagnostics))
        })
        .await
        .clone()
}

/// Get the global LSP diagnostics store if LSP has been initialized.
///
/// Returns None if the LSP tool has never been invoked in this session.
/// This is used by the system reminder injection to access diagnostics
/// collected from LSP servers.
pub fn get_lsp_diagnostics_store() -> Option<Arc<DiagnosticsStore>> {
    LSP_MANAGER.get().map(|m| m.diagnostics().clone())
}

/// LSP tool arguments
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LspArgs {
    operation: String,
    file_path: String,
    #[serde(default)]
    symbol_name: Option<String>,
    #[serde(default)]
    symbol_kind: Option<String>,
    #[serde(default)]
    direction: Option<String>,
}

/// LSP Handler
pub struct LspHandler;

impl LspHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LspHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolHandler for LspHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        // 1. Parse arguments
        let arguments = match &invocation.payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "Invalid payload type for lsp".to_string(),
                ));
            }
        };

        let args: LspArgs = serde_json::from_str(arguments)
            .map_err(|e| FunctionCallError::RespondToModel(format!("Invalid arguments: {e}")))?;

        // 2. Resolve file path
        let file_path = invocation.turn.resolve_path(Some(args.file_path.clone()));

        // For workspaceSymbol, file doesn't need to exist - it's just used to determine which server to use
        let is_workspace_symbol = args.operation == "workspaceSymbol";
        if !is_workspace_symbol && !file_path.exists() {
            return Err(FunctionCallError::RespondToModel(format!(
                "File not found: {}",
                file_path.display()
            )));
        }

        // 3. Get LSP manager and client
        let manager = get_lsp_manager().await;

        // Check if LSP is available for this file type
        if !manager.is_available(&file_path).await {
            let ext = file_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("unknown");
            return Err(FunctionCallError::RespondToModel(format!(
                "No LSP server available for .{} files. Supported: .rs (Rust), .go (Go), .py (Python)",
                ext
            )));
        }

        let client = manager
            .get_client(&file_path)
            .await
            .map_err(|e| FunctionCallError::RespondToModel(e.to_string()))?;

        // 4. Parse symbol kind if provided
        let symbol_kind = args
            .symbol_kind
            .as_ref()
            .and_then(|k| SymbolKind::from_str_loose(k));

        // 5. Execute operation
        let result = match args.operation.as_str() {
            "goToDefinition" => {
                let symbol_name = args.symbol_name.as_ref().ok_or_else(|| {
                    FunctionCallError::RespondToModel(
                        "symbolName is required for goToDefinition".to_string(),
                    )
                })?;

                let locations = client
                    .definition(&file_path, symbol_name, symbol_kind)
                    .await
                    .map_err(|e| FunctionCallError::RespondToModel(e.to_string()))?;

                format_definition_result(symbol_name, &locations)
            }

            "findReferences" => {
                let symbol_name = args.symbol_name.as_ref().ok_or_else(|| {
                    FunctionCallError::RespondToModel(
                        "symbolName is required for findReferences".to_string(),
                    )
                })?;

                let locations = client
                    .references(&file_path, symbol_name, symbol_kind, true)
                    .await
                    .map_err(|e| FunctionCallError::RespondToModel(e.to_string()))?;

                format_references_result(symbol_name, &locations)
            }

            "hover" => {
                let symbol_name = args.symbol_name.as_ref().ok_or_else(|| {
                    FunctionCallError::RespondToModel(
                        "symbolName is required for hover".to_string(),
                    )
                })?;

                let hover_info = client
                    .hover(&file_path, symbol_name, symbol_kind)
                    .await
                    .map_err(|e| FunctionCallError::RespondToModel(e.to_string()))?;

                match hover_info {
                    Some(info) => format!("Hover info for '{symbol_name}':\n\n{info}"),
                    None => format!("No hover information available for '{symbol_name}'"),
                }
            }

            "documentSymbol" => {
                let symbols = client
                    .document_symbols(&file_path)
                    .await
                    .map_err(|e| FunctionCallError::RespondToModel(e.to_string()))?;

                if symbols.is_empty() {
                    format!("No symbols found in {}", file_path.display())
                } else {
                    let mut output = format!(
                        "Found {} symbol(s) in {}:\n\n",
                        symbols.len(),
                        file_path.display()
                    );
                    for sym in symbols.iter() {
                        output.push_str(&format!(
                            "  {} {} (line {})\n",
                            sym.kind.display_name(),
                            sym.name,
                            sym.position.line + 1
                        ));
                    }
                    output
                }
            }

            "getDiagnostics" => {
                let diagnostics = manager.diagnostics().get_file(&file_path).await;

                if diagnostics.is_empty() {
                    format!("No diagnostics for {}", file_path.display())
                } else {
                    let mut output = format!(
                        "Found {} diagnostic(s) in {}:\n\n",
                        diagnostics.len(),
                        file_path.display()
                    );
                    for diag in &diagnostics {
                        let code_str = diag
                            .code
                            .as_ref()
                            .map(|c| format!(" [{}]", c))
                            .unwrap_or_default();
                        output.push_str(&format!(
                            "  Line {}: [{}]{} {}\n",
                            diag.line,
                            diag.severity.as_str(),
                            code_str,
                            diag.message
                        ));
                    }
                    output
                }
            }

            "workspaceSymbol" => {
                let query = args.symbol_name.as_ref().ok_or_else(|| {
                    FunctionCallError::RespondToModel(
                        "symbolName is required for workspaceSymbol (as search query)".to_string(),
                    )
                })?;

                let symbols = client
                    .workspace_symbol(query)
                    .await
                    .map_err(|e| FunctionCallError::RespondToModel(e.to_string()))?;

                format_workspace_symbol_result(query, &symbols)
            }

            "goToImplementation" => {
                let symbol_name = args.symbol_name.as_ref().ok_or_else(|| {
                    FunctionCallError::RespondToModel(
                        "symbolName is required for goToImplementation".to_string(),
                    )
                })?;

                let locations = client
                    .implementation(&file_path, symbol_name, symbol_kind)
                    .await
                    .map_err(|e| FunctionCallError::RespondToModel(e.to_string()))?;

                format_implementation_result(symbol_name, &locations)
            }

            "getCallHierarchy" => {
                let symbol_name = args.symbol_name.as_ref().ok_or_else(|| {
                    FunctionCallError::RespondToModel(
                        "symbolName is required for getCallHierarchy".to_string(),
                    )
                })?;
                let direction = args.direction.as_ref().ok_or_else(|| {
                    FunctionCallError::RespondToModel(
                        "direction is required for getCallHierarchy (incoming or outgoing)"
                            .to_string(),
                    )
                })?;

                // Step 1: Prepare call hierarchy
                let items = client
                    .prepare_call_hierarchy(&file_path, symbol_name, symbol_kind)
                    .await
                    .map_err(|e| FunctionCallError::RespondToModel(e.to_string()))?;

                let item = items.into_iter().next().ok_or_else(|| {
                    FunctionCallError::RespondToModel(format!(
                        "No call hierarchy found for '{symbol_name}'"
                    ))
                })?;

                // Step 2: Get calls based on direction
                match direction.as_str() {
                    "incoming" => {
                        let calls = client
                            .incoming_calls(item)
                            .await
                            .map_err(|e| FunctionCallError::RespondToModel(e.to_string()))?;
                        format_incoming_calls_result(symbol_name, &calls)
                    }
                    "outgoing" => {
                        let calls = client
                            .outgoing_calls(item)
                            .await
                            .map_err(|e| FunctionCallError::RespondToModel(e.to_string()))?;
                        format_outgoing_calls_result(symbol_name, &calls)
                    }
                    _ => {
                        return Err(FunctionCallError::RespondToModel(format!(
                            "Invalid direction: {direction}. Use 'incoming' or 'outgoing'"
                        )));
                    }
                }
            }

            "goToTypeDefinition" => {
                let symbol_name = args.symbol_name.as_ref().ok_or_else(|| {
                    FunctionCallError::RespondToModel(
                        "symbolName is required for goToTypeDefinition".to_string(),
                    )
                })?;

                let locations = client
                    .type_definition(&file_path, symbol_name, symbol_kind)
                    .await
                    .map_err(|e| FunctionCallError::RespondToModel(e.to_string()))?;

                format_type_definition_result(symbol_name, &locations)
            }

            "goToDeclaration" => {
                let symbol_name = args.symbol_name.as_ref().ok_or_else(|| {
                    FunctionCallError::RespondToModel(
                        "symbolName is required for goToDeclaration".to_string(),
                    )
                })?;

                let locations = client
                    .declaration(&file_path, symbol_name, symbol_kind)
                    .await
                    .map_err(|e| FunctionCallError::RespondToModel(e.to_string()))?;

                format_declaration_result(symbol_name, &locations)
            }

            other => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "Unknown operation: {}. Valid operations: goToDefinition, findReferences, \
                     hover, documentSymbol, getDiagnostics, workspaceSymbol, goToImplementation, \
                     getCallHierarchy, goToTypeDefinition, goToDeclaration",
                    other
                )));
            }
        };

        Ok(ToolOutput::Function {
            content: result,
            content_items: None,
            success: Some(true),
        })
    }
}

/// Format definition locations for display
fn format_definition_result(symbol_name: &str, locations: &[Location]) -> String {
    if locations.is_empty() {
        return format!("No definition found for '{symbol_name}'");
    }

    let mut output = format!(
        "Found {} definition(s) for '{symbol_name}':\n",
        locations.len()
    );

    for loc in locations {
        let path = PathBuf::from(loc.uri.path());
        let line = loc.range.start.line + 1;
        let col = loc.range.start.character + 1;
        output.push_str(&format!("  {}:{}:{}\n", path.display(), line, col));
    }

    output
}

/// Format reference locations for display
fn format_references_result(symbol_name: &str, locations: &[Location]) -> String {
    if locations.is_empty() {
        return format!("No references found for '{symbol_name}'");
    }

    // Group by file
    let mut by_file: std::collections::HashMap<&str, Vec<&Location>> =
        std::collections::HashMap::new();
    for loc in locations {
        by_file.entry(loc.uri.path()).or_default().push(loc);
    }

    let mut output = format!(
        "Found {} reference(s) for '{}' in {} file(s):\n\n",
        locations.len(),
        symbol_name,
        by_file.len()
    );

    for (path, locs) in by_file {
        output.push_str(&format!("{path}:\n"));
        for loc in locs {
            output.push_str(&format!("  Line {}\n", loc.range.start.line + 1));
        }
        output.push('\n');
    }

    output
}

/// Format workspace symbol search results for display
fn format_workspace_symbol_result(query: &str, symbols: &[SymbolInformation]) -> String {
    if symbols.is_empty() {
        return format!("No symbols found matching '{query}'");
    }

    let mut output = format!(
        "Found {} symbol(s) matching '{}':\n\n",
        symbols.len(),
        query
    );

    for sym in symbols {
        let path = PathBuf::from(sym.location.uri.path());
        let line = sym.location.range.start.line + 1;
        let kind = format!("{:?}", sym.kind);
        let container = sym
            .container_name
            .as_ref()
            .map(|c| format!(" in {}", c))
            .unwrap_or_default();
        output.push_str(&format!(
            "  {} {}{} - {}:{}\n",
            kind,
            sym.name,
            container,
            path.display(),
            line
        ));
    }

    output
}

/// Format implementation locations for display
fn format_implementation_result(symbol_name: &str, locations: &[Location]) -> String {
    if locations.is_empty() {
        return format!("No implementations found for '{symbol_name}'");
    }

    let mut output = format!(
        "Found {} implementation(s) for '{symbol_name}':\n",
        locations.len()
    );

    for loc in locations {
        let path = PathBuf::from(loc.uri.path());
        let line = loc.range.start.line + 1;
        let col = loc.range.start.character + 1;
        output.push_str(&format!("  {}:{}:{}\n", path.display(), line, col));
    }

    output
}

/// Format incoming calls for display
fn format_incoming_calls_result(symbol_name: &str, calls: &[CallHierarchyIncomingCall]) -> String {
    if calls.is_empty() {
        return format!("No incoming calls found for '{symbol_name}'");
    }

    let mut output = format!(
        "Found {} incoming call(s) to '{}':\n\n",
        calls.len(),
        symbol_name
    );

    for call in calls {
        let path = PathBuf::from(call.from.uri.path());
        let line = call.from.selection_range.start.line + 1;
        let kind = format!("{:?}", call.from.kind);
        output.push_str(&format!(
            "  {} {} - {}:{}\n",
            kind,
            call.from.name,
            path.display(),
            line
        ));
    }

    output
}

/// Format outgoing calls for display
fn format_outgoing_calls_result(symbol_name: &str, calls: &[CallHierarchyOutgoingCall]) -> String {
    if calls.is_empty() {
        return format!("No outgoing calls found for '{symbol_name}'");
    }

    let mut output = format!(
        "Found {} outgoing call(s) from '{}':\n\n",
        calls.len(),
        symbol_name
    );

    for call in calls {
        let path = PathBuf::from(call.to.uri.path());
        let line = call.to.selection_range.start.line + 1;
        let kind = format!("{:?}", call.to.kind);
        output.push_str(&format!(
            "  {} {} - {}:{}\n",
            kind,
            call.to.name,
            path.display(),
            line
        ));
    }

    output
}

/// Format type definition locations for display
fn format_type_definition_result(symbol_name: &str, locations: &[Location]) -> String {
    if locations.is_empty() {
        return format!("No type definition found for '{symbol_name}'");
    }

    let mut output = format!(
        "Found {} type definition(s) for '{symbol_name}':\n",
        locations.len()
    );

    for loc in locations {
        let path = PathBuf::from(loc.uri.path());
        let line = loc.range.start.line + 1;
        let col = loc.range.start.character + 1;
        output.push_str(&format!("  {}:{}:{}\n", path.display(), line, col));
    }

    output
}

/// Format declaration locations for display
fn format_declaration_result(symbol_name: &str, locations: &[Location]) -> String {
    if locations.is_empty() {
        return format!("No declaration found for '{symbol_name}'");
    }

    let mut output = format!(
        "Found {} declaration(s) for '{symbol_name}':\n",
        locations.len()
    );

    for loc in locations {
        let path = PathBuf::from(loc.uri.path());
        let line = loc.range.start.line + 1;
        let col = loc.range.start.character + 1;
        output.push_str(&format!("  {}:{}:{}\n", path.display(), line, col));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_lsp::Location;

    fn make_location(path: &str, line: u32) -> Location {
        use codex_lsp::lsp_types_reexport::Position;
        use codex_lsp::lsp_types_reexport::Range;
        use codex_lsp::lsp_types_reexport::Url;
        Location {
            uri: Url::parse(&format!("file://{path}")).unwrap(),
            range: Range {
                start: Position { line, character: 0 },
                end: Position {
                    line,
                    character: 10,
                },
            },
        }
    }

    #[test]
    fn test_format_definition_result_empty() {
        let result = format_definition_result("TestFn", &[]);
        assert_eq!(result, "No definition found for 'TestFn'");
    }

    #[test]
    fn test_format_definition_result_single() {
        let locations = vec![make_location("/src/lib.rs", 41)];
        let result = format_definition_result("TestFn", &locations);

        assert!(result.contains("Found 1 definition(s)"));
        assert!(result.contains("/src/lib.rs:42:1")); // line + 1
    }

    #[test]
    fn test_format_definition_result_multiple() {
        let locations = vec![
            make_location("/src/lib.rs", 41),
            make_location("/src/main.rs", 99),
        ];
        let result = format_definition_result("Config", &locations);

        assert!(result.contains("Found 2 definition(s)"));
        assert!(result.contains("/src/lib.rs:42:1"));
        assert!(result.contains("/src/main.rs:100:1"));
    }

    #[test]
    fn test_format_references_result_empty() {
        let result = format_references_result("unused_var", &[]);
        assert_eq!(result, "No references found for 'unused_var'");
    }

    #[test]
    fn test_format_references_result_grouped() {
        let locations = vec![
            make_location("/src/lib.rs", 10),
            make_location("/src/lib.rs", 20),
            make_location("/src/main.rs", 5),
        ];
        let result = format_references_result("my_func", &locations);

        assert!(result.contains("Found 3 reference(s)"));
        assert!(result.contains("in 2 file(s)"));
        assert!(result.contains("/src/lib.rs:"));
        assert!(result.contains("Line 11"));
        assert!(result.contains("Line 21"));
        assert!(result.contains("/src/main.rs:"));
        assert!(result.contains("Line 6"));
    }

    #[test]
    fn test_lsp_handler_default() {
        let handler = LspHandler::default();
        assert_eq!(handler.kind(), ToolKind::Function);
    }

    #[test]
    fn test_lsp_handler_matches_function_payload() {
        let handler = LspHandler::new();

        let function_payload = ToolPayload::Function {
            arguments: "{}".to_string(),
        };
        assert!(handler.matches_kind(&function_payload));
    }

    #[test]
    fn test_lsp_args_parsing() {
        let json =
            r#"{"operation": "goToDefinition", "filePath": "/src/lib.rs", "symbolName": "Config"}"#;
        let args: LspArgs = serde_json::from_str(json).unwrap();

        assert_eq!(args.operation, "goToDefinition");
        assert_eq!(args.file_path, "/src/lib.rs");
        assert_eq!(args.symbol_name, Some("Config".to_string()));
        assert_eq!(args.symbol_kind, None);
    }

    #[test]
    fn test_lsp_args_parsing_with_kind() {
        let json = r#"{"operation": "hover", "filePath": "/main.go", "symbolName": "Handler", "symbolKind": "function"}"#;
        let args: LspArgs = serde_json::from_str(json).unwrap();

        assert_eq!(args.operation, "hover");
        assert_eq!(args.file_path, "/main.go");
        assert_eq!(args.symbol_name, Some("Handler".to_string()));
        assert_eq!(args.symbol_kind, Some("function".to_string()));
    }

    #[test]
    fn test_lsp_args_parsing_with_direction() {
        let json = r#"{"operation": "getCallHierarchy", "filePath": "/src/lib.rs", "symbolName": "process", "direction": "incoming"}"#;
        let args: LspArgs = serde_json::from_str(json).unwrap();

        assert_eq!(args.operation, "getCallHierarchy");
        assert_eq!(args.file_path, "/src/lib.rs");
        assert_eq!(args.symbol_name, Some("process".to_string()));
        assert_eq!(args.direction, Some("incoming".to_string()));
    }

    #[test]
    fn test_format_implementation_result_empty() {
        let result = format_implementation_result("MyTrait", &[]);
        assert_eq!(result, "No implementations found for 'MyTrait'");
    }

    #[test]
    fn test_format_implementation_result_single() {
        let locations = vec![make_location("/src/impl.rs", 50)];
        let result = format_implementation_result("MyTrait", &locations);

        assert!(result.contains("Found 1 implementation(s)"));
        assert!(result.contains("/src/impl.rs:51:1"));
    }
}
