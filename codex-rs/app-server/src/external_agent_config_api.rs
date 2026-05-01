use crate::config::external_agent_config::ExternalAgentConfigDetectOptions;
use crate::config::external_agent_config::ExternalAgentConfigMigrationItem as CoreMigrationItem;
use crate::config::external_agent_config::ExternalAgentConfigMigrationItemType as CoreMigrationItemType;
use crate::config::external_agent_config::ExternalAgentConfigService;
use crate::config::external_agent_config::NamedMigration as CoreNamedMigration;
use crate::config::external_agent_config::PendingPluginImport;
use crate::error_code::internal_error;
use crate::error_code::invalid_params;
use codex_app_server_protocol::CommandMigration;
use codex_app_server_protocol::ExternalAgentConfigDetectParams;
use codex_app_server_protocol::ExternalAgentConfigDetectResponse;
use codex_app_server_protocol::ExternalAgentConfigImportParams;
use codex_app_server_protocol::ExternalAgentConfigMigrationItem;
use codex_app_server_protocol::ExternalAgentConfigMigrationItemType;
use codex_app_server_protocol::HookMigration;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::McpServerMigration;
use codex_app_server_protocol::MigrationDetails;
use codex_app_server_protocol::PluginsMigration;
use codex_app_server_protocol::SubagentMigration;
use codex_external_agent_sessions::ExternalAgentSessionMigration as CoreSessionMigration;
use codex_external_agent_sessions::PendingSessionImport;
use codex_external_agent_sessions::prepare_validated_session_imports;
use codex_external_agent_sessions::record_imported_session;
use codex_protocol::ThreadId;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Semaphore;

#[derive(Clone)]
pub(crate) struct ExternalAgentConfigApi {
    codex_home: PathBuf,
    migration_service: ExternalAgentConfigService,
    session_import_permits: Arc<Semaphore>,
}

impl ExternalAgentConfigApi {
    pub(crate) fn new(codex_home: PathBuf) -> Self {
        Self {
            migration_service: ExternalAgentConfigService::new(codex_home.clone()),
            codex_home,
            session_import_permits: Arc::new(Semaphore::new(1)),
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
                            .map(|subagent| SubagentMigration {
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

    pub(crate) fn validate_pending_session_imports(
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

    pub(crate) fn prepare_validated_session_imports(
        &self,
        sessions: Vec<CoreSessionMigration>,
    ) -> Vec<PendingSessionImport> {
        prepare_validated_session_imports(&self.codex_home, sessions)
    }

    pub(crate) fn session_import_permits(&self) -> Arc<Semaphore> {
        Arc::clone(&self.session_import_permits)
    }

    pub(crate) fn record_imported_session(
        &self,
        source_path: &std::path::Path,
        imported_thread_id: ThreadId,
    ) {
        if let Err(err) = record_imported_session(&self.codex_home, source_path, imported_thread_id)
        {
            tracing::warn!(
                error = %err,
                path = %source_path.display(),
                "external agent session import ledger update failed"
            );
        }
    }

    pub(crate) async fn import(
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

    pub(crate) async fn complete_pending_plugin_import(
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

fn session_not_detected_error(path: &std::path::Path) -> JSONRPCErrorError {
    invalid_params(format!(
        "external agent session was not detected for import: {}",
        path.display()
    ))
}
