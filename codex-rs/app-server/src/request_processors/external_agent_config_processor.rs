use std::sync::Arc;

use crate::config::external_agent_config::ExternalAgentConfigDetectOptions;
use crate::config::external_agent_config::ExternalAgentConfigImportItemResult as CoreImportItemResult;
use crate::config::external_agent_config::ExternalAgentConfigImportOutcome as CoreImportOutcome;
use crate::config::external_agent_config::ExternalAgentConfigImportRawError as CoreImportRawError;
use crate::config::external_agent_config::ExternalAgentConfigMigrationItem as CoreMigrationItem;
use crate::config::external_agent_config::ExternalAgentConfigMigrationItemType as CoreMigrationItemType;
use crate::config::external_agent_config::ExternalAgentConfigService;
use crate::config::external_agent_config::NamedMigration as CoreNamedMigration;
use crate::config::external_agent_config::PendingPluginImport;
use crate::config::external_agent_config::PluginImportOutcome;
use crate::config::external_agent_config::record_import_error;
use crate::config_manager::ConfigManager;
use crate::error_code::internal_error;
use crate::outgoing_message::ConnectionRequestId;
use crate::outgoing_message::OutgoingMessageSender;
use codex_app_server_protocol::CommandMigration;
use codex_app_server_protocol::ExternalAgentConfigDetectParams;
use codex_app_server_protocol::ExternalAgentConfigDetectResponse;
use codex_app_server_protocol::ExternalAgentConfigImportCompletedNotification;
use codex_app_server_protocol::ExternalAgentConfigImportItemTypeFailure as ProtocolImportFailure;
use codex_app_server_protocol::ExternalAgentConfigImportItemTypeSuccess as ProtocolImportSuccess;
use codex_app_server_protocol::ExternalAgentConfigImportParams;
use codex_app_server_protocol::ExternalAgentConfigImportProgressNotification;
use codex_app_server_protocol::ExternalAgentConfigImportResponse;
use codex_app_server_protocol::ExternalAgentConfigImportTypeResult as ProtocolImportTypeResult;
use codex_app_server_protocol::ExternalAgentConfigMigrationItem;
use codex_app_server_protocol::ExternalAgentConfigMigrationItemType;
use codex_app_server_protocol::HookMigration;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::McpServerMigration;
use codex_app_server_protocol::MigrationDetails;
use codex_app_server_protocol::PluginsMigration;
use codex_app_server_protocol::ServerNotification;
use codex_arg0::Arg0DispatchPaths;
use codex_core::ThreadManager;
use codex_external_agent_sessions::ExternalAgentSessionMigration as CoreSessionMigration;
use codex_rollout::StateDbHandle;
use codex_state::ExternalAgentConfigImportFailureRecord;
use codex_state::ExternalAgentConfigImportSuccessRecord;
use codex_thread_store::ThreadStore;
use std::collections::HashSet;
use std::path::PathBuf;

use super::ConfigRequestProcessor;
use super::external_agent_session_import::ExternalAgentSessionImporter;
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct ExternalAgentConfigRequestProcessor {
    outgoing: Arc<OutgoingMessageSender>,
    migration_service: ExternalAgentConfigService,
    session_importer: ExternalAgentSessionImporter,
    thread_manager: Arc<ThreadManager>,
    config_processor: ConfigRequestProcessor,
    state_db: Option<StateDbHandle>,
}

pub(crate) struct ExternalAgentConfigRequestProcessorArgs {
    pub(crate) outgoing: Arc<OutgoingMessageSender>,
    pub(crate) thread_manager: Arc<ThreadManager>,
    pub(crate) thread_store: Arc<dyn ThreadStore>,
    pub(crate) config_manager: ConfigManager,
    pub(crate) config_processor: ConfigRequestProcessor,
    pub(crate) state_db: Option<StateDbHandle>,
    pub(crate) arg0_paths: Arg0DispatchPaths,
    pub(crate) codex_home: PathBuf,
}

impl ExternalAgentConfigRequestProcessor {
    pub(crate) fn new(args: ExternalAgentConfigRequestProcessorArgs) -> Self {
        let ExternalAgentConfigRequestProcessorArgs {
            outgoing,
            thread_manager,
            thread_store,
            config_manager,
            config_processor,
            state_db,
            arg0_paths,
            codex_home,
        } = args;
        let session_importer = ExternalAgentSessionImporter::new(
            codex_home.clone(),
            Arc::clone(&thread_manager),
            thread_store,
            config_manager,
            arg0_paths,
        );
        Self {
            outgoing,
            migration_service: ExternalAgentConfigService::new(codex_home),
            session_importer,
            thread_manager,
            config_processor,
            state_db,
        }
    }

    pub(crate) async fn detect(
        &self,
        params: ExternalAgentConfigDetectParams,
    ) -> Result<ExternalAgentConfigDetectResponse, JSONRPCErrorError> {
        let items = self
            .migration_service
            .detect(ExternalAgentConfigDetectOptions {
                include_home: params.include_home,
                cwds: params.cwds,
            })
            .await
            .map_err(|err| internal_error(err.to_string()))?;

        Ok(ExternalAgentConfigDetectResponse {
            items: items
                .into_iter()
                .map(|migration_item| ExternalAgentConfigMigrationItem {
                    item_type: match migration_item.item_type {
                        CoreMigrationItemType::Config => {
                            ExternalAgentConfigMigrationItemType::Config
                        }
                        CoreMigrationItemType::Skills => {
                            ExternalAgentConfigMigrationItemType::Skills
                        }
                        CoreMigrationItemType::AgentsMd => {
                            ExternalAgentConfigMigrationItemType::AgentsMd
                        }
                        CoreMigrationItemType::Plugins => {
                            ExternalAgentConfigMigrationItemType::Plugins
                        }
                        CoreMigrationItemType::McpServerConfig => {
                            ExternalAgentConfigMigrationItemType::McpServerConfig
                        }
                        CoreMigrationItemType::Subagents => {
                            ExternalAgentConfigMigrationItemType::Subagents
                        }
                        CoreMigrationItemType::Hooks => ExternalAgentConfigMigrationItemType::Hooks,
                        CoreMigrationItemType::Commands => {
                            ExternalAgentConfigMigrationItemType::Commands
                        }
                        CoreMigrationItemType::Sessions => {
                            ExternalAgentConfigMigrationItemType::Sessions
                        }
                    },
                    description: migration_item.description,
                    cwd: migration_item.cwd,
                    details: migration_item.details.map(|details| MigrationDetails {
                        plugins: details
                            .plugins
                            .into_iter()
                            .map(|plugin| PluginsMigration {
                                marketplace_name: plugin.marketplace_name,
                                plugin_names: plugin.plugin_names,
                            })
                            .collect(),
                        sessions: details
                            .sessions
                            .into_iter()
                            .map(|session| codex_app_server_protocol::SessionMigration {
                                path: session.path,
                                cwd: session.cwd,
                                title: session.title,
                            })
                            .collect(),
                        mcp_servers: details
                            .mcp_servers
                            .into_iter()
                            .map(|mcp_server| McpServerMigration {
                                name: mcp_server.name,
                            })
                            .collect(),
                        hooks: details
                            .hooks
                            .into_iter()
                            .map(|hook| HookMigration { name: hook.name })
                            .collect(),
                        subagents: details
                            .subagents
                            .into_iter()
                            .map(|subagent| codex_app_server_protocol::SubagentMigration {
                                name: subagent.name,
                            })
                            .collect(),
                        commands: details
                            .commands
                            .into_iter()
                            .map(|command| CommandMigration { name: command.name })
                            .collect(),
                    }),
                })
                .collect(),
        })
    }

    pub(crate) async fn import(
        &self,
        request_id: ConnectionRequestId,
        params: ExternalAgentConfigImportParams,
    ) -> Result<(), JSONRPCErrorError> {
        let import_id = Uuid::new_v4().to_string();
        let needs_runtime_refresh = migration_items_need_runtime_refresh(&params.migration_items);
        let has_migration_items = !params.migration_items.is_empty();
        let has_plugin_imports = params.migration_items.iter().any(|item| {
            matches!(
                item.item_type,
                ExternalAgentConfigMigrationItemType::Plugins
            )
        });
        let (pending_session_imports, session_validation_result) =
            self.validate_pending_session_imports(&params);
        let import_outcome = self.import_external_agent_config(params).await?;
        if needs_runtime_refresh {
            self.config_processor.handle_config_mutation().await;
        }
        self.outgoing
            .send_response(
                request_id,
                ExternalAgentConfigImportResponse {
                    import_id: import_id.clone(),
                },
            )
            .await;

        if !has_migration_items {
            return Ok(());
        }

        let mut completed_item_results = Vec::new();
        if let Some(session_validation_result) = session_validation_result {
            send_import_progress(&self.outgoing, &import_id, &session_validation_result).await;
            completed_item_results.push(session_validation_result);
        }
        for item_result in import_outcome.item_results {
            send_import_progress(&self.outgoing, &import_id, &item_result).await;
            completed_item_results.push(item_result);
        }

        let has_background_imports = !import_outcome.pending_plugin_imports.is_empty()
            || !pending_session_imports.is_empty();
        if !has_background_imports {
            send_completed_import_notification(
                &self.outgoing,
                self.state_db.as_ref(),
                import_id,
                &completed_item_results,
            )
            .await;
            return Ok(());
        }

        let session_importer = self.session_importer.clone();
        let plugin_processor = self.clone();
        let outgoing = Arc::clone(&self.outgoing);
        let state_db = self.state_db.clone();
        let thread_manager = Arc::clone(&self.thread_manager);
        let session_import_result = (!pending_session_imports.is_empty()).then(|| {
            CoreImportItemResult::new(
                CoreMigrationItemType::Sessions,
                "Import sessions".to_string(),
                /*cwd*/ None,
            )
        });
        let pending_plugin_imports = import_outcome.pending_plugin_imports;
        tokio::spawn(async move {
            let session_progress_outgoing = Arc::clone(&outgoing);
            let session_import_id = import_id.clone();
            let session_imports = async move {
                let session_import_result = session_import_result?;
                let item_result = session_importer
                    .import_sessions(pending_session_imports, session_import_result)
                    .await;
                send_import_progress(&session_progress_outgoing, &session_import_id, &item_result)
                    .await;
                Some(item_result)
            };
            let plugin_progress_outgoing = Arc::clone(&outgoing);
            let plugin_import_id = import_id.clone();
            let plugin_imports = async move {
                let mut item_results = Vec::new();
                for pending_plugin_import in pending_plugin_imports {
                    let mut item_result = CoreImportItemResult::new(
                        CoreMigrationItemType::Plugins,
                        pending_plugin_import.description.clone(),
                        pending_plugin_import.cwd.clone(),
                    );
                    match plugin_processor
                        .complete_pending_plugin_import(pending_plugin_import)
                        .await
                    {
                        Ok(plugin_outcome) => {
                            apply_plugin_outcome_to_item_result(&mut item_result, plugin_outcome);
                        }
                        Err(error) => {
                            record_import_error(
                                &mut item_result,
                                "plugin_import",
                                error.message.clone(),
                                /*source*/ None,
                            );
                        }
                    }
                    send_import_progress(
                        &plugin_progress_outgoing,
                        &plugin_import_id,
                        &item_result,
                    )
                    .await;
                    item_results.push(item_result);
                }
                item_results
            };
            let (session_result, plugin_results) = tokio::join!(session_imports, plugin_imports);
            let mut background_item_results = Vec::new();
            if let Some(session_result) = session_result {
                background_item_results.push(session_result);
            }
            background_item_results.extend(plugin_results);
            completed_item_results.extend(background_item_results);
            if has_plugin_imports {
                thread_manager.plugins_manager().clear_cache();
                thread_manager.skills_manager().clear_cache();
            }
            send_completed_import_notification(
                &outgoing,
                state_db.as_ref(),
                import_id,
                &completed_item_results,
            )
            .await;
        });

        Ok(())
    }

    fn validate_pending_session_imports(
        &self,
        params: &ExternalAgentConfigImportParams,
    ) -> (Vec<CoreSessionMigration>, Option<CoreImportItemResult>) {
        let sessions = params
            .migration_items
            .iter()
            .filter(|item| {
                matches!(
                    item.item_type,
                    ExternalAgentConfigMigrationItemType::Sessions
                )
            })
            .filter_map(|item| item.details.as_ref())
            .flat_map(|details| details.sessions.clone())
            .map(|session| CoreSessionMigration {
                path: session.path,
                cwd: session.cwd,
                title: session.title,
            })
            .collect::<Vec<_>>();
        if sessions.is_empty() {
            return (Vec::new(), None);
        }
        let mut item_result = CoreImportItemResult::new(
            CoreMigrationItemType::Sessions,
            "Validate session imports".to_string(),
            /*cwd*/ None,
        );
        let mut selected_session_paths = HashSet::new();
        let mut selected_sessions = Vec::new();
        for session in sessions {
            let canonical_path = match self
                .migration_service
                .external_agent_session_source_path(&session.path)
            {
                Ok(Some(canonical_path)) => canonical_path,
                Ok(None) => {
                    record_import_error(
                        &mut item_result,
                        "session_missing",
                        format!(
                            "external agent session was not detected for import: {}",
                            session.path.display()
                        ),
                        Some(session.path.display().to_string()),
                    );
                    continue;
                }
                Err(err) => {
                    record_import_error(
                        &mut item_result,
                        "session_source_path",
                        err.to_string(),
                        Some(session.path.display().to_string()),
                    );
                    continue;
                }
            };
            if selected_session_paths.insert(canonical_path) {
                selected_sessions.push(session);
            }
        }
        (selected_sessions, Some(item_result))
    }

    async fn import_external_agent_config(
        &self,
        params: ExternalAgentConfigImportParams,
    ) -> Result<CoreImportOutcome, JSONRPCErrorError> {
        self.migration_service
            .import(
                params
                    .migration_items
                    .into_iter()
                    .filter(|migration_item| {
                        !matches!(
                            migration_item.item_type,
                            ExternalAgentConfigMigrationItemType::Sessions
                        )
                    })
                    .map(|migration_item| CoreMigrationItem {
                        item_type: match migration_item.item_type {
                            ExternalAgentConfigMigrationItemType::Config => {
                                CoreMigrationItemType::Config
                            }
                            ExternalAgentConfigMigrationItemType::Skills => {
                                CoreMigrationItemType::Skills
                            }
                            ExternalAgentConfigMigrationItemType::AgentsMd => {
                                CoreMigrationItemType::AgentsMd
                            }
                            ExternalAgentConfigMigrationItemType::Plugins => {
                                CoreMigrationItemType::Plugins
                            }
                            ExternalAgentConfigMigrationItemType::McpServerConfig => {
                                CoreMigrationItemType::McpServerConfig
                            }
                            ExternalAgentConfigMigrationItemType::Subagents => {
                                CoreMigrationItemType::Subagents
                            }
                            ExternalAgentConfigMigrationItemType::Hooks => {
                                CoreMigrationItemType::Hooks
                            }
                            ExternalAgentConfigMigrationItemType::Commands => {
                                CoreMigrationItemType::Commands
                            }
                            ExternalAgentConfigMigrationItemType::Sessions => {
                                CoreMigrationItemType::Sessions
                            }
                        },
                        description: migration_item.description,
                        cwd: migration_item.cwd,
                        details: migration_item.details.map(|details| {
                            crate::config::external_agent_config::MigrationDetails {
                                plugins: details
                                    .plugins
                                    .into_iter()
                                    .map(|plugin| {
                                        crate::config::external_agent_config::PluginsMigration {
                                            marketplace_name: plugin.marketplace_name,
                                            plugin_names: plugin.plugin_names,
                                        }
                                    })
                                    .collect(),
                                sessions: details
                                    .sessions
                                    .into_iter()
                                    .map(|session| CoreSessionMigration {
                                        path: session.path,
                                        cwd: session.cwd,
                                        title: session.title,
                                    })
                                    .collect(),
                                mcp_servers: details
                                    .mcp_servers
                                    .into_iter()
                                    .map(|mcp_server| CoreNamedMigration {
                                        name: mcp_server.name,
                                    })
                                    .collect(),
                                hooks: details
                                    .hooks
                                    .into_iter()
                                    .map(|hook| CoreNamedMigration { name: hook.name })
                                    .collect(),
                                subagents: details
                                    .subagents
                                    .into_iter()
                                    .map(|subagent| CoreNamedMigration {
                                        name: subagent.name,
                                    })
                                    .collect(),
                                commands: details
                                    .commands
                                    .into_iter()
                                    .map(|command| CoreNamedMigration { name: command.name })
                                    .collect(),
                            }
                        }),
                    })
                    .collect(),
            )
            .await
            .map_err(|err| internal_error(err.to_string()))
    }

    async fn complete_pending_plugin_import(
        &self,
        pending_plugin_import: PendingPluginImport,
    ) -> Result<PluginImportOutcome, JSONRPCErrorError> {
        self.migration_service
            .import_plugins(
                pending_plugin_import.cwd.as_deref(),
                Some(pending_plugin_import.details),
            )
            .await
            .map_err(|err| internal_error(err.to_string()))
    }
}

async fn send_import_progress(
    outgoing: &OutgoingMessageSender,
    import_id: &str,
    item_result: &CoreImportItemResult,
) {
    outgoing
        .send_server_notification(ServerNotification::ExternalAgentConfigImportProgress(
            ExternalAgentConfigImportProgressNotification {
                import_id: import_id.to_string(),
                item_type_results: vec![protocol_import_type_result(item_result)],
            },
        ))
        .await;
}

async fn send_completed_import_notification(
    outgoing: &OutgoingMessageSender,
    state_db: Option<&StateDbHandle>,
    import_id: String,
    item_results: &[CoreImportItemResult],
) {
    let notification = completed_notification(import_id, item_results);
    if let Some(state_db) = state_db
        && let Err(err) = record_completed_import_notification(state_db, &notification).await
    {
        tracing::warn!(
            import_id = %notification.import_id,
            error = %err,
            "failed to record external agent config import completion"
        );
    }
    outgoing
        .send_server_notification(ServerNotification::ExternalAgentConfigImportCompleted(
            notification,
        ))
        .await;
}

async fn record_completed_import_notification(
    state_db: &StateDbHandle,
    notification: &ExternalAgentConfigImportCompletedNotification,
) -> anyhow::Result<()> {
    let successes = notification
        .item_type_results
        .iter()
        .flat_map(|type_result| type_result.successes.iter())
        .map(|success| {
            Ok(ExternalAgentConfigImportSuccessRecord {
                item_type: serde_json::to_value(success.item_type)?
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                cwd: success.cwd.clone(),
                source: success.source.clone(),
                target: success.target.clone(),
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let failures = notification
        .item_type_results
        .iter()
        .flat_map(|type_result| type_result.failures.iter())
        .map(|failure| {
            Ok(ExternalAgentConfigImportFailureRecord {
                item_type: serde_json::to_value(failure.item_type)?
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                failure_stage: failure.failure_stage.clone(),
                message: failure.message.clone(),
                cwd: failure.cwd.clone(),
                source: failure.source.clone(),
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    state_db
        .record_external_agent_config_import_completed(
            notification.import_id.as_str(),
            &successes,
            &failures,
        )
        .await
}

fn completed_notification(
    import_id: String,
    item_results: &[CoreImportItemResult],
) -> ExternalAgentConfigImportCompletedNotification {
    let mut protocol_type_results: Vec<ProtocolImportTypeResult> = Vec::new();
    for item_result in item_results {
        let item_raw_errors = item_result
            .raw_errors
            .iter()
            .map(protocol_import_raw_error)
            .collect::<Vec<_>>();
        let item_successes = item_result
            .successes
            .iter()
            .map(protocol_import_success)
            .collect::<Vec<_>>();
        let item_type = protocol_migration_item_type(item_result.item_type);
        if let Some(type_result) = protocol_type_results
            .iter_mut()
            .find(|type_result| type_result.item_type == item_type)
        {
            type_result.successes.extend(item_successes);
            type_result.failures.extend(item_raw_errors);
        } else {
            protocol_type_results.push(ProtocolImportTypeResult {
                item_type,
                successes: item_successes,
                failures: item_raw_errors,
            });
        }
    }
    protocol_type_results.sort_by_key(|type_result| match type_result.item_type {
        ExternalAgentConfigMigrationItemType::Config => 0,
        ExternalAgentConfigMigrationItemType::Skills => 1,
        ExternalAgentConfigMigrationItemType::AgentsMd => 2,
        ExternalAgentConfigMigrationItemType::Plugins => 3,
        ExternalAgentConfigMigrationItemType::McpServerConfig => 4,
        ExternalAgentConfigMigrationItemType::Subagents => 5,
        ExternalAgentConfigMigrationItemType::Hooks => 6,
        ExternalAgentConfigMigrationItemType::Commands => 7,
        ExternalAgentConfigMigrationItemType::Sessions => 8,
    });

    ExternalAgentConfigImportCompletedNotification {
        import_id,
        item_type_results: protocol_type_results,
    }
}

fn protocol_import_type_result(item_result: &CoreImportItemResult) -> ProtocolImportTypeResult {
    ProtocolImportTypeResult {
        item_type: protocol_migration_item_type(item_result.item_type),
        successes: item_result
            .successes
            .iter()
            .map(protocol_import_success)
            .collect(),
        failures: item_result
            .raw_errors
            .iter()
            .map(protocol_import_raw_error)
            .collect(),
    }
}

fn protocol_import_success(
    success: &crate::config::external_agent_config::ExternalAgentConfigImportSuccess,
) -> ProtocolImportSuccess {
    ProtocolImportSuccess {
        item_type: protocol_migration_item_type(success.item_type),
        cwd: success.cwd.clone(),
        source: success.source.clone(),
        target: success.target.clone(),
    }
}

fn protocol_import_raw_error(raw_error: &CoreImportRawError) -> ProtocolImportFailure {
    ProtocolImportFailure {
        item_type: protocol_migration_item_type(raw_error.item_type),
        failure_stage: raw_error.failure_stage.clone(),
        message: raw_error.message.clone(),
        cwd: raw_error.cwd.clone(),
        source: raw_error.source.clone(),
    }
}

fn protocol_migration_item_type(
    item_type: CoreMigrationItemType,
) -> ExternalAgentConfigMigrationItemType {
    match item_type {
        CoreMigrationItemType::Config => ExternalAgentConfigMigrationItemType::Config,
        CoreMigrationItemType::Skills => ExternalAgentConfigMigrationItemType::Skills,
        CoreMigrationItemType::AgentsMd => ExternalAgentConfigMigrationItemType::AgentsMd,
        CoreMigrationItemType::Plugins => ExternalAgentConfigMigrationItemType::Plugins,
        CoreMigrationItemType::McpServerConfig => {
            ExternalAgentConfigMigrationItemType::McpServerConfig
        }
        CoreMigrationItemType::Subagents => ExternalAgentConfigMigrationItemType::Subagents,
        CoreMigrationItemType::Hooks => ExternalAgentConfigMigrationItemType::Hooks,
        CoreMigrationItemType::Commands => ExternalAgentConfigMigrationItemType::Commands,
        CoreMigrationItemType::Sessions => ExternalAgentConfigMigrationItemType::Sessions,
    }
}

fn apply_plugin_outcome_to_item_result(
    item_result: &mut CoreImportItemResult,
    plugin_outcome: PluginImportOutcome,
) {
    for plugin_id in plugin_outcome.succeeded_plugin_ids {
        item_result.record_success(Some(plugin_id.clone()), Some(plugin_id));
    }
    for raw_error in plugin_outcome.raw_errors {
        item_result.record_error(raw_error);
    }
}

fn migration_items_need_runtime_refresh(items: &[ExternalAgentConfigMigrationItem]) -> bool {
    items.iter().any(|item| {
        matches!(
            item.item_type,
            ExternalAgentConfigMigrationItemType::Config
                | ExternalAgentConfigMigrationItemType::Skills
                | ExternalAgentConfigMigrationItemType::McpServerConfig
                | ExternalAgentConfigMigrationItemType::Hooks
                | ExternalAgentConfigMigrationItemType::Commands
                | ExternalAgentConfigMigrationItemType::Plugins
        )
    })
}

#[cfg(test)]
#[path = "external_agent_config_processor_tests.rs"]
mod external_agent_config_processor_tests;
