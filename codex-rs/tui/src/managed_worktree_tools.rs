//! Local managed-worktree operations invoked through authenticated TUI tool calls.
//! Durable ownership and attachments survive the bounded, process-local operation registry.
//! Creation does not change the caller's cwd or grant filesystem permissions.

use crate::legacy_core::config::Config;
use crate::legacy_core::config::ConfigBuilder;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::DynamicToolCallOutputContentItem;
use codex_app_server_protocol::DynamicToolCallResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadAttachmentAddParams;
use codex_app_server_protocol::ThreadAttachmentAddResponse;
use codex_app_server_protocol::ThreadAttachmentListParams;
use codex_app_server_protocol::ThreadAttachmentListResponse;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_worktree::CreateWorktree;
use codex_worktree::WorktreeManager;
use codex_worktree::WorktreeSettings;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::Instant;
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct ManagedWorktreeTools {
    manager: WorktreeManager,
    source_config_builder: ConfigBuilder,
    operations: Arc<Mutex<HashMap<String, Operation>>>,
}

struct Operation {
    thread_id: String,
    started: Instant,
    result: Option<Result<Value, String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateArguments {
    #[serde(rename = "ref")]
    revision: Option<String>,
    #[serde(rename = "allowAsync", alias = "async")]
    asynchronous: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StatusArguments {
    operation_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListArguments {
    cursor: Option<String>,
}

impl ManagedWorktreeTools {
    pub(crate) async fn new(config: &Config) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !config.active_project.is_untrusted(),
            "Worktree tools require a trusted source project"
        );
        let host = crate::legacy_core::config::load_config_toml_with_layer_stack(
            &config.codex_home,
            /*cwd*/ None,
            Vec::new(),
            codex_config::ConfigLoadOptions::default(),
        )
        .await?;
        Ok(Self {
            manager: WorktreeManager::new(WorktreeSettings::for_cli(
                &config.codex_home,
                host.config_toml.desktop.as_ref(),
            )?),
            source_config_builder: ConfigBuilder::default()
                .codex_home(config.codex_home.to_path_buf()),
            operations: Arc::default(),
        })
    }

    pub(crate) fn with_source_config_builder(mut self, builder: ConfigBuilder) -> Self {
        self.source_config_builder = builder;
        self
    }

    pub(crate) async fn execute(
        &self,
        handle: AppServerRequestHandle,
        thread_id: String,
        tool: &str,
        arguments: Value,
    ) -> DynamicToolCallResponse {
        let result = self.execute_inner(handle, thread_id, tool, arguments).await;
        let success = result.is_ok();
        let text = match result {
            Ok(value) => value.to_string(),
            Err(message) => json!({"error": message}).to_string(),
        };
        // Keep each injected tool result below the existing task-tools output budget.
        let (text, success) = if text.len() > 999 {
            ("Worktree result exceeds the display limit. Use the worktree browser to inspect retained checkouts; do not repeat creation.".to_owned(), false)
        } else {
            (text, success)
        };
        DynamicToolCallResponse {
            content_items: vec![DynamicToolCallOutputContentItem::InputText { text }],
            success,
        }
    }

    async fn execute_inner(
        &self,
        handle: AppServerRequestHandle,
        thread_id: String,
        tool: &str,
        arguments: Value,
    ) -> Result<Value, String> {
        if arguments.to_string().len() > 1024 {
            return Err("Worktree arguments are too large".to_owned());
        }
        match tool {
            "get_worktree_creation_status" => {
                let args: StatusArguments =
                    serde_json::from_value(arguments).map_err(|error| error.to_string())?;
                let operations = self.operations.lock().await;
                let operation = operations.get(&args.operation_id).filter(|operation| operation.thread_id == thread_id)
                    .ok_or("Unknown operation. Inspect this task's worktree attachments before creating another checkout.")?;
                operation.result.clone().unwrap_or_else(|| {
                    Ok(json!({"type":"pending", "operationId":args.operation_id}))
                })
            }
            "list_worktrees" => {
                let args: ListArguments =
                    serde_json::from_value(arguments).map_err(|error| error.to_string())?;
                let page = attachments(&handle, &thread_id, args.cursor).await?;
                let mut data = Vec::new();
                for item in page.data.into_iter().filter(|item| {
                    matches!(
                        item.attachment_type.as_str(),
                        "worktree" | "archived_worktree"
                    )
                }) {
                    let worktree = if item.attachment_type == "archived_worktree" {
                        &item.payload["worktree"]
                    } else {
                        &item.payload
                    };
                    let mut entry = json!({"id":item.id,"type":item.attachment_type,"root":worktree["root"],"workspaceRoot":worktree["workspaceRoot"],"sourceCwd":worktree["sourceCwd"]});
                    if entry.to_string().len() > 650 {
                        entry = json!({"id":item.id,"type":item.attachment_type,"truncated":true,"message":"Inspect this attachment in the worktree browser for its full paths."});
                    }
                    data.push(entry);
                }
                Ok(json!({"data":data,"nextCursor":page.next_cursor}))
            }
            "create_worktree" => {
                let args: CreateArguments =
                    serde_json::from_value(arguments).map_err(|error| error.to_string())?;
                if !args.asynchronous
                    || args
                        .revision
                        .as_ref()
                        .is_some_and(|value| value.len() > 128)
                {
                    return Err(
                        "Use allowAsync: true and a revision of at most 128 bytes".to_owned()
                    );
                }
                let response: ThreadReadResponse = handle
                    .request_typed(ClientRequest::ThreadRead {
                        request_id: request_id(),
                        params: ThreadReadParams {
                            thread_id: thread_id.clone(),
                            include_turns: false,
                        },
                    })
                    .await
                    .map_err(|error| error.to_string())?;
                if response.thread.ephemeral {
                    return Err("Ephemeral side conversations cannot retain worktree attachments. Request a new worktree in the main conversation.".to_owned());
                }
                // Verify capability before allocating a checkout, including SQLite availability.
                attachments(&handle, &thread_id, /*cursor*/ None).await?;
                let environments = response
                    .thread
                    .environments
                    .as_ref()
                    .ok_or("Server cannot verify the task's execution environment")?;
                if environments.is_empty()
                    || environments.iter().any(|environment| {
                        environment.environment_id != codex_exec_server::LOCAL_ENVIRONMENT_ID
                    })
                {
                    return Err(
                        "Managed worktree tools require a local execution environment".to_owned(),
                    );
                }
                let source_cwd = PathBuf::from(environments[0].cwd.as_str());
                let source = self
                    .source_config_builder
                    .clone()
                    .fallback_cwd(Some(source_cwd.clone()))
                    .build()
                    .await
                    .map_err(|error| error.to_string())?;
                if source.active_project.is_untrusted() {
                    return Err("Source project is explicitly untrusted".to_owned());
                }
                let mut operations = self.operations.lock().await;
                operations.retain(|_, operation| {
                    operation.result.is_none()
                        || operation.started.elapsed() < Duration::from_secs(3600)
                });
                if let Some((id, _)) = operations.iter().find(|(_, operation)| {
                    operation.thread_id == thread_id && operation.result.is_none()
                }) {
                    return Ok(json!({"type":"pending", "operationId":id}));
                }
                if operations.len() >= 64 {
                    return Err(
                        "Too many worktree operations; inspect existing checkouts first".to_owned(),
                    );
                }
                let operation_id = Uuid::new_v4().to_string();
                operations.insert(
                    operation_id.clone(),
                    Operation {
                        thread_id: thread_id.clone(),
                        started: Instant::now(),
                        result: None,
                    },
                );
                let manager = self.manager.clone();
                let registry = Arc::clone(&self.operations);
                let id = operation_id.clone();
                tokio::spawn(async move {
                    // Supervise the entire worker, including serialization and attachment RPCs.
                    // A panic after allocation must not leave status permanently pending.
                    let worker = tokio::spawn(async move {
                        let owner = thread_id.clone();
                        let created = tokio::task::spawn_blocking(move || {
                            let base = match args.revision {
                                Some(base) => base,
                                None => codex_worktree::default_worktree_base(&source_cwd)?,
                            };
                            let checkout = manager.create(&CreateWorktree {
                                source_cwd,
                                base: Some(base),
                            })?;
                            let registration = manager.bind_thread(&checkout.root, &owner);
                            Ok::<_, anyhow::Error>((checkout, registration))
                        })
                        .await;
                        match created {
                            Ok(Ok((checkout, registration))) => {
                                let payload = json!({"root":checkout.root,"workspaceRoot":checkout.cwd,"sourceCwd":checkout.source_cwd});
                                let registration = match registration {
                                    Ok(()) => handle
                                        .request_typed::<ThreadAttachmentAddResponse>(
                                            ClientRequest::ThreadAttachmentAdd {
                                                request_id: request_id(),
                                                params: ThreadAttachmentAddParams {
                                                    thread_id,
                                                    attachment_type: "worktree".to_owned(),
                                                    identity_key: checkout
                                                        .root
                                                        .to_string_lossy()
                                                        .into_owned(),
                                                    payload: payload.clone(),
                                                },
                                            },
                                        )
                                        .await
                                        .map(|_| ())
                                        .map_err(|error| error.to_string()),
                                    Err(error) => Err(error.to_string()),
                                };
                                Ok(
                                    json!({"type":if registration.is_ok() {"created"} else {"registrationFailed"}, "worktree":payload,
                                "message":if registration.is_ok() {"Keep the task cwd. Use the returned workspaceRoot explicitly and request filesystem permissions if needed."} else {"Checkout retained; registration failed. Do not create a duplicate. Inspect the worktree browser to recover."}}),
                                )
                            }
                            Ok(Err(error)) => Err(error.to_string()),
                            Err(error) => Err(format!(
                                "Worktree creation was interrupted: {error}. Inspect managed worktrees before retrying."
                            )),
                        }
                    });
                    let result = worker.await.unwrap_or_else(|error| {
                        Err(format!("Worktree creation outcome is uncertain: {error}. Inspect this task's attachments and managed worktrees before retrying."))
                    });
                    if let Some(operation) = registry.lock().await.get_mut(&id) {
                        operation.result = Some(result);
                    }
                });
                Ok(
                    json!({"type":"pending", "operationId":operation_id, "message":"Creation is running. Poll status; do not repeat creation. Work on independent tasks while waiting."}),
                )
            }
            _ => Err("Unknown managed worktree tool".to_owned()),
        }
    }
}

fn request_id() -> RequestId {
    RequestId::String(Uuid::new_v4().to_string())
}

async fn attachments(
    handle: &AppServerRequestHandle,
    thread_id: &str,
    cursor: Option<String>,
) -> Result<ThreadAttachmentListResponse, String> {
    handle
        .request_typed(ClientRequest::ThreadAttachmentList {
            request_id: request_id(),
            params: ThreadAttachmentListParams {
                thread_id: thread_id.to_owned(),
                cursor,
                limit: Some(1),
            },
        })
        .await
        .map_err(|error| format!("Managed attachments unavailable: {error}"))
}

#[cfg(test)]
#[path = "managed_worktree_tools_tests.rs"]
mod tests;
