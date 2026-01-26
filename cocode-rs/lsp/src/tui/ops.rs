//! LSP operation wrappers.

use super::app::CallHierarchyResult;
use super::app::LspErrorContext;
use super::app::LspResult;
use super::app::Operation;
use cocode_lsp::LspServerManager;
use cocode_lsp::SymbolKind;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

/// Helper to create a structured error result.
fn make_error(
    operation: &str,
    error: impl ToString,
    file: Option<&Path>,
    symbol: Option<&str>,
) -> LspResult {
    LspResult::Error(LspErrorContext {
        operation: operation.to_string(),
        file: file.map(|p| p.display().to_string()),
        symbol: symbol.map(|s| s.to_string()),
        error: error.to_string(),
    })
}

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
        Operation::ConfigureServers => execute_list_servers(manager).await,
        // InstallBinaries is handled directly in app.rs, should not reach here
        Operation::InstallBinaries => LspResult::Error(LspErrorContext {
            operation: "install_binaries".to_string(),
            file: None,
            symbol: None,
            error: "InstallBinaries is handled directly in app.rs".to_string(),
        }),
    }
}

async fn execute_definition(
    manager: Arc<LspServerManager>,
    file: Option<PathBuf>,
    symbol: String,
    symbol_kind: Option<SymbolKind>,
) -> LspResult {
    info!(
        operation = "definition",
        file = ?file.as_ref().map(|p| p.display().to_string()),
        symbol = %symbol,
        "Executing LSP operation"
    );

    let Some(file) = file else {
        return make_error("definition", "No file specified", None, Some(&symbol));
    };

    match manager.get_client(&file).await {
        Ok(client) => match client.definition(&file, &symbol, symbol_kind).await {
            Ok(locations) => LspResult::Locations(locations),
            Err(e) => make_error("definition", e, Some(&file), Some(&symbol)),
        },
        Err(e) => make_error(
            "definition",
            format!("Failed to get client: {e}"),
            Some(&file),
            Some(&symbol),
        ),
    }
}

async fn execute_references(
    manager: Arc<LspServerManager>,
    file: Option<PathBuf>,
    symbol: String,
    symbol_kind: Option<SymbolKind>,
) -> LspResult {
    info!(
        operation = "references",
        file = ?file.as_ref().map(|p| p.display().to_string()),
        symbol = %symbol,
        "Executing LSP operation"
    );

    let Some(file) = file else {
        return make_error("references", "No file specified", None, Some(&symbol));
    };

    match manager.get_client(&file).await {
        Ok(client) => match client.references(&file, &symbol, symbol_kind, true).await {
            Ok(locations) => LspResult::Locations(locations),
            Err(e) => make_error("references", e, Some(&file), Some(&symbol)),
        },
        Err(e) => make_error(
            "references",
            format!("Failed to get client: {e}"),
            Some(&file),
            Some(&symbol),
        ),
    }
}

async fn execute_implementation(
    manager: Arc<LspServerManager>,
    file: Option<PathBuf>,
    symbol: String,
    symbol_kind: Option<SymbolKind>,
) -> LspResult {
    info!(
        operation = "implementation",
        file = ?file.as_ref().map(|p| p.display().to_string()),
        symbol = %symbol,
        "Executing LSP operation"
    );

    let Some(file) = file else {
        return make_error("implementation", "No file specified", None, Some(&symbol));
    };

    match manager.get_client(&file).await {
        Ok(client) => {
            if !client.supports_implementation().await {
                return make_error(
                    "implementation",
                    "Server does not support implementation",
                    Some(&file),
                    Some(&symbol),
                );
            }
            match client.implementation(&file, &symbol, symbol_kind).await {
                Ok(locations) => LspResult::Locations(locations),
                Err(e) => make_error("implementation", e, Some(&file), Some(&symbol)),
            }
        }
        Err(e) => make_error(
            "implementation",
            format!("Failed to get client: {e}"),
            Some(&file),
            Some(&symbol),
        ),
    }
}

async fn execute_hover(
    manager: Arc<LspServerManager>,
    file: Option<PathBuf>,
    symbol: String,
    symbol_kind: Option<SymbolKind>,
) -> LspResult {
    info!(
        operation = "hover",
        file = ?file.as_ref().map(|p| p.display().to_string()),
        symbol = %symbol,
        "Executing LSP operation"
    );

    let Some(file) = file else {
        return make_error("hover", "No file specified", None, Some(&symbol));
    };

    match manager.get_client(&file).await {
        Ok(client) => match client.hover(&file, &symbol, symbol_kind).await {
            Ok(hover_info) => LspResult::HoverInfo(hover_info),
            Err(e) => make_error("hover", e, Some(&file), Some(&symbol)),
        },
        Err(e) => make_error(
            "hover",
            format!("Failed to get client: {e}"),
            Some(&file),
            Some(&symbol),
        ),
    }
}

async fn execute_workspace_symbol(manager: Arc<LspServerManager>, query: String) -> LspResult {
    info!(
        operation = "workspace_symbol",
        query = %query,
        "Executing LSP operation"
    );

    // Try to get any client - use the manager's supported extensions to find one
    let extensions = manager.all_supported_extensions().await;
    if extensions.is_empty() {
        return make_error(
            "workspace_symbol",
            "No language servers configured",
            None,
            Some(&query),
        );
    }

    // Create a dummy path with a supported extension to get a client
    let dummy_path = PathBuf::from(format!("dummy{}", extensions[0]));

    match manager.get_client(&dummy_path).await {
        Ok(client) => {
            if !client.supports_workspace_symbol().await {
                return make_error(
                    "workspace_symbol",
                    "Server does not support workspace symbol",
                    None,
                    Some(&query),
                );
            }
            match client.workspace_symbol(&query).await {
                Ok(symbols) => {
                    // workspace_symbol returns Vec<SymbolInformation>
                    // We need to convert to our simplified format
                    LspResult::WorkspaceSymbols(symbols)
                }
                Err(e) => make_error("workspace_symbol", e, None, Some(&query)),
            }
        }
        Err(e) => make_error(
            "workspace_symbol",
            format!("Failed to get client: {e}"),
            None,
            Some(&query),
        ),
    }
}

async fn execute_document_symbols(
    manager: Arc<LspServerManager>,
    file: Option<PathBuf>,
) -> LspResult {
    info!(
        operation = "document_symbols",
        file = ?file.as_ref().map(|p| p.display().to_string()),
        "Executing LSP operation"
    );

    let Some(file) = file else {
        return make_error("document_symbols", "No file specified", None, None);
    };

    match manager.get_client(&file).await {
        Ok(client) => match client.document_symbols(&file).await {
            Ok(symbols) => LspResult::Symbols((*symbols).clone()),
            Err(e) => make_error("document_symbols", e, Some(&file), None),
        },
        Err(e) => make_error(
            "document_symbols",
            format!("Failed to get client: {e}"),
            Some(&file),
            None,
        ),
    }
}

async fn execute_type_definition(
    manager: Arc<LspServerManager>,
    file: Option<PathBuf>,
    symbol: String,
    symbol_kind: Option<SymbolKind>,
) -> LspResult {
    info!(
        operation = "type_definition",
        file = ?file.as_ref().map(|p| p.display().to_string()),
        symbol = %symbol,
        "Executing LSP operation"
    );

    let Some(file) = file else {
        return make_error("type_definition", "No file specified", None, Some(&symbol));
    };

    match manager.get_client(&file).await {
        Ok(client) => {
            if !client.supports_type_definition().await {
                return make_error(
                    "type_definition",
                    "Server does not support type definition",
                    Some(&file),
                    Some(&symbol),
                );
            }
            match client.type_definition(&file, &symbol, symbol_kind).await {
                Ok(locations) => LspResult::Locations(locations),
                Err(e) => make_error("type_definition", e, Some(&file), Some(&symbol)),
            }
        }
        Err(e) => make_error(
            "type_definition",
            format!("Failed to get client: {e}"),
            Some(&file),
            Some(&symbol),
        ),
    }
}

async fn execute_declaration(
    manager: Arc<LspServerManager>,
    file: Option<PathBuf>,
    symbol: String,
    symbol_kind: Option<SymbolKind>,
) -> LspResult {
    info!(
        operation = "declaration",
        file = ?file.as_ref().map(|p| p.display().to_string()),
        symbol = %symbol,
        "Executing LSP operation"
    );

    let Some(file) = file else {
        return make_error("declaration", "No file specified", None, Some(&symbol));
    };

    match manager.get_client(&file).await {
        Ok(client) => {
            if !client.supports_declaration().await {
                return make_error(
                    "declaration",
                    "Server does not support declaration",
                    Some(&file),
                    Some(&symbol),
                );
            }
            match client.declaration(&file, &symbol, symbol_kind).await {
                Ok(locations) => LspResult::Locations(locations),
                Err(e) => make_error("declaration", e, Some(&file), Some(&symbol)),
            }
        }
        Err(e) => make_error(
            "declaration",
            format!("Failed to get client: {e}"),
            Some(&file),
            Some(&symbol),
        ),
    }
}

async fn execute_call_hierarchy(
    manager: Arc<LspServerManager>,
    file: Option<PathBuf>,
    symbol: String,
    symbol_kind: Option<SymbolKind>,
) -> LspResult {
    info!(
        operation = "call_hierarchy",
        file = ?file.as_ref().map(|p| p.display().to_string()),
        symbol = %symbol,
        "Executing LSP operation"
    );

    let Some(file) = file else {
        return make_error("call_hierarchy", "No file specified", None, Some(&symbol));
    };

    match manager.get_client(&file).await {
        Ok(client) => {
            if !client.supports_call_hierarchy().await {
                return make_error(
                    "call_hierarchy",
                    "Server does not support call hierarchy",
                    Some(&file),
                    Some(&symbol),
                );
            }

            // First, prepare call hierarchy to get the items
            let items = match client
                .prepare_call_hierarchy(&file, &symbol, symbol_kind)
                .await
            {
                Ok(items) => items,
                Err(e) => {
                    return make_error("call_hierarchy", e, Some(&file), Some(&symbol));
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
        Err(e) => make_error(
            "call_hierarchy",
            format!("Failed to get client: {e}"),
            Some(&file),
            Some(&symbol),
        ),
    }
}

async fn execute_health_check(manager: Arc<LspServerManager>, file: Option<PathBuf>) -> LspResult {
    info!(
        operation = "health_check",
        file = ?file.as_ref().map(|p| p.display().to_string()),
        "Executing LSP operation"
    );

    // If a file is specified, get the client for that file type
    // Otherwise, try to find any client
    let extensions = manager.all_supported_extensions().await;
    if extensions.is_empty() {
        return make_error("health_check", "No language servers configured", None, None);
    }

    let path = file.unwrap_or_else(|| PathBuf::from(format!("dummy{}", extensions[0])));

    match manager.get_client(&path).await {
        Ok(client) => {
            let is_healthy = client.health_check().await;
            if is_healthy {
                LspResult::HealthOk("Server is healthy".to_string())
            } else {
                make_error(
                    "health_check",
                    "Server health check failed",
                    Some(&path),
                    None,
                )
            }
        }
        Err(e) => make_error(
            "health_check",
            format!("Failed to get client: {e}"),
            Some(&path),
            None,
        ),
    }
}

async fn execute_list_servers(manager: Arc<LspServerManager>) -> LspResult {
    info!(operation = "list_servers", "Executing LSP operation");
    let servers = manager.get_all_servers_status().await;
    LspResult::ServerList(servers)
}
