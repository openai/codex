use crate::config::external_agent_config::ExternalAgentConfigDetectOptions;
use crate::config::external_agent_config::ExternalAgentConfigMigrationItem as CoreMigrationItem;
use crate::config::external_agent_config::ExternalAgentConfigMigrationItemType as CoreMigrationItemType;
use crate::config::external_agent_config::ExternalAgentConfigService;
use crate::config::external_agent_config::PendingPluginImport;
use crate::error_code::internal_error;
use crate::error_code::invalid_params;
use crate::external_agent_sessions::ExternalAgentSessionMigration as CoreSessionMigration;
use crate::external_agent_sessions::ImportedExternalAgentSession;
use crate::external_agent_sessions::has_current_session_been_imported;
use crate::external_agent_sessions::load_session_for_import;
use crate::external_agent_sessions::record_imported_session;
use codex_app_server_protocol::ExternalAgentConfigDetectParams;
use codex_app_server_protocol::ExternalAgentConfigDetectResponse;
use codex_app_server_protocol::ExternalAgentConfigImportParams;
use codex_app_server_protocol::ExternalAgentConfigMigrationItem;
use codex_app_server_protocol::ExternalAgentConfigMigrationItemType;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::MigrationDetails;
use codex_app_server_protocol::PluginsMigration;
use codex_protocol::ThreadId;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Clone)]
pub(crate) struct ExternalAgentConfigApi {
    codex_home: PathBuf,
    migration_service: ExternalAgentConfigService,
}

pub(crate) struct PendingSessionImport {
    pub(crate) source_path: PathBuf,
    pub(crate) session: ImportedExternalAgentSession,
}

impl ExternalAgentConfigApi {
    pub(crate) fn new(codex_home: PathBuf) -> Self {
        Self {
            migration_service: ExternalAgentConfigService::new(codex_home.clone()),
            codex_home,
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
                    }),
                })
                .collect(),
        })
    }

    pub(crate) fn detect_recent_sessions(
        &self,
    ) -> Result<Vec<CoreSessionMigration>, JSONRPCErrorError> {
        self.migration_service
            .detect_recent_sessions()
            .map_err(|err| internal_error(err.to_string()))
    }

    pub(crate) fn prepare_pending_session_imports(
        &self,
        params: &ExternalAgentConfigImportParams,
    ) -> Result<Vec<PendingSessionImport>, JSONRPCErrorError> {
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
            .collect::<Vec<_>>();
        let detected_session_paths = if sessions.is_empty() {
            HashSet::new()
        } else {
            self.detect_recent_sessions()?
                .into_iter()
                .map(|session| session.path)
                .collect::<HashSet<_>>()
        };

        let mut pending_session_imports = Vec::new();
        for session in sessions {
            let has_been_imported =
                match has_current_session_been_imported(&self.codex_home, &session.path) {
                    Ok(has_been_imported) => has_been_imported,
                    Err(err) => {
                        tracing::warn!(
                            error = %err,
                            path = %session.path.display(),
                            "external agent session import ledger check failed"
                        );
                        continue;
                    }
                };
            if !detected_session_paths.contains(&session.path) && !has_been_imported {
                return Err(invalid_params(format!(
                    "external agent session was not detected for import: {}",
                    session.path.display()
                )));
            }
            if has_been_imported {
                continue;
            }
            let imported_session = match load_session_for_import(&session.path) {
                Ok(Some(imported_session)) if imported_session.cwd.is_dir() => imported_session,
                Ok(Some(_)) | Ok(None) => continue,
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        path = %session.path.display(),
                        "external agent session import skipped"
                    );
                    continue;
                }
            };
            pending_session_imports.push(PendingSessionImport {
                source_path: session.path,
                session: imported_session,
            });
        }
        Ok(pending_session_imports)
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
