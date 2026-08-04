use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::DateTime;
use chrono::Utc;
use codex_arg0::Arg0DispatchPaths;
use codex_core::ThreadManager;
use codex_core::config::ConfigOverrides;
use codex_external_agent_migration::ExternalAgentConfigImportItemResult;
use codex_external_agent_migration::record_import_error;
use codex_external_agent_migration::sessions::CompletedExternalAgentSessionImport;
use codex_external_agent_migration::sessions::ExistingSessionAppend;
use codex_external_agent_migration::sessions::ExternalAgentSessionMigration;
use codex_external_agent_migration::sessions::ImportedExternalAgentSession;
use codex_external_agent_migration::sessions::ImportedSessionConnectorAttribution;
use codex_external_agent_migration::sessions::PendingSessionImport;
use codex_external_agent_migration::sessions::SessionImportTarget;
use codex_external_agent_migration::sessions::SessionMetadataMode;
use codex_external_agent_migration::sessions::append_existing_session;
use codex_external_agent_migration::sessions::append_imported_session_connector_names;
use codex_external_agent_migration::sessions::detect_imported_cla_session_connectors_by_source_path;
use codex_external_agent_migration::sessions::prepare_validated_session_import_with_metadata_mode;
use codex_external_agent_migration::sessions::record_completed_session_imports;
use codex_models_manager::manager::RefreshStrategy;
use codex_protocol::ThreadId;
use codex_protocol::models::BaseInstructions;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_rollout::is_persisted_rollout_item;
use codex_thread_store::AppendThreadItemsParams;
use codex_thread_store::CreateThreadParams;
use codex_thread_store::ThreadMetadataPatch;
use codex_thread_store::ThreadPersistenceMetadata;
use codex_thread_store::ThreadStore;
use codex_thread_store::UpdateThreadMetadataParams;
use futures::StreamExt;
use tokio::sync::Semaphore;

use crate::config_manager::ConfigManager;

const SESSION_IMPORT_CONCURRENCY: usize = 5;

struct CompletedSessionImport {
    cwd: PathBuf,
    import: CompletedExternalAgentSessionImport,
    connector_attribution: Option<ImportedSessionConnectorAttribution>,
}

enum SessionImportOutcome {
    Created(CompletedSessionImport),
    Appended {
        cwd: PathBuf,
        source_path: PathBuf,
        imported_thread_id: ThreadId,
        title: Option<String>,
    },
}

#[derive(Clone)]
pub(super) struct ExternalAgentSessionImporter {
    codex_home: PathBuf,
    connector_metadata_roots: Vec<PathBuf>,
    permits: Arc<Semaphore>,
    append_checkpoint_permits: Arc<Semaphore>,
    thread_manager: Arc<ThreadManager>,
    thread_store: Arc<dyn ThreadStore>,
    config_manager: ConfigManager,
    arg0_paths: Arg0DispatchPaths,
}

impl ExternalAgentSessionImporter {
    pub(super) fn new(
        codex_home: PathBuf,
        connector_metadata_roots: Vec<PathBuf>,
        thread_manager: Arc<ThreadManager>,
        thread_store: Arc<dyn ThreadStore>,
        config_manager: ConfigManager,
        arg0_paths: Arg0DispatchPaths,
    ) -> Self {
        Self {
            codex_home,
            connector_metadata_roots,
            permits: Arc::new(Semaphore::new(1)),
            append_checkpoint_permits: Arc::new(Semaphore::new(1)),
            thread_manager,
            thread_store,
            config_manager,
            arg0_paths,
        }
    }

    pub(super) async fn import_sessions(
        &self,
        sessions: Vec<ExternalAgentSessionMigration>,
        mut item_result: ExternalAgentConfigImportItemResult,
        metadata_mode: SessionMetadataMode,
        mut connector_names_by_source_path: BTreeMap<PathBuf, Vec<String>>,
    ) -> ExternalAgentConfigImportItemResult {
        if sessions.is_empty() {
            return item_result;
        }
        let Ok(_permit) = self.permits.acquire().await else {
            record_import_error(
                &mut item_result,
                "session_permit",
                Some("failed_to_acquire_import_permit"),
                "external agent session import permit could not be acquired",
                /*source*/ None,
            );
            return item_result;
        };
        let import_results = futures::stream::iter(sessions)
            .map(|session| {
                let importer = self.clone();
                async move {
                    importer
                        .import_requested_session(session, metadata_mode)
                        .await
                }
            })
            .buffer_unordered(SESSION_IMPORT_CONCURRENCY);
        futures::pin_mut!(import_results);

        let mut completed_imports = Vec::new();
        let mut appended_connector_names_by_source_path = BTreeMap::new();
        while let Some(result) = import_results.next().await {
            match result {
                Ok(Some(SessionImportOutcome::Created(completed_import))) => {
                    item_result.record_success_with_cwd(
                        Some(completed_import.cwd.clone()),
                        Some(completed_import.import.source_path.display().to_string()),
                        Some(completed_import.import.imported_thread_id.to_string()),
                        completed_import.import.title.clone(),
                    );
                    completed_imports.push(completed_import);
                }
                Ok(Some(SessionImportOutcome::Appended {
                    cwd,
                    source_path,
                    imported_thread_id,
                    title,
                })) => {
                    item_result.record_success_with_cwd(
                        Some(cwd),
                        Some(source_path.display().to_string()),
                        Some(imported_thread_id.to_string()),
                        title,
                    );
                    if let Some(connector_names) =
                        connector_names_by_source_path.remove(&source_path)
                    {
                        appended_connector_names_by_source_path
                            .insert(source_path, connector_names);
                    }
                }
                Ok(None) => {}
                Err(failure) => {
                    let SessionImportFailure {
                        source_path,
                        message,
                        stage,
                        sub_error_type,
                    } = failure;
                    record_import_error(
                        &mut item_result,
                        stage,
                        Some(sub_error_type),
                        message,
                        Some(source_path.display().to_string()),
                    );
                }
            }
        }
        if let Err(err) = append_imported_session_connector_names(
            &self.codex_home,
            appended_connector_names_by_source_path,
        ) {
            record_import_error(
                &mut item_result,
                "session_ledger_update",
                Some("failed_to_update_session_connector_metadata"),
                err.to_string(),
                /*source*/ None,
            );
        }
        if completed_imports.is_empty() {
            return item_result;
        }
        let connector_attributions_by_source_path = completed_imports
            .iter()
            .filter_map(|completed_import| {
                completed_import
                    .connector_attribution
                    .clone()
                    .map(|attribution| (completed_import.import.source_path.clone(), attribution))
            })
            .collect::<BTreeMap<_, _>>();
        let connector_metadata_roots = self.connector_metadata_roots.clone();
        let mut attributed_connector_names_by_source_path =
            match tokio::task::spawn_blocking(move || {
                detect_imported_cla_session_connectors_by_source_path(
                    &connector_attributions_by_source_path,
                    &connector_metadata_roots,
                )
            })
            .await
            {
                Ok(connector_names_by_source_path) => connector_names_by_source_path,
                Err(err) => {
                    record_import_error(
                        &mut item_result,
                        "session_connector_detection_task",
                        Some("session_connector_detection_task_failed"),
                        err.to_string(),
                        /*source*/ None,
                    );
                    Default::default()
                }
            };
        for completed_import in &mut completed_imports {
            completed_import.import.connector_names = attributed_connector_names_by_source_path
                .remove(&completed_import.import.source_path)
                .unwrap_or_default();
        }
        for completed_import in &mut completed_imports {
            let Some(connector_names) =
                connector_names_by_source_path.remove(&completed_import.import.source_path)
            else {
                continue;
            };
            completed_import
                .import
                .connector_names
                .extend(connector_names);
        }
        let completed_imports = completed_imports
            .into_iter()
            .map(|completed_import| completed_import.import)
            .collect();
        if let Err(err) = record_completed_session_imports(&self.codex_home, completed_imports) {
            record_import_error(
                &mut item_result,
                "session_ledger_update",
                Some("failed_to_update_session_ledger"),
                err.to_string(),
                /*source*/ None,
            );
        }
        item_result
    }

    async fn import_requested_session(
        &self,
        session: ExternalAgentSessionMigration,
        metadata_mode: SessionMetadataMode,
    ) -> Result<Option<SessionImportOutcome>, SessionImportFailure> {
        let source_path = session.path.clone();
        let Some(pending_import) = self
            .prepare_session_import(session, metadata_mode)
            .await
            .map_err(|failure| SessionImportFailure {
                source_path: source_path.clone(),
                message: failure.message,
                stage: "session_prepare",
                sub_error_type: failure.sub_error_type,
            })?
        else {
            return Ok(None);
        };
        let PendingSessionImport {
            source_path,
            source_content_sha256,
            target,
            attributed_mcp_server_ids,
            session,
        } = pending_import;
        match target {
            SessionImportTarget::New => self
                .create_session_import(
                    source_path,
                    source_content_sha256,
                    attributed_mcp_server_ids,
                    session,
                )
                .await
                .map(SessionImportOutcome::Created)
                .map(Some),
            SessionImportTarget::Existing {
                thread_id,
                expected_source_content_sha256,
            } => {
                let cwd = session.cwd.clone();
                let title = session.title.clone();
                let appended = append_existing_session(
                    &self.codex_home,
                    self.append_checkpoint_permits.as_ref(),
                    self.thread_manager.as_ref(),
                    self.thread_store.as_ref(),
                    ExistingSessionAppend {
                        source_path: &source_path,
                        source_content_sha256: &source_content_sha256,
                        expected_source_content_sha256: &expected_source_content_sha256,
                        thread_id,
                        source_items: &session.rollout_items,
                    },
                )
                .await;
                Ok(appended.then_some(SessionImportOutcome::Appended {
                    cwd,
                    source_path,
                    imported_thread_id: thread_id,
                    title,
                }))
            }
        }
    }

    async fn create_session_import(
        &self,
        source_path: PathBuf,
        source_content_sha256: String,
        attributed_mcp_server_ids: BTreeSet<String>,
        session: ImportedExternalAgentSession,
    ) -> Result<CompletedSessionImport, SessionImportFailure> {
        let connector_attribution = source_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(str::trim)
            .filter(|session_id| !session_id.is_empty())
            .map(|session_id| ImportedSessionConnectorAttribution {
                session_id: session_id.to_string(),
                server_ids: attributed_mcp_server_ids,
            });
        let cwd = session.cwd.clone();
        let title = session.title.clone();
        let imported_thread_id =
            self.persist_session(session)
                .await
                .map_err(|failure| SessionImportFailure {
                    source_path: source_path.clone(),
                    message: failure.message,
                    stage: "session_persist",
                    sub_error_type: failure.sub_error_type,
                })?;
        Ok(CompletedSessionImport {
            cwd,
            import: CompletedExternalAgentSessionImport {
                source_path,
                source_content_sha256,
                imported_thread_id,
                connector_names: Vec::new(),
                title,
            },
            connector_attribution,
        })
    }

    async fn prepare_session_import(
        &self,
        session: ExternalAgentSessionMigration,
        metadata_mode: SessionMetadataMode,
    ) -> Result<Option<PendingSessionImport>, SessionImportStepFailure> {
        let codex_home = self.codex_home.clone();
        tokio::task::spawn_blocking(move || {
            prepare_validated_session_import_with_metadata_mode(&codex_home, session, metadata_mode)
        })
        .await
        .map_err(|err| {
            SessionImportStepFailure::new(
                "session_preparation_task_failed",
                format!("external agent session preparation task failed: {err}"),
            )
        })?
        .map_err(|err| {
            SessionImportStepFailure::new(
                "failed_to_prepare_session",
                format!("failed to prepare external agent session: {err}"),
            )
        })
    }

    async fn persist_session(
        &self,
        session: ImportedExternalAgentSession,
    ) -> Result<ThreadId, SessionImportStepFailure> {
        let ImportedExternalAgentSession {
            cwd,
            title,
            first_user_message,
            mut rollout_items,
        } = session;
        let config = self
            .config_manager
            .load_with_overrides(
                /*request_overrides*/ None,
                ConfigOverrides {
                    cwd: Some(cwd),
                    codex_linux_sandbox_exe: self.arg0_paths.codex_linux_sandbox_exe.clone(),
                    main_execve_wrapper_exe: self.arg0_paths.main_execve_wrapper_exe.clone(),
                    ..Default::default()
                },
            )
            .await
            .map_err(|err| {
                SessionImportStepFailure::new(
                    "failed_to_load_session_config",
                    format!("failed to load imported session config: {err}"),
                )
            })?;
        let models_manager = self.thread_manager.get_models_manager();
        let model = models_manager
            .get_default_model(
                &config.model,
                /*allow_provider_model_fallback*/ false,
                RefreshStrategy::Offline,
                config.http_client_factory(),
            )
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
            session_id: thread_id.into(),
            thread_id,
            extra_config: None,
            forked_from_id: None,
            parent_thread_id: None,
            source: source.clone(),
            thread_source: None,
            originator: codex_login::default_client::originator().value,
            base_instructions: BaseInstructions {
                text: config
                    .base_instructions
                    .clone()
                    .unwrap_or_else(|| model_info.get_model_instructions(config.personality)),
            },
            dynamic_tools: Vec::new(),
            selected_capability_roots: Vec::new(),
            multi_agent_version: Some(MultiAgentVersion::V1),
            history_mode: ThreadHistoryMode::Legacy,
            history_base: None,
            subagent_history_start_ordinal: None,
            initial_window_id: uuid::Uuid::now_v7().to_string(),
            metadata: ThreadPersistenceMetadata {
                cwd: Some(cwd.clone()),
                model_provider: model_provider.clone(),
                memory_mode,
            },
        };
        rollout_items.retain(|item| is_persisted_rollout_item(item, ThreadHistoryMode::Legacy));
        let (created_at, updated_at) = rollout_items
            .iter()
            .filter_map(|item| match item {
                RolloutItem::EventMsg(EventMsg::TurnStarted(event)) => event.started_at,
                RolloutItem::EventMsg(EventMsg::TurnComplete(event)) => event.completed_at,
                _ => None,
            })
            .fold(None, |chronology: Option<(i64, i64)>, timestamp| {
                Some(match chronology {
                    Some((created_at, updated_at)) => {
                        (created_at.min(timestamp), updated_at.max(timestamp))
                    }
                    None => (timestamp, timestamp),
                })
            })
            .and_then(|(created_at, updated_at)| {
                Some((
                    DateTime::from_timestamp(created_at, /*nsecs*/ 0)?,
                    DateTime::from_timestamp(updated_at, /*nsecs*/ 0)?,
                ))
            })
            .unwrap_or((now, now));
        let title = title
            .as_deref()
            .and_then(codex_core::util::normalize_thread_name);
        let metadata = ThreadMetadataPatch {
            title,
            preview: first_user_message.clone(),
            model_provider: Some(model_provider),
            created_at: Some(created_at),
            updated_at: Some(updated_at),
            advance_recency_at: Some(updated_at),
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
            .map_err(|err| {
                SessionImportStepFailure::new(
                    "failed_to_create_thread",
                    format!("failed to import session: {err}"),
                )
            })?;
        if !rollout_items.is_empty()
            && let Err(err) = self
                .thread_store
                .append_items(AppendThreadItemsParams {
                    thread_id,
                    items: rollout_items,
                })
                .await
        {
            let _ = self.thread_store.discard_thread(thread_id).await;
            return Err(SessionImportStepFailure::new(
                "failed_to_append_thread_items",
                format!("failed to import session: {err}"),
            ));
        }

        self.thread_store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: metadata,
                include_archived: false,
            })
            .await
            .map_err(|err| {
                SessionImportStepFailure::new(
                    "failed_to_update_thread_metadata",
                    format!("failed to update imported session: {err}"),
                )
            })?;
        self.thread_store
            .persist_thread(thread_id)
            .await
            .map_err(|err| {
                SessionImportStepFailure::new(
                    "failed_to_persist_thread",
                    format!("failed to persist imported session: {err}"),
                )
            })?;
        self.thread_store
            .shutdown_thread(thread_id)
            .await
            .map_err(|err| {
                SessionImportStepFailure::new(
                    "failed_to_shutdown_thread",
                    format!("failed to shutdown imported session: {err}"),
                )
            })?;
        Ok(thread_id)
    }
}

struct SessionImportFailure {
    source_path: PathBuf,
    message: String,
    stage: &'static str,
    sub_error_type: &'static str,
}

struct SessionImportStepFailure {
    sub_error_type: &'static str,
    message: String,
}

impl SessionImportStepFailure {
    fn new(sub_error_type: &'static str, message: String) -> Self {
        Self {
            sub_error_type,
            message,
        }
    }
}
