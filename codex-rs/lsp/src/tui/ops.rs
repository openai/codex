//! LSP operation wrappers.

use super::app::CallHierarchyResult;
use super::app::LspResult;
use super::app::Operation;
use codex_lsp::LspServerManager;
use codex_lsp::SymbolKind;
use std::path::PathBuf;
use std::sync::Arc;

/// Execute an LSP operation and return the result.
pub async fn execute_operation(
    operation: Operation,
    manager: Arc<LspServerManager>,
    file: Option<PathBuf>,
    symbol: String,
    symbol_kind: Option<SymbolKind>,
) -> LspResult {
    match operation {
        Operation::Definition => execute_definition(manager, file, symbol, symbol_kind).await,
        Operation::TypeDefinition => {
            execute_type_definition(manager, file, symbol, symbol_kind).await
        }
        Operation::Declaration => execute_declaration(manager, file, symbol, symbol_kind).await,
        Operation::References => execute_references(manager, file, symbol, symbol_kind).await,
        Operation::Implementation => {
            execute_implementation(manager, file, symbol, symbol_kind).await
        }
        Operation::Hover => execute_hover(manager, file, symbol, symbol_kind).await,
        Operation::WorkspaceSymbol => execute_workspace_symbol(manager, symbol).await,
        Operation::DocumentSymbols => execute_document_symbols(manager, file).await,
        Operation::CallHierarchy => {
            execute_call_hierarchy(manager, file, symbol, symbol_kind).await
        }
        Operation::HealthCheck => execute_health_check(manager, file).await,
    }
}

async fn execute_definition(
    manager: Arc<LspServerManager>,
    file: Option<PathBuf>,
    symbol: String,
    symbol_kind: Option<SymbolKind>,
) -> LspResult {
    let Some(file) = file else {
        return LspResult::Error("No file specified".to_string());
    };

    match manager.get_client(&file).await {
        Ok(client) => match client.definition(&file, &symbol, symbol_kind).await {
            Ok(locations) => LspResult::Locations(locations),
            Err(e) => LspResult::Error(format!("Definition failed: {e}")),
        },
        Err(e) => LspResult::Error(format!("Failed to get client: {e}")),
    }
}

async fn execute_references(
    manager: Arc<LspServerManager>,
    file: Option<PathBuf>,
    symbol: String,
    symbol_kind: Option<SymbolKind>,
) -> LspResult {
    let Some(file) = file else {
        return LspResult::Error("No file specified".to_string());
    };

    match manager.get_client(&file).await {
        Ok(client) => match client.references(&file, &symbol, symbol_kind, true).await {
            Ok(locations) => LspResult::Locations(locations),
            Err(e) => LspResult::Error(format!("References failed: {e}")),
        },
        Err(e) => LspResult::Error(format!("Failed to get client: {e}")),
    }
}

async fn execute_implementation(
    manager: Arc<LspServerManager>,
    file: Option<PathBuf>,
    symbol: String,
    symbol_kind: Option<SymbolKind>,
) -> LspResult {
    let Some(file) = file else {
        return LspResult::Error("No file specified".to_string());
    };

    match manager.get_client(&file).await {
        Ok(client) => {
            if !client.supports_implementation().await {
                return LspResult::Error("Server does not support implementation".to_string());
            }
            match client.implementation(&file, &symbol, symbol_kind).await {
                Ok(locations) => LspResult::Locations(locations),
                Err(e) => LspResult::Error(format!("Implementation failed: {e}")),
            }
        }
        Err(e) => LspResult::Error(format!("Failed to get client: {e}")),
    }
}

async fn execute_hover(
    manager: Arc<LspServerManager>,
    file: Option<PathBuf>,
    symbol: String,
    symbol_kind: Option<SymbolKind>,
) -> LspResult {
    let Some(file) = file else {
        return LspResult::Error("No file specified".to_string());
    };

    match manager.get_client(&file).await {
        Ok(client) => match client.hover(&file, &symbol, symbol_kind).await {
            Ok(hover_info) => LspResult::HoverInfo(hover_info),
            Err(e) => LspResult::Error(format!("Hover failed: {e}")),
        },
        Err(e) => LspResult::Error(format!("Failed to get client: {e}")),
    }
}

async fn execute_workspace_symbol(manager: Arc<LspServerManager>, query: String) -> LspResult {
    // Try to get any client - use the manager's supported extensions to find one
    let extensions = manager.all_supported_extensions();
    if extensions.is_empty() {
        return LspResult::Error("No language servers configured".to_string());
    }

    // Create a dummy path with a supported extension to get a client
    let dummy_path = PathBuf::from(format!("dummy{}", extensions[0]));

    match manager.get_client(&dummy_path).await {
        Ok(client) => {
            if !client.supports_workspace_symbol().await {
                return LspResult::Error("Server does not support workspace symbol".to_string());
            }
            match client.workspace_symbol(&query).await {
                Ok(symbols) => {
                    // workspace_symbol returns Vec<SymbolInformation>
                    // We need to convert to our simplified format
                    LspResult::WorkspaceSymbols(symbols)
                }
                Err(e) => LspResult::Error(format!("Workspace symbol failed: {e}")),
            }
        }
        Err(e) => LspResult::Error(format!("Failed to get client: {e}")),
    }
}

async fn execute_document_symbols(
    manager: Arc<LspServerManager>,
    file: Option<PathBuf>,
) -> LspResult {
    let Some(file) = file else {
        return LspResult::Error("No file specified".to_string());
    };

    match manager.get_client(&file).await {
        Ok(client) => match client.document_symbols(&file).await {
            Ok(symbols) => LspResult::Symbols((*symbols).clone()),
            Err(e) => LspResult::Error(format!("Document symbols failed: {e}")),
        },
        Err(e) => LspResult::Error(format!("Failed to get client: {e}")),
    }
}

async fn execute_type_definition(
    manager: Arc<LspServerManager>,
    file: Option<PathBuf>,
    symbol: String,
    symbol_kind: Option<SymbolKind>,
) -> LspResult {
    let Some(file) = file else {
        return LspResult::Error("No file specified".to_string());
    };

    match manager.get_client(&file).await {
        Ok(client) => {
            if !client.supports_type_definition().await {
                return LspResult::Error("Server does not support type definition".to_string());
            }
            match client.type_definition(&file, &symbol, symbol_kind).await {
                Ok(locations) => LspResult::Locations(locations),
                Err(e) => LspResult::Error(format!("Type definition failed: {e}")),
            }
        }
        Err(e) => LspResult::Error(format!("Failed to get client: {e}")),
    }
}

async fn execute_declaration(
    manager: Arc<LspServerManager>,
    file: Option<PathBuf>,
    symbol: String,
    symbol_kind: Option<SymbolKind>,
) -> LspResult {
    let Some(file) = file else {
        return LspResult::Error("No file specified".to_string());
    };

    match manager.get_client(&file).await {
        Ok(client) => {
            if !client.supports_declaration().await {
                return LspResult::Error("Server does not support declaration".to_string());
            }
            match client.declaration(&file, &symbol, symbol_kind).await {
                Ok(locations) => LspResult::Locations(locations),
                Err(e) => LspResult::Error(format!("Declaration failed: {e}")),
            }
        }
        Err(e) => LspResult::Error(format!("Failed to get client: {e}")),
    }
}

async fn execute_call_hierarchy(
    manager: Arc<LspServerManager>,
    file: Option<PathBuf>,
    symbol: String,
    symbol_kind: Option<SymbolKind>,
) -> LspResult {
    let Some(file) = file else {
        return LspResult::Error("No file specified".to_string());
    };

    match manager.get_client(&file).await {
        Ok(client) => {
            if !client.supports_call_hierarchy().await {
                return LspResult::Error("Server does not support call hierarchy".to_string());
            }

            // First, prepare call hierarchy to get the items
            let items = match client
                .prepare_call_hierarchy(&file, &symbol, symbol_kind)
                .await
            {
                Ok(items) => items,
                Err(e) => {
                    return LspResult::Error(format!("Prepare call hierarchy failed: {e}"));
                }
            };

            if items.is_empty() {
                return LspResult::CallHierarchy(CallHierarchyResult {
                    items: vec!["No call hierarchy items found".to_string()],
                    incoming: vec![],
                    outgoing: vec![],
                });
            }

            // Get incoming and outgoing calls for the first item
            let item = items[0].clone();
            let item_name = item.name.clone();

            let incoming = match client.incoming_calls(item.clone()).await {
                Ok(calls) => calls
                    .iter()
                    .map(|c| {
                        format!(
                            "{} ({}:{})",
                            c.from.name,
                            c.from.uri.path().rsplit('/').next().unwrap_or("?"),
                            c.from.range.start.line + 1
                        )
                    })
                    .collect(),
                Err(_) => vec!["(failed to get incoming calls)".to_string()],
            };

            let outgoing = match client.outgoing_calls(item).await {
                Ok(calls) => calls
                    .iter()
                    .map(|c| {
                        format!(
                            "{} ({}:{})",
                            c.to.name,
                            c.to.uri.path().rsplit('/').next().unwrap_or("?"),
                            c.to.range.start.line + 1
                        )
                    })
                    .collect(),
                Err(_) => vec!["(failed to get outgoing calls)".to_string()],
            };

            LspResult::CallHierarchy(CallHierarchyResult {
                items: vec![item_name],
                incoming,
                outgoing,
            })
        }
        Err(e) => LspResult::Error(format!("Failed to get client: {e}")),
    }
}

async fn execute_health_check(manager: Arc<LspServerManager>, file: Option<PathBuf>) -> LspResult {
    // If a file is specified, get the client for that file type
    // Otherwise, try to find any client
    let extensions = manager.all_supported_extensions();
    if extensions.is_empty() {
        return LspResult::Error("No language servers configured".to_string());
    }

    let path = file.unwrap_or_else(|| PathBuf::from(format!("dummy{}", extensions[0])));

    match manager.get_client(&path).await {
        Ok(client) => {
            let is_healthy = client.health_check().await;
            if is_healthy {
                LspResult::HealthOk("Server is healthy".to_string())
            } else {
                LspResult::Error("Server health check failed".to_string())
            }
        }
        Err(e) => LspResult::Error(format!("Failed to get client: {e}")),
    }
}
