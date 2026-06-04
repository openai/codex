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
use chrono::Utc;
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
use codex_core::ThreadManager;
use codex_core::config::ConfigOverrides;
use codex_external_agent_sessions::CompletedExternalAgentSessionImport;
use codex_external_agent_sessions::ExternalAgentSessionMigration as CoreSessionMigration;
use codex_external_agent_sessions::ImportedExternalAgentSession;
use codex_external_agent_sessions::PendingSessionImport;
use codex_external_agent_sessions::prepare_validated_session_import;
use codex_external_agent_sessions::record_completed_session_imports;
use codex_models_manager::manager::RefreshStrategy;
use codex_protocol::ThreadId;
use codex_protocol::models::BaseInstructions;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_rollout::persisted_rollout_items;
use codex_thread_store::AppendThreadItemsParams;
use codex_thread_store::CreateThreadParams;
use codex_thread_store::ThreadMetadataPatch;
use codex_thread_store::ThreadPersistenceMetadata;
use codex_thread_store::ThreadStore;
use codex_thread_store::UpdateThreadMetadataParams;
use futures::StreamExt;
use std::collections::HashSet;
use std::path::PathBuf;
use tokio::sync::Semaphore;

use super::ConfigRequestProcessor;

const SESSION_IMPORT_CONCURRENCY: usize = 5;

#[derive(Clone)]
pub(crate) struct ExternalAgentConfigRequestProcessor {
    outgoing: Arc<OutgoingMessageSender>,
    codex_home: PathBuf,
    migration_service: ExternalAgentConfigService,
    session_import_permits: Arc<Semaphore>,
    thread_manager: Arc<ThreadManager>,
    thread_store: Arc<dyn ThreadStore>,
    config_manager: ConfigManager,
    config_processor: ConfigRequestProcessor,
    arg0_paths: Arg0DispatchPaths,
}

impl ExternalAgentConfigRequestProcessor {
    pub(crate) fn new(
        outgoing: Arc<OutgoingMessageSender>,
        thread_manager: Arc<ThreadManager>,
        thread_store: Arc<dyn ThreadStore>,
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
            thread_store,
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
                    let import_results = futures::stream::iter(pending_session_imports)
                        .map(|session| {
                            let session_processor = session_processor.clone();
                            async move {
                                let pending_session_import = session_processor
                                    .prepare_validated_session_import(session)
                                    .await;
                                let pending_session_import = pending_session_import?;
                                let result = session_processor
                                    .import_external_agent_session(pending_session_import.session)
                                    .await;
                                Some((
                                    pending_session_import.source_path,
                                    pending_session_import.source_content_sha256,
                                    result,
                                ))
                            }
                        })
                        .buffer_unordered(SESSION_IMPORT_CONCURRENCY)
                        .filter_map(|result| async move { result });
                    futures::pin_mut!(import_results);
                    let mut completed_imports = Vec::new();
                    while let Some((source_path, source_content_sha256, result)) =
                        import_results.next().await
                    {
                        match result {
                            Ok(imported_thread_id) => {
                                completed_imports.push(CompletedExternalAgentSessionImport {
                                    source_path,
                                    source_content_sha256,
                                    imported_thread_id,
                                });
                            }
                            Err(error) => {
                                tracing::warn!(
                                    error = %error.message,
                                    path = %source_path.display(),
                                    "external agent session import failed"
                                );
                            }
                        }
                    }
                    session_processor.record_completed_session_imports(completed_imports);
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
            first_user_message,
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
        let models_manager = self.thread_manager.get_models_manager();
        let model = models_manager
            .get_default_model(&config.model, RefreshStrategy::Offline)
            .await;
        let model_info = models_manager
            .get_model_info(model.as_str(), &config.to_models_manager_config())
            .await;
        let thread_id = ThreadId::new();
        let source = self.thread_manager.session_source();
        let cwd = config.cwd.to_path_buf();
        let model_provider = config.model_provider_id.clone();
        let memory_mode = if config.memories.generate_memories {
            ThreadMemoryMode::Enabled
        } else {
            ThreadMemoryMode::Disabled
        };
        let now = Utc::now();
        let create_params = CreateThreadParams {
            thread_id,
            forked_from_id: None,
            parent_thread_id: None,
            source: source.clone(),
            thread_source: None,
            base_instructions: BaseInstructions {
                text: config
                    .base_instructions
                    .clone()
                    .unwrap_or_else(|| model_info.get_model_instructions(config.personality)),
            },
            dynamic_tools: Vec::new(),
            multi_agent_version: Some(MultiAgentVersion::V1),
            metadata: ThreadPersistenceMetadata {
                cwd: Some(cwd.clone()),
                model_provider: model_provider.clone(),
                memory_mode,
            },
        };
        let canonical_items = persisted_rollout_items(&rollout_items);
        let title = title
            .as_deref()
            .and_then(codex_core::util::normalize_thread_name);
        let metadata = ThreadMetadataPatch {
            title,
            preview: first_user_message.clone(),
            model_provider: Some(model_provider),
            created_at: Some(now),
            updated_at: Some(now),
            source: Some(source.clone()),
            thread_source: Some(None),
            agent_nickname: Some(source.get_nickname()),
            agent_role: Some(source.get_agent_role()),
            agent_path: Some(source.get_agent_path().map(Into::into)),
            cwd: Some(cwd),
            cli_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            first_user_message,
            memory_mode: Some(memory_mode),
            ..Default::default()
        };

        self.thread_store
            .create_thread(create_params)
            .await
            .map_err(|err| internal_error(format!("failed to import session: {err}")))?;
        if !canonical_items.is_empty()
            && let Err(err) = self
                .thread_store
                .append_items(AppendThreadItemsParams {
                    thread_id,
                    items: canonical_items,
                })
                .await
        {
            let _ = self.thread_store.discard_thread(thread_id).await;
            return Err(internal_error(format!("failed to import session: {err}")));
        }

        self.thread_store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: metadata,
                include_archived: false,
            })
            .await
            .map_err(|err| internal_error(format!("failed to update imported session: {err}")))?;
        self.thread_store
            .persist_thread(thread_id)
            .await
            .map_err(|err| internal_error(format!("failed to persist imported session: {err}")))?;
        self.thread_store
            .shutdown_thread(thread_id)
            .await
            .map_err(|err| internal_error(format!("failed to shutdown imported session: {err}")))?;
        Ok(thread_id)
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

    async fn prepare_validated_session_import(
        &self,
        session: CoreSessionMigration,
    ) -> Option<PendingSessionImport> {
        let codex_home = self.codex_home.clone();
        tokio::task::spawn_blocking(move || prepare_validated_session_import(&codex_home, session))
            .await
            .ok()
            .flatten()
    }

    fn record_completed_session_imports(
        &self,
        completed_imports: Vec<CompletedExternalAgentSessionImport>,
    ) {
        if let Err(err) = record_completed_session_imports(&self.codex_home, completed_imports) {
            tracing::warn!(
                error = %err,
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
