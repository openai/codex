use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::DateTime;
use chrono::SecondsFormat;
use chrono::Utc;
use codex_arg0::Arg0DispatchPaths;
use codex_core::ThreadManager;
use codex_core::config::ConfigOverrides;
use codex_external_agent_sessions::CompletedExternalAgentSessionImport;
use codex_external_agent_sessions::ExternalAgentSessionMigration;
use codex_external_agent_sessions::ImportedExternalAgentSession;
use codex_external_agent_sessions::PendingSessionImport;
use codex_external_agent_sessions::prepare_validated_session_import;
use codex_external_agent_sessions::record_completed_session_imports;
use codex_models_manager::manager::RefreshStrategy;
use codex_protocol::ThreadId;
use codex_protocol::models::BaseInstructions;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::SessionContextWindow;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_rollout::is_persisted_rollout_item;
use codex_thread_store::DeleteThreadParams;
use codex_thread_store::LocalThreadStore;
use codex_thread_store::ThreadMetadataPatch;
use codex_thread_store::ThreadStore;
use codex_thread_store::UpdateThreadMetadataParams;
use futures::StreamExt;
use tokio::sync::Mutex;
use tokio::sync::Semaphore;

use crate::config::external_agent_config::ExternalAgentConfigImportItemResult;
use crate::config::external_agent_config::record_import_error;
use crate::config_manager::ConfigManager;

use super::external_agent_session_rollout::materialize_imported_rollout;

const SESSION_IMPORT_CONCURRENCY: usize = 5;

#[derive(Clone)]
pub(super) struct ExternalAgentSessionImporter {
    codex_home: PathBuf,
    permits: Arc<Semaphore>,
    reserved_source_identities: Arc<Mutex<HashSet<String>>>,
    thread_manager: Arc<ThreadManager>,
    thread_store: Arc<dyn ThreadStore>,
    config_manager: ConfigManager,
    arg0_paths: Arg0DispatchPaths,
}

impl ExternalAgentSessionImporter {
    pub(super) fn new(
        codex_home: PathBuf,
        thread_manager: Arc<ThreadManager>,
        thread_store: Arc<dyn ThreadStore>,
        config_manager: ConfigManager,
        arg0_paths: Arg0DispatchPaths,
    ) -> Self {
        Self {
            codex_home,
            permits: Arc::new(Semaphore::new(1)),
            reserved_source_identities: Arc::new(Mutex::new(HashSet::new())),
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
    ) -> ExternalAgentConfigImportItemResult {
        if sessions.is_empty() {
            return item_result;
        }
        let Ok(_permit) = self.permits.acquire().await else {
            record_import_error(
                &mut item_result,
                "session_permit",
                "external agent session import permit could not be acquired",
                /*source*/ None,
            );
            return item_result;
        };
        let import_results = futures::stream::iter(sessions)
            .map(|session| {
                let importer = self.clone();
                async move { importer.import_requested_session(session).await }
            })
            .buffer_unordered(SESSION_IMPORT_CONCURRENCY);
        futures::pin_mut!(import_results);

        let mut completed_imports = Vec::new();
        while let Some(result) = import_results.next().await {
            match result {
                Ok(Some(completed_import)) => {
                    item_result.record_success(
                        Some(completed_import.source_path.display().to_string()),
                        Some(completed_import.imported_thread_id.to_string()),
                    );
                    completed_imports.push(completed_import);
                }
                Ok(None) => {}
                Err(failure) => {
                    record_import_error(
                        &mut item_result,
                        failure.stage,
                        failure.message.clone(),
                        Some(failure.source_path.display().to_string()),
                    );
                }
            }
        }
        if let Err(err) = record_completed_session_imports(&self.codex_home, completed_imports) {
            record_import_error(
                &mut item_result,
                "session_ledger_update",
                err.to_string(),
                /*source*/ None,
            );
        }
        item_result
    }

    async fn import_requested_session(
        &self,
        session: ExternalAgentSessionMigration,
    ) -> Result<Option<CompletedExternalAgentSessionImport>, SessionImportFailure> {
        let source_path = session.path.clone();
        let Some(pending_import) =
            self.prepare_session_import(session)
                .await
                .map_err(|message| SessionImportFailure {
                    source_path: source_path.clone(),
                    message,
                    stage: "session_prepare",
                })?
        else {
            return Ok(None);
        };
        let source_identity = pending_import
            .session
            .source_session_id
            .as_ref()
            .map(|session_id| format!("session:{session_id}"))
            .unwrap_or_else(|| format!("path:{}", pending_import.source_path.display()));
        if !self
            .reserved_source_identities
            .lock()
            .await
            .insert(source_identity.clone())
        {
            return Ok(None);
        }
        let source_session_id = pending_import.session.source_session_id.clone();
        let imported_thread_id = match self.persist_session(pending_import.session).await {
            Ok(imported_thread_id) => imported_thread_id,
            Err(message) => {
                self.reserved_source_identities
                    .lock()
                    .await
                    .remove(&source_identity);
                return Err(SessionImportFailure {
                    source_path: pending_import.source_path,
                    message,
                    stage: "session_persist",
                });
            }
        };
        Ok(Some(CompletedExternalAgentSessionImport {
            source_path: pending_import.source_path,
            source_session_id,
            source_content_sha256: pending_import.source_content_sha256,
            imported_thread_id,
        }))
    }

    async fn prepare_session_import(
        &self,
        session: ExternalAgentSessionMigration,
    ) -> Result<Option<PendingSessionImport>, String> {
        let codex_home = self.codex_home.clone();
        tokio::task::spawn_blocking(move || prepare_validated_session_import(&codex_home, session))
            .await
            .map_err(|err| format!("external agent session preparation task failed: {err}"))?
            .map_err(|err| format!("failed to prepare external agent session: {err}"))
    }

    async fn persist_session(
        &self,
        session: ImportedExternalAgentSession,
    ) -> Result<ThreadId, String> {
        if !self.thread_store.as_any().is::<LocalThreadStore>() {
            return Err(
                "external agent session import requires the local thread store".to_string(),
            );
        }
        let ImportedExternalAgentSession {
            cwd,
            title,
            first_user_message,
            source_session_id: _,
            created_at,
            updated_at,
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
            .map_err(|err| format!("failed to load imported session config: {err}"))?;
        let models_manager = self.thread_manager.get_models_manager();
        let model = models_manager
            .get_default_model(
                &config.model,
                /*allow_provider_model_fallback*/ false,
                RefreshStrategy::Offline,
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
        let created_at = created_at
            .and_then(|timestamp| DateTime::<Utc>::from_timestamp(timestamp, 0))
            .unwrap_or(now);
        let updated_at = updated_at
            .and_then(|timestamp| DateTime::<Utc>::from_timestamp(timestamp, 0))
            .unwrap_or(created_at);
        let base_instructions = BaseInstructions {
            text: config
                .base_instructions
                .clone()
                .unwrap_or_else(|| model_info.get_model_instructions(config.personality)),
        };
        let session_meta = SessionMeta {
            session_id: thread_id.into(),
            id: thread_id,
            forked_from_id: None,
            parent_thread_id: None,
            timestamp: created_at.to_rfc3339_opts(SecondsFormat::Millis, true),
            cwd: cwd.clone(),
            originator: codex_login::default_client::originator().value,
            cli_version: env!("CARGO_PKG_VERSION").to_string(),
            agent_nickname: source.get_nickname(),
            agent_role: source.get_agent_role(),
            agent_path: source.get_agent_path().map(Into::into),
            source: source.clone(),
            thread_source: None,
            model_provider: Some(model_provider.clone()),
            base_instructions: Some(base_instructions),
            dynamic_tools: None,
            selected_capability_roots: Vec::new(),
            memory_mode: matches!(memory_mode, ThreadMemoryMode::Disabled)
                .then_some("disabled".to_string()),
            history_mode: ThreadHistoryMode::Legacy,
            multi_agent_version: Some(MultiAgentVersion::V1),
            context_window: Some(SessionContextWindow::new(uuid::Uuid::now_v7().to_string())),
        };
        rollout_items.retain(is_persisted_rollout_item);
        let rollout_path = materialize_imported_rollout(
            self.codex_home.as_path(),
            created_at,
            updated_at,
            session_meta,
            rollout_items,
        )
        .await
        .map_err(|err| format!("failed to materialize imported session: {err}"))?;
        let title = title
            .as_deref()
            .and_then(codex_core::util::normalize_thread_name);
        let metadata = ThreadMetadataPatch {
            rollout_path: Some(rollout_path.clone()),
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

        if let Err(err) = self
            .thread_store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: metadata,
                include_archived: false,
            })
            .await
        {
            let _ = self
                .thread_store
                .delete_thread(DeleteThreadParams { thread_id })
                .await;
            let _ = std::fs::remove_file(rollout_path);
            return Err(format!("failed to index imported session: {err}"));
        }
        Ok(thread_id)
    }
}

struct SessionImportFailure {
    source_path: PathBuf,
    message: String,
    stage: &'static str,
}
