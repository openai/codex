use std::sync::Arc;

use crate::config::external_agent_config::ExternalAgentConfigDetectOptions;
use crate::config::external_agent_config::ExternalAgentConfigMigrationItem as CoreMigrationItem;
use crate::config::external_agent_config::ExternalAgentConfigMigrationItemType as CoreMigrationItemType;
use crate::config::external_agent_config::ExternalAgentConfigService;
use crate::config::external_agent_config::NamedMigration as CoreNamedMigration;
use crate::config::external_agent_config::PendingPluginImport;
use crate::config_manager::ConfigManager;
use crate::error_code::internal_error;
use crate::error_code::invalid_params;
use crate::outgoing_message::ConnectionRequestId;
use crate::outgoing_message::OutgoingMessageSender;
use codex_app_server_protocol::CommandMigration;
use codex_app_server_protocol::ExternalAgentConfigDetectParams;
use codex_app_server_protocol::ExternalAgentConfigDetectResponse;
use codex_app_server_protocol::ExternalAgentConfigImportCompletedNotification;
use codex_app_server_protocol::ExternalAgentConfigImportParams;
use codex_app_server_protocol::ExternalAgentConfigImportResponse;
use codex_app_server_protocol::ExternalAgentConfigMigrationItem;
use codex_app_server_protocol::ExternalAgentConfigMigrationItemType;
use codex_app_server_protocol::HookMigration;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::McpServerMigration;
use codex_app_server_protocol::MigrationDetails;
use codex_app_server_protocol::PluginsMigration;
use codex_app_server_protocol::ServerNotification;
use codex_arg0::Arg0DispatchPaths;
use codex_core::StartThreadOptions;
use codex_core::ThreadManager;
use codex_core::config::ConfigOverrides;
use codex_external_agent_sessions::ExternalAgentSessionMigration as CoreSessionMigration;
use codex_external_agent_sessions::ImportedExternalAgentSession;
use codex_external_agent_sessions::PendingSessionImport;
use codex_external_agent_sessions::prepare_validated_session_imports;
use codex_external_agent_sessions::record_imported_session;
use codex_protocol::ThreadId;
use codex_protocol::protocol::InitialHistory;
use codex_thread_store::ThreadMetadataPatch;
use std::collections::HashSet;
use std::path::PathBuf;
use tokio::sync::Semaphore;

use super::ConfigRequestProcessor;

#[derive(Clone)]
pub(crate) struct ExternalAgentConfigRequestProcessor {
    outgoing: Arc<OutgoingMessageSender>,
    codex_home: PathBuf,
    migration_service: ExternalAgentConfigService,
    session_import_permits: Arc<Semaphore>,
    thread_manager: Arc<ThreadManager>,
    config_manager: ConfigManager,
    config_processor: ConfigRequestProcessor,
    arg0_paths: Arg0DispatchPaths,
}

impl ExternalAgentConfigRequestProcessor {
    pub(crate) fn new(
        outgoing: Arc<OutgoingMessageSender>,
        thread_manager: Arc<ThreadManager>,
        config_manager: ConfigManager,
        config_processor: ConfigRequestProcessor,
        arg0_paths: Arg0DispatchPaths,
        codex_home: PathBuf,
    ) -> Self {
        Self {
            outgoing,
            migration_service: ExternalAgentConfigService::new(codex_home.clone()),
            codex_home,
            session_import_permits: Arc::new(Semaphore::new(1)),
            thread_manager,
            config_manager,
            config_processor,
            arg0_paths,
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
        let needs_runtime_refresh = migration_items_need_runtime_refresh(&params.migration_items);
        let has_migration_items = !params.migration_items.is_empty();
        let has_plugin_imports = params.migration_items.iter().any(|item| {
            matches!(
                item.item_type,
                ExternalAgentConfigMigrationItemType::Plugins
            )
        });
        let pending_session_imports = self.validate_pending_session_imports(&params)?;
        let pending_plugin_imports = self.import_external_agent_config(params).await?;
        if needs_runtime_refresh {
            self.config_processor.handle_config_mutation().await;
        }
        self.outgoing
            .send_response(request_id, ExternalAgentConfigImportResponse {})
            .await;

        if !has_migration_items {
            return Ok(());
        }

        let has_background_imports =
            !pending_plugin_imports.is_empty() || !pending_session_imports.is_empty();
        if !has_background_imports {
            self.outgoing
                .send_server_notification(ServerNotification::ExternalAgentConfigImportCompleted(
                    ExternalAgentConfigImportCompletedNotification {},
                ))
                .await;
            return Ok(());
        }

        let session_import_permits = Arc::clone(&self.session_import_permits);
        let session_processor = self.clone();
        let plugin_processor = self.clone();
        let outgoing = Arc::clone(&self.outgoing);
        let thread_manager = Arc::clone(&self.thread_manager);
        tokio::spawn(async move {
            let session_imports = async move {
                if !pending_session_imports.is_empty() {
                    let Ok(_session_import_permit) = session_import_permits.acquire_owned().await
                    else {
                        return;
                    };
                    let pending_session_imports = session_processor
                        .prepare_validated_session_imports(pending_session_imports);
                    for pending_session_import in pending_session_imports {
                        match session_processor
                            .import_external_agent_session(pending_session_import.session)
                            .await
                        {
                            Ok(imported_thread_id) => {
                                session_processor.record_imported_session(
                                    &pending_session_import.source_path,
                                    imported_thread_id,
                                );
                            }
                            Err(error) => {
                                tracing::warn!(
                                    error = %error.message,
                                    path = %pending_session_import.source_path.display(),
                                    "external agent session import failed"
                                );
                            }
                        }
                    }
                }
            };
            let plugin_imports = async move {
                for pending_plugin_import in pending_plugin_imports {
                    match plugin_processor
                        .complete_pending_plugin_import(pending_plugin_import)
                        .await
                    {
                        Ok(()) => {}
                        Err(error) => {
                            tracing::warn!(
                                error = %error.message,
                                "external agent config plugin import failed"
                            );
                        }
                    }
                }
            };
            tokio::join!(session_imports, plugin_imports);
            if has_plugin_imports {
                thread_manager.plugins_manager().clear_cache();
                thread_manager.skills_manager().clear_cache();
            }
            outgoing
                .send_server_notification(ServerNotification::ExternalAgentConfigImportCompleted(
                    ExternalAgentConfigImportCompletedNotification {},
                ))
                .await;
        });

        Ok(())
    }

    async fn import_external_agent_session(
        &self,
        session: ImportedExternalAgentSession,
    ) -> Result<ThreadId, JSONRPCErrorError> {
        let ImportedExternalAgentSession {
            cwd,
            title,
            rollout_items,
        } = session;
        let config = self
            .config_manager
            .load_with_overrides(
                /*request_overrides*/ None,
                ConfigOverrides {
                    cwd: Some(PathBuf::from(cwd.to_string_lossy().into_owned())),
                    codex_linux_sandbox_exe: self.arg0_paths.codex_linux_sandbox_exe.clone(),
                    main_execve_wrapper_exe: self.arg0_paths.main_execve_wrapper_exe.clone(),
                    ..Default::default()
                },
            )
            .await
            .map_err(|err| {
                internal_error(format!("failed to load imported session config: {err}"))
            })?;
        let environments = self
            .thread_manager
            .default_environment_selections(&config.cwd);
        let imported_thread = self
            .thread_manager
            .start_thread_with_options(StartThreadOptions {
                config,
                initial_history: InitialHistory::Forked(rollout_items),
                session_source: None,
                thread_source: None,
                dynamic_tools: Vec::new(),
                persist_extended_history: false,
                metrics_service_name: None,
                parent_trace: None,
                environments,
            })
            .await
            .map_err(|err| internal_error(format!("failed to import session: {err}")))?;
        if let Some(title) = title
            && let Some(name) = codex_core::util::normalize_thread_name(&title)
        {
            imported_thread
                .thread
                .update_thread_metadata(
                    ThreadMetadataPatch {
                        name: Some(name),
                        ..Default::default()
                    },
                    /*include_archived*/ false,
                )
                .await
                .map_err(|err| internal_error(format!("failed to name imported session: {err}")))?;
        }
        Ok(imported_thread.thread_id)
    }

    fn validate_pending_session_imports(
        &self,
        params: &ExternalAgentConfigImportParams,
    ) -> Result<Vec<CoreSessionMigration>, JSONRPCErrorError> {
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
        let mut selected_session_paths = HashSet::new();
        let mut selected_sessions = Vec::new();
        for session in sessions {
            let Some(canonical_path) = self
                .migration_service
                .external_agent_session_source_path(&session.path)
                .map_err(|err| internal_error(err.to_string()))?
            else {
                return Err(session_not_detected_error(&session.path));
            };
            if selected_session_paths.insert(canonical_path) {
                selected_sessions.push(session);
            }
        }
        Ok(selected_sessions)
    }

    fn prepare_validated_session_imports(
        &self,
        sessions: Vec<CoreSessionMigration>,
    ) -> Vec<PendingSessionImport> {
        prepare_validated_session_imports(&self.codex_home, sessions)
    }

    fn record_imported_session(&self, source_path: &std::path::Path, imported_thread_id: ThreadId) {
        if let Err(err) = record_imported_session(&self.codex_home, source_path, imported_thread_id)
        {
            tracing::warn!(
                error = %err,
                path = %source_path.display(),
                "external agent session import ledger update failed"
            );
        }
    }

    async fn import_external_agent_config(
        &self,
        params: ExternalAgentConfigImportParams,
    ) -> Result<Vec<PendingPluginImport>, JSONRPCErrorError> {
        self.migration_service
            .import(
                params
                    .migration_items
                    .into_iter()
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
    ) -> Result<(), JSONRPCErrorError> {
        self.migration_service
            .import_plugins(
                pending_plugin_import.cwd.as_deref(),
                Some(pending_plugin_import.details),
            )
            .await
            .map(|_| ())
            .map_err(|err| internal_error(err.to_string()))
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

fn session_not_detected_error(path: &std::path::Path) -> JSONRPCErrorError {
    invalid_params(format!(
        "external agent session was not detected for import: {}",
        path.display()
    ))
}

#[cfg(test)]
#[path = "external_agent_config_processor_tests.rs"]
mod external_agent_config_processor_tests;
