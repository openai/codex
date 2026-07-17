use std::sync::Arc;

use crate::config_manager::ConfigManager;
use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use crate::outgoing_message::ConnectionRequestId;
use crate::outgoing_message::OutgoingMessageSender;
use crate::request_processors::ConfigRequestProcessor;
use codex_analytics::AnalyticsEventsClient;
use codex_analytics::ExternalAgentConfigImportCompletedInput;
use codex_analytics::ExternalAgentConfigImportFailureInput;
use codex_app_server_protocol::ExternalAgentConfigDetectParams;
use codex_app_server_protocol::ExternalAgentConfigDetectResponse;
use codex_app_server_protocol::ExternalAgentConfigImportCompletedNotification;
use codex_app_server_protocol::ExternalAgentConfigImportHistoriesReadResponse;
use codex_app_server_protocol::ExternalAgentConfigImportItemTypeFailure as ProtocolImportFailure;
use codex_app_server_protocol::ExternalAgentConfigImportParams;
use codex_app_server_protocol::ExternalAgentConfigImportProgressNotification;
use codex_app_server_protocol::ExternalAgentConfigImportResponse;
use codex_app_server_protocol::ExternalAgentConfigMigrationItem;
use codex_app_server_protocol::ExternalAgentConfigMigrationItemType;
use codex_app_server_protocol::ExternalAgentImportedConnectorCandidate;
use codex_app_server_protocol::ExternalAgentImportedConnectorSource;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ServerNotification;
use codex_arg0::Arg0DispatchPaths;
use codex_core::ThreadManager;
use codex_external_agent_migration::ExternalAgentConfigDetectOptions;
use codex_external_agent_migration::ExternalAgentConfigImportItemResult as CoreImportItemResult;
use codex_external_agent_migration::ExternalAgentConfigImportOutcome as CoreImportOutcome;
use codex_external_agent_migration::ExternalAgentConfigMigrationItemType as CoreMigrationItemType;
use codex_external_agent_migration::ExternalAgentConfigService;
use codex_external_agent_migration::PluginImportOutcome;
use codex_external_agent_migration::record_import_error;
use codex_external_agent_migration::sessions::ExternalAgentSessionMigration as CoreSessionMigration;
use codex_external_agent_migration::sessions::read_imported_connector_candidates;
use codex_features::Feature;
use codex_rollout::StateDbHandle;
use codex_state::ExternalAgentConfigImportFailureRecord;
use codex_state::ExternalAgentConfigImportSuccessRecord;
use codex_thread_store::ThreadStore;
use std::collections::HashSet;
use std::path::PathBuf;

use super::protocol::completed_notification;
use super::protocol::core_migration_items;
use super::protocol::detect_response;
use super::protocol::protocol_import_history;
use super::protocol::protocol_import_type_result;
use super::session_importer::ExternalAgentSessionImporter;
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct ExternalAgentConfigRequestProcessor {
    outgoing: Arc<OutgoingMessageSender>,
    migration_service: ExternalAgentConfigService,
    session_importer: ExternalAgentSessionImporter,
    thread_manager: Arc<ThreadManager>,
    config_manager: ConfigManager,
    config_processor: ConfigRequestProcessor,
    state_db: Option<StateDbHandle>,
    analytics_events_client: AnalyticsEventsClient,
}

pub(crate) struct ExternalAgentConfigRequestProcessorArgs {
    pub(crate) outgoing: Arc<OutgoingMessageSender>,
    pub(crate) thread_manager: Arc<ThreadManager>,
    pub(crate) thread_store: Arc<dyn ThreadStore>,
    pub(crate) config_manager: ConfigManager,
    pub(crate) config_processor: ConfigRequestProcessor,
    pub(crate) state_db: Option<StateDbHandle>,
    pub(crate) analytics_events_client: AnalyticsEventsClient,
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
            analytics_events_client,
            arg0_paths,
            codex_home,
        } = args;
        let migration_service = ExternalAgentConfigService::new(
            codex_home.clone(),
            analytics_events_client.clone(),
            state_db.clone(),
        );
        let session_importer = ExternalAgentSessionImporter::new(
            codex_home,
            migration_service.connector_metadata_roots().to_vec(),
            Arc::clone(&thread_manager),
            thread_store,
            config_manager.clone(),
            arg0_paths,
        );
        Self {
            outgoing,
            migration_service,
            session_importer,
            thread_manager,
            config_manager,
            config_processor,
            state_db,
            analytics_events_client,
        }
    }

    pub(crate) async fn detect(
        &self,
        params: ExternalAgentConfigDetectParams,
    ) -> Result<ExternalAgentConfigDetectResponse, JSONRPCErrorError> {
        let migration_service = self
            .migration_service
            .with_migration_source(params.migration_source.as_deref());
        let options = ExternalAgentConfigDetectOptions {
            include_home: params.include_home,
            include_memory: self.external_agent_memory_import_enabled().await,
            cwds: params.cwds,
        };
        let items = migration_service
            .detect(options)
            .await
            .map_err(|err| internal_error(err.to_string()))?;

        Ok(detect_response(items))
    }

    pub(crate) async fn import(
        &self,
        request_id: ConnectionRequestId,
        params: ExternalAgentConfigImportParams,
    ) -> Result<(), JSONRPCErrorError> {
        if params
            .migration_items
            .iter()
            .any(|item| item.item_type == ExternalAgentConfigMigrationItemType::Memory)
            && !self.external_agent_memory_import_enabled().await
        {
            return Err(invalid_request("external agent memory import is disabled"));
        }
        if params.migration_items.iter().any(|item| {
            item.item_type == ExternalAgentConfigMigrationItemType::Memory
                && item
                    .details
                    .as_ref()
                    .is_none_or(|details| details.memory.is_empty())
        }) {
            return Err(invalid_request(
                "memory import requires at least one selected memory",
            ));
        }
        let import_id = Uuid::new_v4().to_string();
        let analytics_source = params.source.clone().unwrap_or_default();
        let migration_service = self
            .migration_service
            .with_migration_source(params.migration_source.as_deref());
        let needs_runtime_refresh = migration_items_need_runtime_refresh(&params.migration_items);
        let has_migration_items = !params.migration_items.is_empty();
        let has_plugin_imports = params.migration_items.iter().any(|item| {
            matches!(
                item.item_type,
                ExternalAgentConfigMigrationItemType::Plugins
            )
        });
        let (pending_session_imports, session_validation_result) =
            self.validate_pending_session_imports(&params, &migration_service);
        let import_outcome = self
            .import_external_agent_config(params, &migration_service)
            .await;
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
                &self.analytics_events_client,
                import_id,
                analytics_source,
                &completed_item_results,
            )
            .await;
            return Ok(());
        }

        let session_importer = self.session_importer.clone();
        let outgoing = Arc::clone(&self.outgoing);
        let state_db = self.state_db.clone();
        let analytics_events_client = self.analytics_events_client.clone();
        let thread_manager = Arc::clone(&self.thread_manager);
        let session_metadata_mode = migration_service.session_metadata_mode();
        let plugin_migration_service = migration_service;
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
                    .import_sessions(
                        pending_session_imports,
                        session_import_result,
                        session_metadata_mode,
                    )
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
                    match plugin_migration_service
                        .import_plugins(
                            pending_plugin_import.cwd.as_deref(),
                            Some(pending_plugin_import.details),
                        )
                        .await
                    {
                        Ok(plugin_outcome) => {
                            apply_plugin_outcome_to_item_result(&mut item_result, plugin_outcome);
                        }
                        Err(error) => {
                            record_import_error(
                                &mut item_result,
                                "plugin_import",
                                /*sub_error_type*/ None,
                                error.to_string(),
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
                thread_manager.skills_service().clear_cache();
            }
            send_completed_import_notification(
                &outgoing,
                state_db.as_ref(),
                &analytics_events_client,
                import_id,
                analytics_source,
                &completed_item_results,
            )
            .await;
        });

        Ok(())
    }

    async fn external_agent_memory_import_enabled(&self) -> bool {
        let config = match self
            .config_manager
            .load_latest_config(/*fallback_cwd*/ None)
            .await
        {
            Ok(config) => config,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "failed to reload config for external agent memory import detection"
                );
                return false;
            }
        };
        config.features.enabled(Feature::ExternalAgentMemoryImport)
    }

    pub(crate) async fn read_import_histories(
        &self,
    ) -> Result<ExternalAgentConfigImportHistoriesReadResponse, JSONRPCErrorError> {
        let state_db = self
            .state_db
            .as_ref()
            .ok_or_else(|| internal_error("state database is unavailable"))?;
        let histories = state_db
            .external_agent_config_import_history_records()
            .await
            .map_err(|err| internal_error(format!("failed to read import histories: {err}")))?;
        let data = histories
            .into_iter()
            .map(protocol_import_history)
            .collect::<Result<Vec<_>, _>>()?;
        let connectors = read_imported_connector_candidates(self.migration_service.codex_home())
            .map_err(|err| {
                internal_error(format!(
                    "failed to read imported connector candidates: {err}"
                ))
            })?
            .into_iter()
            .map(|candidate| ExternalAgentImportedConnectorCandidate {
                name: candidate.name,
                session_count: candidate.session_count,
                source: ExternalAgentImportedConnectorSource::RemoteMcpServersConfig,
            })
            .collect();

        Ok(ExternalAgentConfigImportHistoriesReadResponse { data, connectors })
    }

    fn validate_pending_session_imports(
        &self,
        params: &ExternalAgentConfigImportParams,
        migration_service: &ExternalAgentConfigService,
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
            let canonical_path =
                match migration_service.external_agent_session_source_path(&session.path) {
                    Ok(Some(canonical_path)) => canonical_path,
                    Ok(None) => {
                        record_import_error(
                            &mut item_result,
                            "session_missing",
                            Some("session_not_detected"),
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
                            Some("failed_to_resolve_session_source_path"),
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
        migration_service: &ExternalAgentConfigService,
    ) -> CoreImportOutcome {
        migration_service
            .import(core_migration_items(
                params
                    .migration_items
                    .into_iter()
                    .filter(|item| item.item_type != ExternalAgentConfigMigrationItemType::Sessions)
                    .collect(),
            ))
            .await
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
    analytics_events_client: &AnalyticsEventsClient,
    import_id: String,
    analytics_source: String,
    item_results: &[CoreImportItemResult],
) {
    let notification = completed_notification(import_id, item_results);
    log_completed_import_failures(&notification);
    track_completed_import_notification(analytics_events_client, &analytics_source, &notification);
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

fn log_completed_import_failures(notification: &ExternalAgentConfigImportCompletedNotification) {
    for type_result in &notification.item_type_results {
        for failure in &type_result.failures {
            let error_type = import_failure_error_type(failure);
            tracing::warn!(
                import_id = %notification.import_id,
                item_type = ?failure.item_type,
                error_type = %error_type,
                failure_stage = %failure.failure_stage,
                cwd = ?failure.cwd,
                source = ?failure.source,
                error = %failure.message,
                "external agent config migration item failed"
            );
        }
    }
}

fn track_completed_import_notification(
    analytics_events_client: &AnalyticsEventsClient,
    analytics_source: &str,
    notification: &ExternalAgentConfigImportCompletedNotification,
) {
    for type_result in &notification.item_type_results {
        let item_type = analytics_migration_item_type(type_result.item_type).to_string();
        analytics_events_client.track_external_agent_config_import_completed(
            ExternalAgentConfigImportCompletedInput {
                import_id: notification.import_id.clone(),
                source: analytics_source.to_string(),
                item_type: item_type.clone(),
                success_count: type_result.successes.len(),
                failed_count: type_result.failures.len(),
            },
        );
        for failure in &type_result.failures {
            analytics_events_client.track_external_agent_config_import_failure(
                ExternalAgentConfigImportFailureInput {
                    import_id: notification.import_id.clone(),
                    source: analytics_source.to_string(),
                    item_type: item_type.clone(),
                    failure_stage: failure.failure_stage.clone(),
                    error_type: import_failure_error_type(failure),
                    sub_error_type: failure.sub_error_type.clone(),
                },
            );
        }
    }
}

fn import_failure_error_type(failure: &ProtocolImportFailure) -> String {
    failure
        .error_type
        .clone()
        .unwrap_or_else(|| failure.failure_stage.clone())
}

fn analytics_migration_item_type(item_type: ExternalAgentConfigMigrationItemType) -> &'static str {
    match item_type {
        ExternalAgentConfigMigrationItemType::AgentsMd => "AGENTS_MD",
        ExternalAgentConfigMigrationItemType::Config => "CONFIG",
        ExternalAgentConfigMigrationItemType::Skills => "SKILLS",
        ExternalAgentConfigMigrationItemType::Plugins => "PLUGINS",
        ExternalAgentConfigMigrationItemType::McpServerConfig => "MCP_SERVER_CONFIG",
        ExternalAgentConfigMigrationItemType::Subagents => "SUBAGENTS",
        ExternalAgentConfigMigrationItemType::Hooks => "HOOKS",
        ExternalAgentConfigMigrationItemType::Commands => "COMMANDS",
        ExternalAgentConfigMigrationItemType::Memory => "MEMORY",
        ExternalAgentConfigMigrationItemType::Sessions => "SESSIONS",
    }
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
                item_type: serde_json::from_value(serde_json::to_value(success.item_type)?)?,
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
                item_type: serde_json::from_value(serde_json::to_value(failure.item_type)?)?,
                error_type: failure.error_type.clone(),
                sub_error_type: failure.sub_error_type.clone(),
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
#[path = "processor_tests.rs"]
mod tests;
