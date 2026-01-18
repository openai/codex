use crate::client::jsonrpc::IncomingMessage;
use crate::client::jsonrpc::JsonRpcClient;
use crate::client::jsonrpc::JsonRpcError;
use crate::client::transport::Transport;
use crate::diagnostics::DiagnosticStore;
use crate::uri::uri_to_file_path;
use anyhow::Context;
use anyhow::Result;
use lsp_types::ClientCapabilities;
use lsp_types::ConfigurationParams;
use lsp_types::DidChangeTextDocumentParams;
use lsp_types::DidCloseTextDocumentParams;
use lsp_types::DidOpenTextDocumentParams;
use lsp_types::GeneralClientCapabilities;
use lsp_types::InitializeParams;
use lsp_types::InitializeResult;
use lsp_types::InitializedParams;
use lsp_types::PositionEncodingKind;
use lsp_types::PublishDiagnosticsParams;
use lsp_types::ReferenceParams;
use lsp_types::TextDocumentContentChangeEvent;
use lsp_types::TextDocumentIdentifier;
use lsp_types::TextDocumentItem;
use lsp_types::TextDocumentPositionParams;
use lsp_types::TextDocumentSyncCapability;
use lsp_types::TextDocumentSyncKind;
use lsp_types::Uri;
use lsp_types::WorkspaceFolder;
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct LspClient {
    rpc: JsonRpcClient,
    capabilities: lsp_types::ServerCapabilities,
    position_encoding: PositionEncodingKind,
}

impl LspClient {
    pub async fn start(
        command: &str,
        args: &[String],
        env: Option<&std::collections::HashMap<String, String>>,
        cwd: Option<&Path>,
        root_uri: Uri,
        diagnostics: DiagnosticStore,
    ) -> Result<Self> {
        let transport = Transport::spawn(command, args, env, cwd).await?;
        let (rpc, mut incoming) = JsonRpcClient::new(transport);

        let workspace_folders = vec![WorkspaceFolder {
            uri: root_uri.clone(),
            name: "workspace".to_string(),
        }];
        let initialize = build_initialize_params(workspace_folders.clone());
        let init_value = rpc
            .request("initialize", Some(serde_json::to_value(initialize)?))
            .await
            .context("initialize request")?;
        let init: InitializeResult =
            serde_json::from_value(init_value).context("deserialize initialize result")?;

        rpc.notify(
            "initialized",
            Some(serde_json::to_value(InitializedParams {})?),
        )
        .await?;

        let position_encoding = init
            .capabilities
            .position_encoding
            .clone()
            .unwrap_or(PositionEncodingKind::UTF16);

        let client = Self {
            rpc: rpc.clone(),
            capabilities: init.capabilities,
            position_encoding,
        };

        let rpc_clone = rpc.clone();
        let diagnostics = diagnostics.clone();
        let workspace_folders = Arc::new(workspace_folders);
        tokio::spawn({
            let workspace_folders = Arc::clone(&workspace_folders);
            async move {
                while let Some(message) = incoming.rx.recv().await {
                    handle_incoming(message, &rpc_clone, &diagnostics, &workspace_folders).await;
                }
            }
        });

        Ok(client)
    }

    pub fn position_encoding(&self) -> &PositionEncodingKind {
        &self.position_encoding
    }

    pub fn text_document_sync_kind(&self) -> TextDocumentSyncKind {
        match &self.capabilities.text_document_sync {
            Some(TextDocumentSyncCapability::Kind(kind)) => *kind,
            Some(TextDocumentSyncCapability::Options(options)) => {
                options.change.unwrap_or(TextDocumentSyncKind::FULL)
            }
            _ => TextDocumentSyncKind::FULL,
        }
    }

    pub async fn notify_did_open(
        &self,
        uri: Uri,
        language_id: &str,
        version: i32,
        text: String,
    ) -> Result<()> {
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: language_id.to_string(),
                version,
                text,
            },
        };
        self.rpc
            .notify("textDocument/didOpen", Some(serde_json::to_value(params)?))
            .await
    }

    pub async fn notify_did_change(&self, uri: Uri, version: i32, text: String) -> Result<()> {
        let params = DidChangeTextDocumentParams {
            text_document: lsp_types::VersionedTextDocumentIdentifier { uri, version },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text,
            }],
        };
        self.rpc
            .notify(
                "textDocument/didChange",
                Some(serde_json::to_value(params)?),
            )
            .await
    }

    pub async fn notify_did_close(&self, uri: Uri) -> Result<()> {
        let params = DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
        };
        self.rpc
            .notify("textDocument/didClose", Some(serde_json::to_value(params)?))
            .await
    }

    pub async fn request_definition(
        &self,
        text_document: TextDocumentIdentifier,
        position: lsp_types::Position,
    ) -> Result<Value, JsonRpcError> {
        let params = TextDocumentPositionParams {
            text_document,
            position,
        };
        let params = serde_json::to_value(params).map_err(|err| JsonRpcError {
            code: -32603,
            message: format!("serialize definition params failed: {err}"),
            data: None,
        })?;
        self.rpc
            .request("textDocument/definition", Some(params))
            .await
    }

    pub async fn request_references(
        &self,
        text_document: TextDocumentIdentifier,
        position: lsp_types::Position,
        include_declaration: bool,
    ) -> Result<Value, JsonRpcError> {
        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document,
                position,
            },
            context: lsp_types::ReferenceContext {
                include_declaration,
            },
            partial_result_params: lsp_types::PartialResultParams::default(),
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        };
        let params = serde_json::to_value(params).map_err(|err| JsonRpcError {
            code: -32603,
            message: format!("serialize references params failed: {err}"),
            data: None,
        })?;
        self.rpc
            .request("textDocument/references", Some(params))
            .await
    }

    pub async fn request_rename(
        &self,
        text_document: TextDocumentIdentifier,
        position: lsp_types::Position,
        new_name: &str,
    ) -> Result<Value, JsonRpcError> {
        let params = lsp_types::RenameParams {
            text_document_position: TextDocumentPositionParams {
                text_document,
                position,
            },
            new_name: new_name.to_string(),
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        };
        let params = serde_json::to_value(params).map_err(|err| JsonRpcError {
            code: -32603,
            message: format!("serialize rename params failed: {err}"),
            data: None,
        })?;
        self.rpc.request("textDocument/rename", Some(params)).await
    }

    pub async fn shutdown(&self) -> Result<()> {
        let _ = self.rpc.request("shutdown", None).await;
        self.rpc.notify("exit", None).await?;
        Ok(())
    }
}

fn build_initialize_params(workspace_folders: Vec<WorkspaceFolder>) -> InitializeParams {
    InitializeParams {
        process_id: Some(std::process::id()),
        capabilities: ClientCapabilities {
            general: Some(GeneralClientCapabilities {
                position_encodings: Some(vec![
                    PositionEncodingKind::UTF16,
                    PositionEncodingKind::UTF8,
                ]),
                ..Default::default()
            }),
            workspace: Some(lsp_types::WorkspaceClientCapabilities {
                configuration: Some(true),
                workspace_folders: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        },
        initialization_options: None,
        trace: None,
        workspace_folders: Some(workspace_folders),
        client_info: Some(lsp_types::ClientInfo {
            name: "codex".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
        locale: None,
        ..Default::default()
    }
}

async fn handle_incoming(
    message: IncomingMessage,
    rpc: &JsonRpcClient,
    diagnostics: &DiagnosticStore,
    workspace_folders: &Arc<Vec<WorkspaceFolder>>,
) {
    match message {
        IncomingMessage::Notification { method, params } => {
            if method == "textDocument/publishDiagnostics"
                && let Some(params) = params
                && let Ok(parsed) = serde_json::from_value::<PublishDiagnosticsParams>(params)
                && let Some(path) = uri_to_file_path(&parsed.uri)
            {
                diagnostics.update(path, parsed.diagnostics);
            }
        }
        IncomingMessage::Request { id, method, params } => {
            let _ = match method.as_str() {
                "workspace/configuration" => {
                    let mut items = Vec::new();
                    if let Some(params) = params
                        && let Ok(parsed) = serde_json::from_value::<ConfigurationParams>(params)
                    {
                        items.resize(parsed.items.len(), Value::Null);
                    }
                    rpc.respond(id, Some(Value::Array(items)), None).await
                }
                "client/registerCapability" => rpc.respond(id, None, None).await,
                "window/showMessageRequest" => rpc.respond(id, None, None).await,
                "workspace/workspaceFolders" => {
                    let folders = serde_json::to_value(workspace_folders.as_ref())
                        .unwrap_or_else(|_| Value::Array(Vec::new()));
                    rpc.respond(id, Some(folders), None).await
                }
                _ => {
                    let error = crate::client::jsonrpc::JsonRpcError {
                        code: -32601,
                        message: format!("unsupported request: {method}"),
                        data: None,
                    };
                    rpc.respond(id, None, Some(error)).await
                }
            };
        }
    }
}
