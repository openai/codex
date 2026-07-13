//! App-server session facade used by the TUI event loop.
//!
//! This module owns the typed JSON-RPC calls needed by the TUI and keeps
//! request/response plumbing out of `App` and `ChatWidget`.

mod fs;

use crate::bottom_pane::FeedbackAudience;
use crate::legacy_core::config::Config;
use crate::permission_compat::legacy_compatible_permission_profile;
use crate::service_tier_resolution;
use crate::session_state::MessageHistoryMetadata;
use crate::session_state::ThreadSessionState;
use crate::status::StatusAccountDisplay;
use crate::status::plan_type_display_name;
use crate::terminal_visualization_instructions::with_terminal_visualization_instructions;
use codex_app_server_client::AppServerClient;
use codex_app_server_client::AppServerEvent;
use codex_app_server_client::AppServerPath;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::Account;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::AuthMode;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigRequirementsReadResponse;
use codex_app_server_protocol::ConfigWriteResponse;
use codex_app_server_protocol::ExternalAgentConfigDetectParams;
use codex_app_server_protocol::ExternalAgentConfigDetectResponse;
use codex_app_server_protocol::ExternalAgentConfigImportParams;
use codex_app_server_protocol::ExternalAgentConfigImportResponse;
use codex_app_server_protocol::ExternalAgentConfigMigrationItem;
use codex_app_server_protocol::GetAccountParams;
use codex_app_server_protocol::GetAccountRateLimitsResponse;
use codex_app_server_protocol::GetAccountResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::LogoutAccountResponse;
use codex_app_server_protocol::MemoryResetResponse;
use codex_app_server_protocol::Model as ApiModel;
use codex_app_server_protocol::ModelListParams;
use codex_app_server_protocol::ModelListResponse;
use codex_app_server_protocol::NewThreadModelDefaults;
use codex_app_server_protocol::RateLimitSnapshot;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ReviewDelivery;
use codex_app_server_protocol::ReviewStartParams;
use codex_app_server_protocol::ReviewStartResponse;
use codex_app_server_protocol::ReviewTarget;
use codex_app_server_protocol::SkillsListParams;
use codex_app_server_protocol::SkillsListResponse;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadApproveGuardianDeniedActionParams;
use codex_app_server_protocol::ThreadApproveGuardianDeniedActionResponse;
use codex_app_server_protocol::ThreadArchiveParams;
use codex_app_server_protocol::ThreadArchiveResponse;
use codex_app_server_protocol::ThreadBackgroundTerminalsCleanParams;
use codex_app_server_protocol::ThreadBackgroundTerminalsCleanResponse;
use codex_app_server_protocol::ThreadCompactStartParams;
use codex_app_server_protocol::ThreadCompactStartResponse;
use codex_app_server_protocol::ThreadDeleteParams;
use codex_app_server_protocol::ThreadDeleteResponse;
use codex_app_server_protocol::ThreadForkParams;
use codex_app_server_protocol::ThreadForkResponse;
use codex_app_server_protocol::ThreadGoalClearParams;
use codex_app_server_protocol::ThreadGoalClearResponse;
use codex_app_server_protocol::ThreadGoalGetParams;
use codex_app_server_protocol::ThreadGoalGetResponse;
use codex_app_server_protocol::ThreadGoalSetParams;
use codex_app_server_protocol::ThreadGoalSetResponse;
use codex_app_server_protocol::ThreadGoalStatus;
use codex_app_server_protocol::ThreadInjectItemsParams;
use codex_app_server_protocol::ThreadInjectItemsResponse;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadLoadedListResponse;
use codex_app_server_protocol::ThreadMemoryMode;
use codex_app_server_protocol::ThreadMemoryModeSetParams;
use codex_app_server_protocol::ThreadMemoryModeSetResponse;
use codex_app_server_protocol::ThreadMetadataGitInfoUpdateParams;
use codex_app_server_protocol::ThreadMetadataUpdateParams;
use codex_app_server_protocol::ThreadMetadataUpdateResponse;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadRollbackParams;
use codex_app_server_protocol::ThreadRollbackResponse;
use codex_app_server_protocol::ThreadSetNameParams;
use codex_app_server_protocol::ThreadSetNameResponse;
use codex_app_server_protocol::ThreadSettingsUpdateParams;
use codex_app_server_protocol::ThreadSettingsUpdateResponse;
use codex_app_server_protocol::ThreadShellCommandParams;
use codex_app_server_protocol::ThreadShellCommandResponse;
use codex_app_server_protocol::ThreadSource;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStartSource;
use codex_app_server_protocol::ThreadUnarchiveParams;
use codex_app_server_protocol::ThreadUnarchiveResponse;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnInterruptResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnSteerParams;
use codex_app_server_protocol::TurnSteerResponse;
use codex_app_server_protocol::UserInput;
use codex_otel::TelemetryAuthMode;
use codex_protocol::ThreadId;
use codex_protocol::approvals::GuardianAssessmentEvent;
use codex_protocol::config_types::SERVICE_TIER_DEFAULT_REQUEST_VALUE;
use codex_protocol::models::ActivePermissionProfile;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelAvailabilityNux;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelServiceTier;
use codex_protocol::openai_models::ModelUpgrade;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use color_eyre::eyre::ContextCompat;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use uuid::Uuid;

const JSONRPC_INVALID_REQUEST: i64 = -32600;
const JSONRPC_METHOD_NOT_FOUND: i64 = -32601;
pub(crate) const EXTERNAL_AGENT_CONFIG_IMPORT_IN_PROGRESS_MESSAGE: &str =
    "A previous Claude Code import is still running. Wait for it to finish before importing again.";
const THREAD_SETTINGS_UPDATE_METHOD: &str = "thread/settings/update";

fn bootstrap_request_error(context: &'static str, err: TypedRequestError) -> color_eyre::Report {
    color_eyre::eyre::eyre!("{context}: {err}")
}

fn is_thread_settings_update_unsupported(source: &JSONRPCErrorError) -> bool {
    source.code == JSONRPC_METHOD_NOT_FOUND
        || (source.code == JSONRPC_INVALID_REQUEST
            && source.message.contains(THREAD_SETTINGS_UPDATE_METHOD))
}

/// Data collected during the TUI bootstrap phase that the main event loop
/// needs to configure the UI, telemetry, and initial rate-limit prefetch.
///
/// Rate-limit snapshots are intentionally **not** included here; they are
/// fetched asynchronously after bootstrap returns so that the TUI can render
/// its first frame without waiting for the rate-limit round-trip.
pub(crate) struct AppServerBootstrap {
    pub(crate) duration: Duration,
    pub(crate) account_email: Option<String>,
    pub(crate) auth_mode: Option<TelemetryAuthMode>,
    pub(crate) status_account_display: Option<StatusAccountDisplay>,
    pub(crate) plan_type: Option<codex_protocol::account::PlanType>,
    /// Whether the configured model provider needs OpenAI-style auth. Combined
    /// with `has_chatgpt_account` to decide if a startup rate-limit prefetch
    /// should be fired.
    pub(crate) requires_openai_auth: bool,
    pub(crate) default_model: String,
    pub(crate) feedback_audience: FeedbackAudience,
    pub(crate) has_chatgpt_account: bool,
    pub(crate) available_models: Vec<ModelPreset>,
}

pub(crate) struct AppServerSession {
    client: AppServerClient,
    next_request_id: i64,
    remote_cwd_override: Option<PathBuf>,
    thread_params_mode: ThreadParamsMode,
    thread_settings_update_supported: bool,
    default_model: Option<String>,
    available_models: Vec<ModelPreset>,
    managed_new_thread_defaults: Option<NewThreadModelDefaults>,
    external_agent_config_import_completion_pending: AtomicBool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ThreadParamsMode {
    Embedded,
    Remote,
}

/// Determines where model settings come from when resuming a thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResumeModelSettings {
    /// Sends the current config's model, provider, and reasoning effort as explicit overrides.
    OverrideFromCurrentConfig,
    /// Omits those overrides so app-server restores the settings saved with the thread.
    RestoreFromThread,
}

impl ThreadParamsMode {
    fn model_provider_from_config(self, config: &Config) -> Option<String> {
        match self {
            Self::Embedded => Some(config.model_provider_id.clone()),
            Self::Remote => None,
        }
    }
}

#[derive(Debug)]
pub(crate) struct AppServerStartedThread {
    pub(crate) session: ThreadSessionState,
    pub(crate) turns: Vec<Turn>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TurnPermissionsOverride {
    /// Leave the app-server thread's sticky permission profile unchanged.
    Preserve,
    /// Select a named or built-in profile by id.
    ActiveProfile(ActivePermissionProfile),
    /// Apply a user-selected legacy/custom permission profile.
    LegacySandbox(PermissionProfile),
}

impl AppServerSession {
    pub(crate) fn new(client: AppServerClient, thread_params_mode: ThreadParamsMode) -> Self {
        Self {
            client,
            next_request_id: 1,
            remote_cwd_override: None,
            thread_params_mode,
            thread_settings_update_supported: true,
            default_model: None,
            available_models: Vec::new(),
            managed_new_thread_defaults: None,
            external_agent_config_import_completion_pending: AtomicBool::new(false),
        }
    }

    pub(crate) fn with_remote_cwd_override(mut self, remote_cwd_override: Option<PathBuf>) -> Self {
        self.remote_cwd_override = remote_cwd_override;
        self
    }

    pub(crate) fn remote_cwd_override(&self) -> Option<&std::path::Path> {
        self.remote_cwd_override.as_deref()
    }

    pub(crate) fn uses_remote_workspace(&self) -> bool {
        matches!(self.thread_params_mode, ThreadParamsMode::Remote)
    }

    pub(crate) fn uses_embedded_app_server(&self) -> bool {
        matches!(&self.client, AppServerClient::InProcess(_))
    }

    pub(crate) fn codex_home_path(
        &self,
        local_codex_home: &AbsolutePathBuf,
    ) -> Option<AppServerPath> {
        self.client.codex_home(local_codex_home)
    }

    pub(crate) fn server_version(&self) -> Option<&str> {
        let AppServerClient::Remote(client) = &self.client else {
            return None;
        };
        client.server_version()
    }

    pub(crate) async fn bootstrap(&mut self, config: &Config) -> Result<AppServerBootstrap> {
        let started_at = Instant::now();
        let account = self.read_account().await?;
        let requirements_request_id = self.next_request_id();
        let requirements: ConfigRequirementsReadResponse = self
            .client
            .request_typed(ClientRequest::ConfigRequirementsRead {
                request_id: requirements_request_id,
                params: None,
            })
            .await
            .map_err(|err| {
                bootstrap_request_error("configRequirements/read failed during TUI bootstrap", err)
            })?;
        self.managed_new_thread_defaults = requirements
            .requirements
            .and_then(|requirements| requirements.models)
            .and_then(|models| models.new_thread);
        let model_request_id = self.next_request_id();
        let models: ModelListResponse = self
            .client
            .request_typed(ClientRequest::ModelList {
                request_id: model_request_id,
                params: ModelListParams {
                    cursor: None,
                    limit: None,
                    include_hidden: Some(true),
                },
            })
            .await
            .map_err(|err| {
                bootstrap_request_error("model/list failed during TUI bootstrap", err)
            })?;
        let available_models = models
            .data
            .into_iter()
            .map(model_preset_from_api_model)
            .collect::<Vec<_>>();
        let default_model = config
            .model
            .clone()
            .or_else(|| {
                available_models
                    .iter()
                    .find(|model| model.is_default)
                    .map(|model| model.model.clone())
            })
            .or_else(|| available_models.first().map(|model| model.model.clone()))
            .wrap_err("model/list returned no models for TUI bootstrap")?;
        self.default_model = Some(default_model.clone());
        self.available_models = available_models.clone();

        let (
            account_email,
            auth_mode,
            status_account_display,
            plan_type,
            feedback_audience,
            has_chatgpt_account,
        ) = match account.account {
            Some(Account::ApiKey {}) => (
                None,
                Some(TelemetryAuthMode::ApiKey),
                Some(StatusAccountDisplay::ApiKey),
                None,
                FeedbackAudience::External,
                false,
            ),
            Some(Account::Chatgpt { email, plan_type }) => {
                let feedback_audience = if email
                    .as_deref()
                    .is_some_and(|email| email.ends_with("@openai.com"))
                {
                    FeedbackAudience::OpenAiEmployee
                } else {
                    FeedbackAudience::External
                };
                (
                    email.clone(),
                    Some(TelemetryAuthMode::Chatgpt),
                    Some(StatusAccountDisplay::ChatGpt {
                        email,
                        plan: Some(plan_type_display_name(plan_type)),
                    }),
                    Some(plan_type),
                    feedback_audience,
                    true,
                )
            }
            Some(Account::AmazonBedrock { .. }) => {
                (None, None, None, None, FeedbackAudience::External, false)
            }
            None => (None, None, None, None, FeedbackAudience::External, false),
        };
        Ok(AppServerBootstrap {
            duration: started_at.elapsed(),
            account_email,
            auth_mode,
            status_account_display,
            plan_type,
            requires_openai_auth: account.requires_openai_auth,
            default_model,
            feedback_audience,
            has_chatgpt_account,
            available_models,
        })
    }

    pub(crate) fn managed_new_thread_defaults(&self) -> Option<&NewThreadModelDefaults> {
        self.managed_new_thread_defaults.as_ref()
    }

    /// Fetches the current account info without refreshing the auth token.
    ///
    /// Used by both `bootstrap` (to populate the initial UI) and `get_login_status`
    /// (to check auth mode without the overhead of a full bootstrap).
    pub(crate) async fn read_account(&mut self) -> Result<GetAccountResponse> {
        let account_request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::GetAccount {
                request_id: account_request_id,
                params: GetAccountParams {
                    refresh_token: false,
                },
            })
            .await
            .map_err(|err| bootstrap_request_error("account/read failed during TUI bootstrap", err))
    }

    pub(crate) async fn external_agent_config_detect(
        &mut self,
        params: ExternalAgentConfigDetectParams,
    ) -> Result<ExternalAgentConfigDetectResponse> {
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::ExternalAgentConfigDetect { request_id, params })
            .await
            .wrap_err("externalAgentConfig/detect failed during Claude Code import")
    }

    pub(crate) async fn external_agent_config_import(
        &mut self,
        migration_items: Vec<ExternalAgentConfigMigrationItem>,
    ) -> Result<()> {
        // Mark the import active before sending the request so a fast completion notification
        // cannot arrive before the TUI records it.
        if self
            .external_agent_config_import_completion_pending
            .swap(true, Ordering::Relaxed)
        {
            color_eyre::eyre::bail!(EXTERNAL_AGENT_CONFIG_IMPORT_IN_PROGRESS_MESSAGE);
        }
        let request_id = self.next_request_id();
        let response: Result<ExternalAgentConfigImportResponse> = self
            .client
            .request_typed(ClientRequest::ExternalAgentConfigImport {
                request_id,
                params: ExternalAgentConfigImportParams {
                    migration_items,
                    source: None,
                },
            })
            .await
            .wrap_err("externalAgentConfig/import failed during Claude Code import");
        match response {
            Ok(_) => Ok(()),
            Err(err) => {
                self.external_agent_config_import_completion_pending
                    .store(false, Ordering::Relaxed);
                Err(err)
            }
        }
    }

    pub(crate) fn external_agent_config_import_in_progress(&self) -> bool {
        self.external_agent_config_import_completion_pending
            .load(Ordering::Relaxed)
    }

    pub(crate) fn consume_external_agent_config_import_completion(&self) -> bool {
        self.external_agent_config_import_completion_pending
            .swap(false, Ordering::Relaxed)
    }

    pub(crate) async fn next_event(&mut self) -> Option<AppServerEvent> {
        self.client.next_event().await
    }

    #[cfg(test)]
    pub(crate) async fn start_thread(&mut self, config: &Config) -> Result<AppServerStartedThread> {
        self.start_thread_with_session_start_source(config, /*session_start_source*/ None)
            .await
    }

    pub(crate) async fn start_thread_with_session_start_source(
        &mut self,
        config: &Config,
        session_start_source: Option<ThreadStartSource>,
    ) -> Result<AppServerStartedThread> {
        let request_id = self.next_request_id();
        let session_config = self.session_config_with_effective_service_tier(config);
        let response: ThreadStartResponse = self
            .client
            .request_typed(ClientRequest::ThreadStart {
                request_id,
                params: thread_start_params_from_config(
                    &session_config,
                    self.thread_params_mode(),
                    self.remote_cwd_override.as_deref(),
                    session_start_source,
                ),
            })
            .await
            .map_err(|err| {
                bootstrap_request_error("thread/start failed during TUI bootstrap", err)
            })?;
        started_thread_from_start_response(response, config, self.thread_params_mode()).await
    }

    pub(crate) async fn resume_thread(
        &mut self,
        config: Config,
        thread_id: ThreadId,
        model_settings: ResumeModelSettings,
    ) -> Result<AppServerStartedThread> {
        let request_id = self.next_request_id();
        let session_config = if model_settings == ResumeModelSettings::RestoreFromThread {
            config.clone()
        } else {
            self.session_config_with_effective_service_tier(&config)
        };
        let response: ThreadResumeResponse = self
            .client
            .request_typed(ClientRequest::ThreadResume {
                request_id,
                params: thread_resume_params_from_config(
                    session_config,
                    thread_id,
                    self.thread_params_mode(),
                    self.remote_cwd_override.as_deref(),
                    model_settings,
                ),
            })
            .await
            .map_err(|err| {
                bootstrap_request_error("thread/resume failed during TUI bootstrap", err)
            })?;
        let fork_parent_title = self
            .fork_parent_title_from_app_server(response.thread.forked_from_id.as_deref())
            .await;
        let mut started =
            started_thread_from_resume_response(response, &config, self.thread_params_mode())
                .await?;
        started.session.fork_parent_title = fork_parent_title;
        Ok(started)
    }

    pub(crate) async fn fork_thread(
        &mut self,
        config: Config,
        thread_id: ThreadId,
    ) -> Result<AppServerStartedThread> {
        let request_id = self.next_request_id();
        let session_config = self.session_config_with_effective_service_tier(&config);
        let response: ThreadForkResponse = self
            .client
            .request_typed(ClientRequest::ThreadFork {
                request_id,
                params: thread_fork_params_from_config(
                    session_config,
                    thread_id,
                    self.thread_params_mode(),
                    self.remote_cwd_override.as_deref(),
                ),
            })
            .await
            .map_err(|err| {
                bootstrap_request_error("thread/fork failed during TUI bootstrap", err)
            })?;
        let fork_parent_title = self
            .fork_parent_title_from_app_server(response.thread.forked_from_id.as_deref())
            .await;
        let mut started =
            started_thread_from_fork_response(response, &config, self.thread_params_mode()).await?;
        started.session.fork_parent_title = fork_parent_title;
        Ok(started)
    }

    pub(crate) fn thread_params_mode(&self) -> ThreadParamsMode {
        self.thread_params_mode
    }

    fn session_config_with_effective_service_tier(&self, config: &Config) -> Config {
        let Some(model) = config.model.as_deref().or(self.default_model.as_deref()) else {
            return config.clone();
        };
        let mut session_config = config.clone();
        match service_tier_resolution::service_tier_update_for_core(
            config,
            model,
            &self.available_models,
        ) {
            Some(Some(service_tier)) => {
                session_config.service_tier = Some(service_tier);
                session_config.notices.fast_default_opt_out = None;
            }
            Some(None) => {
                session_config.service_tier = Some(SERVICE_TIER_DEFAULT_REQUEST_VALUE.to_string());
                session_config.notices.fast_default_opt_out = None;
            }
            None => {
                session_config.service_tier = None;
                session_config.notices.fast_default_opt_out = None;
            }
        }
        session_config
    }

    async fn fork_parent_title_from_app_server(
        &mut self,
        forked_from_id: Option<&str>,
    ) -> Option<String> {
        let forked_from_id = forked_from_id?;
        let forked_from_id = match ThreadId::from_string(forked_from_id) {
            Ok(thread_id) => thread_id,
            Err(err) => {
                tracing::warn!("Failed to parse fork parent thread id from app server: {err}");
                return None;
            }
        };

        match self
            .thread_read(forked_from_id, /*include_turns*/ false)
            .await
        {
            Ok(thread) => thread.name,
            Err(err) => {
                tracing::warn!("Failed to read fork parent metadata from app server: {err}");
                None
            }
        }
    }

    pub(crate) async fn thread_list(
        &mut self,
        params: ThreadListParams,
    ) -> Result<ThreadListResponse> {
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::ThreadList { request_id, params })
            .await
            .wrap_err("thread/list failed during TUI session lookup")
    }

    /// Lists thread ids that the app server currently holds in memory.
    ///
    /// Used by `App::backfill_loaded_subagent_threads` to discover subagent threads that were
    /// spawned before the TUI connected. The caller then fetches full metadata per thread via
    /// `thread_read` and walks the spawn tree.
    pub(crate) async fn thread_loaded_list(
        &mut self,
        params: ThreadLoadedListParams,
    ) -> Result<ThreadLoadedListResponse> {
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::ThreadLoadedList { request_id, params })
            .await
            .wrap_err("failed to list loaded threads from app server")
    }

    pub(crate) async fn thread_read(
        &mut self,
        thread_id: ThreadId,
        include_turns: bool,
    ) -> Result<Thread> {
        let request_id = self.next_request_id();
        let response: ThreadReadResponse = self
            .client
            .request_typed(ClientRequest::ThreadRead {
                request_id,
                params: ThreadReadParams {
                    thread_id: thread_id.to_string(),
                    include_turns,
                },
            })
            .await
            .wrap_err("thread/read failed during TUI session lookup")?;
        Ok(response.thread)
    }

    pub(crate) async fn thread_archive(&mut self, thread_id: ThreadId) -> Result<()> {
        let request_id = self.next_request_id();
        let _: ThreadArchiveResponse = self
            .client
            .request_typed(ClientRequest::ThreadArchive {
                request_id,
                params: ThreadArchiveParams {
                    thread_id: thread_id.to_string(),
                },
            })
            .await
            .wrap_err("failed to archive session")?;
        Ok(())
    }

    pub(crate) async fn thread_delete(&mut self, thread_id: ThreadId) -> Result<()> {
        let request_id = self.next_request_id();
        let _: ThreadDeleteResponse = self
            .client
            .request_typed(ClientRequest::ThreadDelete {
                request_id,
                params: ThreadDeleteParams {
                    thread_id: thread_id.to_string(),
                },
            })
            .await
            .wrap_err("failed to delete session")?;
        Ok(())
    }

    pub(crate) async fn thread_unarchive(&mut self, thread_id: ThreadId) -> Result<Thread> {
        let request_id = self.next_request_id();
        let response: ThreadUnarchiveResponse = self
            .client
            .request_typed(ClientRequest::ThreadUnarchive {
                request_id,
                params: ThreadUnarchiveParams {
                    thread_id: thread_id.to_string(),
                },
            })
            .await
            .wrap_err("failed to unarchive session")?;
        Ok(response.thread)
    }

    pub(crate) async fn thread_metadata_update_branch(
        &mut self,
        thread_id: ThreadId,
        branch: String,
    ) -> Result<ThreadMetadataUpdateResponse> {
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::ThreadMetadataUpdate {
                request_id,
                params: ThreadMetadataUpdateParams {
                    thread_id: thread_id.to_string(),
                    git_info: Some(ThreadMetadataGitInfoUpdateParams {
                        sha: None,
                        branch: Some(Some(branch)),
                        origin_url: None,
                    }),
                },
            })
            .await
            .wrap_err("thread/metadata/update failed while syncing git branch")
    }

    pub(crate) async fn thread_settings_update(
        &mut self,
        params: ThreadSettingsUpdateParams,
    ) -> Result<()> {
        if !self.thread_settings_update_supported {
            return Ok(());
        }
        let request_id = self.next_request_id();
        match self
            .client
            .request_typed::<ThreadSettingsUpdateResponse>(ClientRequest::ThreadSettingsUpdate {
                request_id,
                params,
            })
            .await
        {
            Ok(_) => Ok(()),
            Err(TypedRequestError::Server { source, .. })
                if is_thread_settings_update_unsupported(&source) =>
            {
                // Older remote app servers can reject this experimental method as
                // method-not-found, experimental-capability-gated, or an unknown
                // request variant. Treat those as a session-level capability
                // downgrade so local TUI setting changes stay best-effort instead
                // of showing an error every time the user changes model, effort,
                // personality, or mode.
                self.thread_settings_update_supported = false;
                Ok(())
            }
            Err(err) => Err(err).wrap_err("thread/settings/update failed in TUI"),
        }
    }

    pub(crate) async fn thread_inject_items(
        &mut self,
        thread_id: ThreadId,
        items: Vec<ResponseItem>,
    ) -> Result<ThreadInjectItemsResponse> {
        let items = items
            .into_iter()
            .map(serde_json::to_value)
            .collect::<std::result::Result<Vec<_>, _>>()
            .wrap_err("failed to encode thread/inject_items payload")?;
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::ThreadInjectItems {
                request_id,
                params: ThreadInjectItemsParams {
                    thread_id: thread_id.to_string(),
                    items,
                },
            })
            .await
            .wrap_err("thread/inject_items failed during TUI side conversation setup")
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn turn_start(
        &mut self,
        thread_id: ThreadId,
        items: Vec<UserInput>,
        cwd: PathBuf,
        approval_policy: AskForApproval,
        approvals_reviewer: codex_protocol::config_types::ApprovalsReviewer,
        permissions_override: TurnPermissionsOverride,
        workspace_roots: &[AbsolutePathBuf],
        model: String,
        effort: Option<codex_protocol::openai_models::ReasoningEffort>,
        summary: Option<codex_protocol::config_types::ReasoningSummary>,
        service_tier: Option<Option<String>>,
        collaboration_mode: Option<codex_protocol::config_types::CollaborationMode>,
        personality: Option<codex_protocol::config_types::Personality>,
        output_schema: Option<serde_json::Value>,
    ) -> Result<TurnStartResponse> {
        let request_id = self.next_request_id();
        let (sandbox_policy, permissions) =
            turn_permissions_overrides(permissions_override, cwd.as_path());
        self.client
            .request_typed(ClientRequest::TurnStart {
                request_id,
                params: TurnStartParams {
                    thread_id: thread_id.to_string(),
                    client_user_message_id: None,
                    input: items,
                    responsesapi_client_metadata: None,
                    additional_context: None,
                    environments: None,
                    cwd: Some(cwd),
                    runtime_workspace_roots: Some(workspace_roots.to_vec()),
                    approval_policy: Some(approval_policy),
                    approvals_reviewer: Some(approvals_reviewer.into()),
                    sandbox_policy,
                    permissions,
                    model: Some(model),
                    service_tier,
                    effort,
                    summary,
                    personality,
                    output_schema,
                    collaboration_mode,
                    multi_agent_mode: None,
                },
            })
            .await
            .wrap_err("turn/start failed in TUI")
    }

    pub(crate) async fn turn_interrupt(
        &mut self,
        thread_id: ThreadId,
        turn_id: String,
    ) -> std::result::Result<(), TypedRequestError> {
        let request_id = self.next_request_id();
        let _: TurnInterruptResponse = self
            .client
            .request_typed(ClientRequest::TurnInterrupt {
                request_id,
                params: TurnInterruptParams {
                    thread_id: thread_id.to_string(),
                    turn_id,
                },
            })
            .await?;
        Ok(())
    }

    pub(crate) async fn startup_interrupt(
        &mut self,
        thread_id: ThreadId,
    ) -> std::result::Result<(), TypedRequestError> {
        self.turn_interrupt(thread_id, String::new()).await
    }

    pub(crate) async fn turn_steer(
        &mut self,
        thread_id: ThreadId,
        turn_id: String,
        items: Vec<UserInput>,
    ) -> std::result::Result<TurnSteerResponse, TypedRequestError> {
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::TurnSteer {
                request_id,
                params: TurnSteerParams {
                    thread_id: thread_id.to_string(),
                    client_user_message_id: None,
                    input: items,
                    responsesapi_client_metadata: None,
                    additional_context: None,
                    expected_turn_id: turn_id,
                },
            })
            .await
    }

    pub(crate) async fn thread_set_name(
        &mut self,
        thread_id: ThreadId,
        name: String,
    ) -> Result<()> {
        let request_id = self.next_request_id();
        let _: ThreadSetNameResponse = self
            .client
            .request_typed(ClientRequest::ThreadSetName {
                request_id,
                params: ThreadSetNameParams {
                    thread_id: thread_id.to_string(),
                    name,
                },
            })
            .await
            .wrap_err("thread/name/set failed in TUI")?;
        Ok(())
    }

    pub(crate) async fn thread_memory_mode_set(
        &mut self,
        thread_id: ThreadId,
        mode: ThreadMemoryMode,
    ) -> Result<()> {
        let request_id = self.next_request_id();
        let _: ThreadMemoryModeSetResponse = self
            .client
            .request_typed(ClientRequest::ThreadMemoryModeSet {
                request_id,
                params: ThreadMemoryModeSetParams {
                    thread_id: thread_id.to_string(),
                    mode,
                },
            })
            .await
            .wrap_err("thread/memoryMode/set failed in TUI")?;
        Ok(())
    }

    pub(crate) async fn memory_reset(&mut self) -> Result<()> {
        let request_id = self.next_request_id();
        let _: MemoryResetResponse = self
            .client
            .request_typed(ClientRequest::MemoryReset {
                request_id,
                params: None,
            })
            .await
            .wrap_err("memory/reset failed in TUI")?;
        Ok(())
    }

    pub(crate) async fn thread_goal_get(
        &mut self,
        thread_id: ThreadId,
    ) -> Result<ThreadGoalGetResponse> {
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::ThreadGoalGet {
                request_id,
                params: ThreadGoalGetParams {
                    thread_id: thread_id.to_string(),
                },
            })
            .await
            .wrap_err("thread/goal/get failed in TUI")
    }

    pub(crate) async fn thread_goal_set(
        &mut self,
        thread_id: ThreadId,
        objective: Option<String>,
        status: Option<ThreadGoalStatus>,
        token_budget: Option<Option<i64>>,
    ) -> Result<ThreadGoalSetResponse> {
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::ThreadGoalSet {
                request_id,
                params: ThreadGoalSetParams {
                    thread_id: thread_id.to_string(),
                    objective,
                    status,
                    token_budget,
                },
            })
            .await
            .wrap_err("thread/goal/set failed in TUI")
    }

    pub(crate) async fn thread_goal_clear(
        &mut self,
        thread_id: ThreadId,
    ) -> Result<ThreadGoalClearResponse> {
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::ThreadGoalClear {
                request_id,
                params: ThreadGoalClearParams {
                    thread_id: thread_id.to_string(),
                },
            })
            .await
            .wrap_err("thread/goal/clear failed in TUI")
    }

    pub(crate) async fn logout_account(&mut self) -> Result<()> {
        let request_id = self.next_request_id();
        let _: LogoutAccountResponse = self
            .client
            .request_typed(ClientRequest::LogoutAccount {
                request_id,
                params: None,
            })
            .await
            .wrap_err("account/logout failed in TUI")?;
        Ok(())
    }

    pub(crate) async fn thread_unsubscribe(&mut self, thread_id: ThreadId) -> Result<()> {
        let request_id = self.next_request_id();
        let _: ThreadUnsubscribeResponse = self
            .client
            .request_typed(ClientRequest::ThreadUnsubscribe {
                request_id,
                params: ThreadUnsubscribeParams {
                    thread_id: thread_id.to_string(),
                },
            })
            .await
            .wrap_err("thread/unsubscribe failed in TUI")?;
        Ok(())
    }

    pub(crate) async fn thread_compact_start(&mut self, thread_id: ThreadId) -> Result<()> {
        let request_id = self.next_request_id();
        let _: ThreadCompactStartResponse = self
            .client
            .request_typed(ClientRequest::ThreadCompactStart {
                request_id,
                params: ThreadCompactStartParams {
                    thread_id: thread_id.to_string(),
                },
            })
            .await
            .wrap_err("thread/compact/start failed in TUI")?;
        Ok(())
    }

    pub(crate) async fn thread_shell_command(
        &mut self,
        thread_id: ThreadId,
        command: String,
    ) -> Result<()> {
        let request_id = self.next_request_id();
        let _: ThreadShellCommandResponse = self
            .client
            .request_typed(ClientRequest::ThreadShellCommand {
                request_id,
                params: ThreadShellCommandParams {
                    thread_id: thread_id.to_string(),
                    command,
                },
            })
            .await
            .wrap_err("thread/shellCommand failed in TUI")?;
        Ok(())
    }

    pub(crate) async fn thread_approve_guardian_denied_action(
        &mut self,
        thread_id: ThreadId,
        event: &GuardianAssessmentEvent,
    ) -> Result<()> {
        let request_id = self.next_request_id();
        let _: ThreadApproveGuardianDeniedActionResponse = self
            .client
            .request_typed(ClientRequest::ThreadApproveGuardianDeniedAction {
                request_id,
                params: ThreadApproveGuardianDeniedActionParams {
                    thread_id: thread_id.to_string(),
                    event: serde_json::to_value(event)
                        .wrap_err("failed to serialize Auto Review denial event")?,
                },
            })
            .await
            .wrap_err("thread/approveGuardianDeniedAction failed in TUI")?;
        Ok(())
    }

    pub(crate) async fn thread_background_terminals_clean(
        &mut self,
        thread_id: ThreadId,
    ) -> Result<()> {
        let request_id = self.next_request_id();
        let _: ThreadBackgroundTerminalsCleanResponse = self
            .client
            .request_typed(ClientRequest::ThreadBackgroundTerminalsClean {
                request_id,
                params: ThreadBackgroundTerminalsCleanParams {
                    thread_id: thread_id.to_string(),
                },
            })
            .await
            .wrap_err("thread/backgroundTerminals/clean failed in TUI")?;
        Ok(())
    }

    pub(crate) async fn thread_rollback(
        &mut self,
        thread_id: ThreadId,
        num_turns: u32,
    ) -> Result<ThreadRollbackResponse> {
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::ThreadRollback {
                request_id,
                params: ThreadRollbackParams {
                    thread_id: thread_id.to_string(),
                    num_turns,
                },
            })
            .await
            .wrap_err("thread/rollback failed in TUI")
    }

    pub(crate) async fn review_start(
        &mut self,
        thread_id: ThreadId,
        target: ReviewTarget,
    ) -> Result<ReviewStartResponse> {
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::ReviewStart {
                request_id,
                params: ReviewStartParams {
                    thread_id: thread_id.to_string(),
                    target,
                    delivery: Some(ReviewDelivery::Inline),
                },
            })
            .await
            .wrap_err("review/start failed in TUI")
    }

    pub(crate) async fn skills_list(
        &mut self,
        params: SkillsListParams,
    ) -> Result<SkillsListResponse> {
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::SkillsList { request_id, params })
            .await
            .wrap_err("skills/list failed in TUI")
    }

    pub(crate) async fn reload_user_config(&mut self) -> Result<()> {
        let request_id = self.next_request_id();
        let _: ConfigWriteResponse = self
            .client
            .request_typed(ClientRequest::ConfigBatchWrite {
                request_id,
                params: ConfigBatchWriteParams {
                    edits: Vec::new(),
                    file_path: None,
                    expected_version: None,
                    reload_user_config: true,
                },
            })
            .await
            .wrap_err("config/batchWrite failed while reloading user config in TUI")?;
        Ok(())
    }

    pub(crate) async fn reject_server_request(
        &self,
        request_id: RequestId,
        error: JSONRPCErrorError,
    ) -> std::io::Result<()> {
        self.client.reject_server_request(request_id, error).await
    }

    pub(crate) async fn resolve_server_request(
        &self,
        request_id: RequestId,
        result: serde_json::Value,
    ) -> std::io::Result<()> {
        self.client.resolve_server_request(request_id, result).await
    }

    pub(crate) async fn shutdown(self) -> std::io::Result<()> {
        self.client.shutdown().await
    }

    pub(crate) fn request_handle(&self) -> AppServerRequestHandle {
        self.client.request_handle()
    }

    fn next_request_id(&mut self) -> RequestId {
        let request_id = self.next_request_id;
        self.next_request_id += 1;
        RequestId::Integer(request_id)
    }
}

pub(crate) async fn start_thread_with_request_handle(
    request_handle: AppServerRequestHandle,
    config: Config,
    thread_params_mode: ThreadParamsMode,
    remote_cwd_override: Option<PathBuf>,
) -> Result<AppServerStartedThread> {
    let response: ThreadStartResponse = request_handle
        .request_typed(ClientRequest::ThreadStart {
            request_id: RequestId::String(format!("startup-thread-start-{}", Uuid::new_v4())),
            params: thread_start_params_from_config(
                &config,
                thread_params_mode,
                remote_cwd_override.as_deref(),
                /*session_start_source*/ None,
            ),
        })
        .await
        .map_err(|err| bootstrap_request_error("thread/start failed during TUI bootstrap", err))?;
    started_thread_from_start_response(response, &config, thread_params_mode).await
}

pub(crate) fn status_account_display_from_auth_mode(
    auth_mode: Option<AuthMode>,
    plan_type: Option<codex_protocol::account::PlanType>,
) -> Option<StatusAccountDisplay> {
    match auth_mode {
        Some(AuthMode::ApiKey) => Some(StatusAccountDisplay::ApiKey),
        Some(AuthMode::Chatgpt)
        | Some(AuthMode::ChatgptAuthTokens)
        | Some(AuthMode::AgentIdentity)
        | Some(AuthMode::PersonalAccessToken) => Some(StatusAccountDisplay::ChatGpt {
            email: None,
            plan: plan_type.map(plan_type_display_name),
        }),
        Some(AuthMode::Headers) | Some(AuthMode::BedrockApiKey) => None,
        None => None,
    }
}

fn model_preset_from_api_model(model: ApiModel) -> ModelPreset {
    let upgrade = model.upgrade.map(|upgrade_id| {
        let upgrade_info = model.upgrade_info.clone();
        ModelUpgrade {
            id: upgrade_id,
            migration_config_key: model.model.clone(),
            model_link: upgrade_info
                .as_ref()
                .and_then(|info| info.model_link.clone()),
            upgrade_copy: upgrade_info
                .as_ref()
                .and_then(|info| info.upgrade_copy.clone()),
            migration_markdown: upgrade_info.and_then(|info| info.migration_markdown),
        }
    });

    ModelPreset {
        id: model.id,
        model: model.model,
        display_name: model.display_name,
        description: model.description,
        default_reasoning_effort: model.default_reasoning_effort,
        supported_reasoning_efforts: model
            .supported_reasoning_efforts
            .into_iter()
            .map(|effort| ReasoningEffortPreset {
                effort: effort.reasoning_effort,
                description: effort.description,
            })
            .collect(),
        supports_personality: model.supports_personality,
        additional_speed_tiers: model.additional_speed_tiers,
        service_tiers: model
            .service_tiers
            .into_iter()
            .map(|service_tier| ModelServiceTier {
                id: service_tier.id,
                name: service_tier.name,
                description: service_tier.description,
            })
            .collect(),
        default_service_tier: model.default_service_tier,
        is_default: model.is_default,
        upgrade,
        show_in_picker: !model.hidden,
        multi_agent_version: None,
        availability_nux: model.availability_nux.map(|nux| ModelAvailabilityNux {
            message: nux.message,
        }),
        // `model/list` already returns models filtered for the active client/auth context.
        supported_in_api: true,
        input_modalities: model.input_modalities,
    }
}

fn approvals_reviewer_override_from_config(
    config: &Config,
) -> Option<codex_app_server_protocol::ApprovalsReviewer> {
    Some(config.approvals_reviewer.into())
}

fn config_request_overrides_from_config(
    config: &Config,
) -> Option<HashMap<String, serde_json::Value>> {
    let mut overrides = HashMap::new();
    let mut insert = |key: &str, value: Option<String>| {
        if let Some(value) = value {
            overrides.insert(key.to_string(), serde_json::Value::String(value));
        }
    };
    insert(
        "model_reasoning_effort",
        config
            .model_reasoning_effort
            .as_ref()
            .map(std::string::ToString::to_string),
    );
    insert(
        "model_reasoning_summary",
        config
            .model_reasoning_summary
            .map(|summary| summary.to_string()),
    );
    insert(
        "model_verbosity",
        config
            .model_verbosity
            .map(|verbosity| verbosity.to_string()),
    );
    insert(
        "personality",
        config
            .personality
            .map(|personality| personality.to_string()),
    );
    insert(
        "web_search",
        Some(config.web_search_mode.value().to_string()),
    );
    if config.bypass_hook_trust {
        overrides.insert("bypass_hook_trust".to_string(), true.into());
    }
    Some(overrides)
}

fn service_tier_override_from_config(config: &Config) -> Option<Option<String>> {
    config.service_tier.clone().map(Some).or_else(|| {
        (config.notices.fast_default_opt_out == Some(true))
            .then(|| Some(SERVICE_TIER_DEFAULT_REQUEST_VALUE.to_string()))
    })
}

fn sandbox_mode_from_permission_profile(
    permission_profile: &PermissionProfile,
    cwd: &std::path::Path,
) -> Option<codex_app_server_protocol::SandboxMode> {
    match permission_profile {
        PermissionProfile::Disabled => {
            Some(codex_app_server_protocol::SandboxMode::DangerFullAccess)
        }
        PermissionProfile::External { .. } => None,
        PermissionProfile::Managed { .. } => {
            let file_system_policy = permission_profile.file_system_sandbox_policy();
            if file_system_policy.has_full_disk_write_access() {
                permission_profile
                    .network_sandbox_policy()
                    .is_enabled()
                    .then_some(codex_app_server_protocol::SandboxMode::DangerFullAccess)
            } else if file_system_policy.can_write_path_with_cwd(cwd, cwd) {
                Some(codex_app_server_protocol::SandboxMode::WorkspaceWrite)
            } else {
                Some(codex_app_server_protocol::SandboxMode::ReadOnly)
            }
        }
    }
}

fn permission_profile_id_from_active_profile(active: ActivePermissionProfile) -> String {
    active.id
}

fn turn_permissions_overrides(
    permissions_override: TurnPermissionsOverride,
    cwd: &std::path::Path,
) -> (
    Option<codex_app_server_protocol::SandboxPolicy>,
    Option<String>,
) {
    match permissions_override {
        TurnPermissionsOverride::Preserve => (None, None),
        TurnPermissionsOverride::ActiveProfile(active_permission_profile) => (
            None,
            Some(permission_profile_id_from_active_profile(
                active_permission_profile,
            )),
        ),
        TurnPermissionsOverride::LegacySandbox(permission_profile) => {
            let legacy_profile = legacy_compatible_permission_profile(&permission_profile, cwd);
            let policy = legacy_profile
                .to_legacy_sandbox_policy(cwd)
                .unwrap_or_else(|err| {
                    unreachable!(
                        "legacy-compatible permissions must project to legacy policy: {err}"
                    )
                });
            (Some(policy.into()), None)
        }
    }
}

fn permissions_selection_from_config(
    config: &Config,
    thread_params_mode: ThreadParamsMode,
) -> Option<String> {
    if matches!(thread_params_mode, ThreadParamsMode::Remote) {
        return None;
    }

    config
        .permissions
        .active_permission_profile()
        .map(permission_profile_id_from_active_profile)
}

fn thread_start_params_from_config(
    config: &Config,
    thread_params_mode: ThreadParamsMode,
    remote_cwd_override: Option<&std::path::Path>,
    session_start_source: Option<ThreadStartSource>,
) -> ThreadStartParams {
    let permissions = permissions_selection_from_config(config, thread_params_mode);
    let sandbox = permissions
        .is_none()
        .then(|| {
            sandbox_mode_from_permission_profile(
                &config.permissions.effective_permission_profile(),
                config.cwd.as_path(),
            )
        })
        .flatten();
    ThreadStartParams {
        model: config.model.clone(),
        model_provider: thread_params_mode.model_provider_from_config(config),
        service_tier: service_tier_override_from_config(config),
        cwd: thread_cwd_from_config(config, thread_params_mode, remote_cwd_override),
        runtime_workspace_roots: Some(config.workspace_roots.clone()),
        approval_policy: Some(config.permissions.approval_policy.value().into()),
        approvals_reviewer: approvals_reviewer_override_from_config(config),
        sandbox,
        permissions,
        config: config_request_overrides_from_config(config),
        ephemeral: Some(config.ephemeral),
        session_start_source,
        thread_source: Some(ThreadSource::User),
        developer_instructions: with_terminal_visualization_instructions(
            config, /*control_instructions*/ None,
        ),
        ..ThreadStartParams::default()
    }
}

fn thread_resume_params_from_config(
    config: Config,
    thread_id: ThreadId,
    thread_params_mode: ThreadParamsMode,
    remote_cwd_override: Option<&std::path::Path>,
    model_settings: ResumeModelSettings,
) -> ThreadResumeParams {
    let permissions = permissions_selection_from_config(&config, thread_params_mode);
    let sandbox = permissions
        .is_none()
        .then(|| {
            sandbox_mode_from_permission_profile(
                &config.permissions.effective_permission_profile(),
                config.cwd.as_path(),
            )
        })
        .flatten();
    let mut config_overrides = config_request_overrides_from_config(&config);
    if model_settings == ResumeModelSettings::RestoreFromThread
        && let Some(overrides) = config_overrides.as_mut()
    {
        overrides.remove("model_reasoning_effort");
        if overrides.is_empty() {
            config_overrides = None;
        }
    }
    let (model, model_provider) = match model_settings {
        ResumeModelSettings::OverrideFromCurrentConfig => (
            config.model.clone(),
            thread_params_mode.model_provider_from_config(&config),
        ),
        ResumeModelSettings::RestoreFromThread => (None, None),
    };
    ThreadResumeParams {
        thread_id: thread_id.to_string(),
        model,
        model_provider,
        service_tier: service_tier_override_from_config(&config),
        cwd: thread_cwd_from_config(&config, thread_params_mode, remote_cwd_override),
        runtime_workspace_roots: Some(config.workspace_roots.clone()),
        approval_policy: Some(config.permissions.approval_policy.value().into()),
        approvals_reviewer: approvals_reviewer_override_from_config(&config),
        sandbox,
        permissions,
        config: config_overrides,
        developer_instructions: with_terminal_visualization_instructions(
            &config, /*control_instructions*/ None,
        ),
        ..ThreadResumeParams::default()
    }
}

fn thread_fork_params_from_config(
    config: Config,
    thread_id: ThreadId,
    thread_params_mode: ThreadParamsMode,
    remote_cwd_override: Option<&std::path::Path>,
) -> ThreadForkParams {
    let permissions = permissions_selection_from_config(&config, thread_params_mode);
    let sandbox = permissions
        .is_none()
        .then(|| {
            sandbox_mode_from_permission_profile(
                &config.permissions.effective_permission_profile(),
                config.cwd.as_path(),
            )
        })
        .flatten();
    ThreadForkParams {
        thread_id: thread_id.to_string(),
        model: config.model.clone(),
        model_provider: thread_params_mode.model_provider_from_config(&config),
        service_tier: service_tier_override_from_config(&config),
        cwd: thread_cwd_from_config(&config, thread_params_mode, remote_cwd_override),
        runtime_workspace_roots: Some(config.workspace_roots.clone()),
        approval_policy: Some(config.permissions.approval_policy.value().into()),
        approvals_reviewer: approvals_reviewer_override_from_config(&config),
        sandbox,
        permissions,
        config: config_request_overrides_from_config(&config),
        base_instructions: config.base_instructions.clone(),
        developer_instructions: with_terminal_visualization_instructions(
            &config,
            config.developer_instructions.clone(),
        ),
        ephemeral: config.ephemeral,
        thread_source: Some(ThreadSource::User),
        ..ThreadForkParams::default()
    }
}

fn thread_cwd_from_config(
    config: &Config,
    thread_params_mode: ThreadParamsMode,
    remote_cwd_override: Option<&std::path::Path>,
) -> Option<String> {
    match thread_params_mode {
        ThreadParamsMode::Embedded => Some(config.cwd.to_string_lossy().to_string()),
        ThreadParamsMode::Remote => {
            remote_cwd_override.map(|cwd| cwd.to_string_lossy().to_string())
        }
    }
}

async fn started_thread_from_start_response(
    response: ThreadStartResponse,
    config: &Config,
    thread_params_mode: ThreadParamsMode,
) -> Result<AppServerStartedThread> {
    let session =
        thread_session_state_from_thread_start_response(&response, config, thread_params_mode)
            .await
            .map_err(color_eyre::eyre::Report::msg)?;
    Ok(AppServerStartedThread {
        session,
        turns: response.thread.turns,
    })
}

async fn started_thread_from_resume_response(
    response: ThreadResumeResponse,
    config: &Config,
    thread_params_mode: ThreadParamsMode,
) -> Result<AppServerStartedThread> {
    let session =
        thread_session_state_from_thread_resume_response(&response, config, thread_params_mode)
            .await
            .map_err(color_eyre::eyre::Report::msg)?;
    Ok(AppServerStartedThread {
        session,
        turns: response.thread.turns,
    })
}

async fn started_thread_from_fork_response(
    response: ThreadForkResponse,
    config: &Config,
    thread_params_mode: ThreadParamsMode,
) -> Result<AppServerStartedThread> {
    let session =
        thread_session_state_from_thread_fork_response(&response, config, thread_params_mode)
            .await
            .map_err(color_eyre::eyre::Report::msg)?;
    Ok(AppServerStartedThread {
        session,
        turns: response.thread.turns,
    })
}

async fn thread_session_state_from_thread_start_response(
    response: &ThreadStartResponse,
    config: &Config,
    thread_params_mode: ThreadParamsMode,
) -> Result<ThreadSessionState, String> {
    let permission_profile = display_permission_profile_from_thread_response(
        &response.sandbox,
        response.cwd.as_path(),
        config,
        thread_params_mode,
    );
    thread_session_state_from_thread_response(
        &response.thread.id,
        response.thread.forked_from_id.clone(),
        response.thread.name.clone(),
        response.thread.path.clone(),
        response.model.clone(),
        response.model_provider.clone(),
        response.service_tier.clone(),
        response.approval_policy,
        response.approvals_reviewer.to_core(),
        permission_profile,
        response.active_permission_profile.clone().map(Into::into),
        response.cwd.clone(),
        response.runtime_workspace_roots.clone(),
        response.instruction_source_path_uris(),
        response.reasoning_effort.clone(),
        config,
    )
    .await
}

async fn thread_session_state_from_thread_resume_response(
    response: &ThreadResumeResponse,
    config: &Config,
    thread_params_mode: ThreadParamsMode,
) -> Result<ThreadSessionState, String> {
    let permission_profile = if matches!(thread_params_mode, ThreadParamsMode::Embedded)
        && response.active_permission_profile.is_none()
    {
        PermissionProfile::from_legacy_sandbox_policy_for_cwd(
            &response.sandbox.to_core(),
            response.cwd.as_path(),
        )
    } else {
        display_permission_profile_from_thread_response(
            &response.sandbox,
            response.cwd.as_path(),
            config,
            thread_params_mode,
        )
    };
    thread_session_state_from_thread_response(
        &response.thread.id,
        response.thread.forked_from_id.clone(),
        response.thread.name.clone(),
        response.thread.path.clone(),
        response.model.clone(),
        response.model_provider.clone(),
        response.service_tier.clone(),
        response.approval_policy,
        response.approvals_reviewer.to_core(),
        permission_profile,
        response.active_permission_profile.clone().map(Into::into),
        response.cwd.clone(),
        response.runtime_workspace_roots.clone(),
        response.instruction_source_path_uris(),
        response.reasoning_effort.clone(),
        config,
    )
    .await
}

async fn thread_session_state_from_thread_fork_response(
    response: &ThreadForkResponse,
    config: &Config,
    thread_params_mode: ThreadParamsMode,
) -> Result<ThreadSessionState, String> {
    let permission_profile = display_permission_profile_from_thread_response(
        &response.sandbox,
        response.cwd.as_path(),
        config,
        thread_params_mode,
    );
    thread_session_state_from_thread_response(
        &response.thread.id,
        response.thread.forked_from_id.clone(),
        response.thread.name.clone(),
        response.thread.path.clone(),
        response.model.clone(),
        response.model_provider.clone(),
        response.service_tier.clone(),
        response.approval_policy,
        response.approvals_reviewer.to_core(),
        permission_profile,
        response.active_permission_profile.clone().map(Into::into),
        response.cwd.clone(),
        response.runtime_workspace_roots.clone(),
        response.instruction_source_path_uris(),
        response.reasoning_effort.clone(),
        config,
    )
    .await
}

fn display_permission_profile_from_thread_response(
    sandbox: &codex_app_server_protocol::SandboxPolicy,
    cwd: &std::path::Path,
    config: &Config,
    thread_params_mode: ThreadParamsMode,
) -> PermissionProfile {
    match thread_params_mode {
        ThreadParamsMode::Embedded => config.permissions.effective_permission_profile(),
        ThreadParamsMode::Remote => {
            PermissionProfile::from_legacy_sandbox_policy_for_cwd(&sandbox.to_core(), cwd)
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "session mapping keeps explicit fields"
)]
async fn thread_session_state_from_thread_response(
    thread_id: &str,
    forked_from_id: Option<String>,
    thread_name: Option<String>,
    rollout_path: Option<PathBuf>,
    model: String,
    model_provider_id: String,
    service_tier: Option<String>,
    approval_policy: AskForApproval,
    approvals_reviewer: codex_protocol::config_types::ApprovalsReviewer,
    permission_profile: PermissionProfile,
    active_permission_profile: Option<ActivePermissionProfile>,
    cwd: AbsolutePathBuf,
    runtime_workspace_roots: Vec<AbsolutePathBuf>,
    instruction_source_paths: Vec<PathUri>,
    reasoning_effort: Option<codex_protocol::openai_models::ReasoningEffort>,
    config: &Config,
) -> Result<ThreadSessionState, String> {
    let thread_id = ThreadId::from_string(thread_id)
        .map_err(|err| format!("thread id `{thread_id}` is invalid: {err}"))?;
    let forked_from_id = forked_from_id
        .as_deref()
        .map(ThreadId::from_string)
        .transpose()
        .map_err(|err| format!("forked_from_id is invalid: {err}"))?;
    let history_config =
        codex_message_history::HistoryConfig::new(config.codex_home.clone(), &config.history);
    let (log_id, entry_count) = codex_message_history::history_metadata(&history_config).await;
    Ok(ThreadSessionState {
        thread_id,
        forked_from_id,
        fork_parent_title: None,
        thread_name,
        model,
        model_provider_id,
        service_tier,
        approval_policy,
        approvals_reviewer,
        permission_profile,
        active_permission_profile,
        cwd,
        runtime_workspace_roots,
        instruction_source_paths,
        reasoning_effort,
        collaboration_mode: None,
        personality: config.personality,
        message_history: Some(MessageHistoryMetadata {
            log_id,
            entry_count,
        }),
        network_proxy: None,
        rollout_path,
    })
}

pub(crate) fn app_server_rate_limit_snapshots(
    response: GetAccountRateLimitsResponse,
) -> Vec<RateLimitSnapshot> {
    let primary_limit_id = response.rate_limits.limit_id.clone();
    let mut snapshots = vec![response.rate_limits];
    if let Some(by_limit_id) = response.rate_limits_by_limit_id {
        snapshots.extend(by_limit_id.into_iter().filter_map(|(limit_id, snapshot)| {
            if primary_limit_id.as_deref().is_some_and(|primary_limit_id| {
                primary_limit_id == limit_id
                    || Some(primary_limit_id) == snapshot.limit_id.as_deref()
            }) {
                None
            } else {
                Some(snapshot)
            }
        }));
    }
    snapshots
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::legacy_core::config::ConfigBuilder;
    use crate::legacy_core::config::ConfigOverrides;
    use app_test_support::create_fake_rollout;
    use codex_app_server_protocol::ThreadStatus;
    use codex_app_server_protocol::Turn;
    use codex_app_server_protocol::TurnStatus;
    use codex_features::Feature;
    use codex_protocol::config_types::Personality;
    use codex_protocol::config_types::ReasoningSummary;
    use codex_protocol::config_types::ServiceTier;
    use codex_protocol::config_types::Verbosity;
    use codex_protocol::config_types::WebSearchMode;
    use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_READ_ONLY;
    use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_WORKSPACE;
    use codex_protocol::models::ManagedFileSystemPermissions;
    use codex_protocol::openai_models::ModelServiceTier;
    use codex_protocol::openai_models::ReasoningEffort;
    use codex_protocol::permissions::FileSystemAccessMode;
    use codex_protocol::permissions::FileSystemPath;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use codex_protocol::permissions::FileSystemSpecialPath;
    use codex_protocol::permissions::NetworkSandboxPolicy;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use codex_utils_path_uri::LegacyAppPathString;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    async fn build_config(temp_dir: &TempDir) -> Config {
        ConfigBuilder::default()
            .codex_home(temp_dir.path().to_path_buf())
            .build()
            .await
            .expect("config should build")
    }

    fn rate_limit_snapshot(limit_id: &str) -> RateLimitSnapshot {
        RateLimitSnapshot {
            limit_id: Some(limit_id.to_string()),
            limit_name: None,
            primary: Some(codex_app_server_protocol::RateLimitWindow {
                used_percent: 0,
                window_duration_mins: Some(10_080),
                resets_at: None,
            }),
            secondary: None,
            credits: None,
            individual_limit: None,
            plan_type: None,
            rate_limit_reached_type: None,
        }
    }

    #[test]
    fn app_server_rate_limit_snapshots_deduplicates_top_level_limit_from_map() {
        let response = GetAccountRateLimitsResponse {
            rate_limits: rate_limit_snapshot("codex"),
            rate_limits_by_limit_id: Some(HashMap::from([
                ("codex".to_string(), rate_limit_snapshot("codex")),
                ("other".to_string(), rate_limit_snapshot("other")),
            ])),
            rate_limit_reset_credits: None,
        };

        let snapshots = app_server_rate_limit_snapshots(response);

        assert_eq!(
            snapshots
                .iter()
                .map(|snapshot| snapshot.limit_id.as_deref())
                .collect::<Vec<_>>(),
            vec![Some("codex"), Some("other")]
        );
    }

    #[test]
    fn thread_settings_update_compat_detects_unsupported_errors() {
        let cases = [
            (JSONRPC_METHOD_NOT_FOUND, "method not found", true),
            (
                JSONRPC_INVALID_REQUEST,
                "thread/settings/update requires experimentalApi capability",
                true,
            ),
            (
                JSONRPC_INVALID_REQUEST,
                "Invalid request: unknown variant `thread/settings/update`",
                true,
            ),
            (JSONRPC_INVALID_REQUEST, "invalid thread id", false),
        ];

        for (code, message, expected) in cases {
            let source = JSONRPCErrorError {
                code,
                data: None,
                message: message.to_string(),
            };
            assert_eq!(
                is_thread_settings_update_unsupported(&source),
                expected,
                "{message}"
            );
        }
    }

    #[tokio::test]
    async fn thread_start_params_include_cwd_for_embedded_sessions() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config = ConfigBuilder::default()
            .codex_home(temp_dir.path().to_path_buf())
            .harness_overrides(ConfigOverrides {
                default_permissions: Some(BUILT_IN_PERMISSION_PROFILE_WORKSPACE.to_string()),
                ..ConfigOverrides::default()
            })
            .build()
            .await
            .expect("config should build");

        let params = thread_start_params_from_config(
            &config,
            ThreadParamsMode::Embedded,
            /*remote_cwd_override*/ None,
            /*session_start_source*/ None,
        );

        assert_eq!(params.cwd, Some(config.cwd.to_string_lossy().to_string()));
        assert_eq!(
            params.runtime_workspace_roots,
            Some(config.workspace_roots.clone())
        );
        assert_eq!(params.sandbox, None);
        assert_eq!(
            params.permissions,
            config
                .permissions
                .active_permission_profile()
                .map(permission_profile_id_from_active_profile)
        );
        assert_eq!(params.model_provider, Some(config.model_provider_id));
        assert_eq!(params.thread_source, Some(ThreadSource::User));
    }

    #[tokio::test]
    async fn thread_start_params_can_mark_clear_source() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config = build_config(&temp_dir).await;

        let params = thread_start_params_from_config(
            &config,
            ThreadParamsMode::Embedded,
            /*remote_cwd_override*/ None,
            Some(ThreadStartSource::Clear),
        );

        assert_eq!(params.session_start_source, Some(ThreadStartSource::Clear));
    }

    #[test]
    fn embedded_turn_permissions_use_active_profile_selection() {
        let cwd = test_path_buf("/workspace/project").abs();
        let active_permission_profile =
            ActivePermissionProfile::new(BUILT_IN_PERMISSION_PROFILE_WORKSPACE);
        let expected_permissions =
            permission_profile_id_from_active_profile(active_permission_profile.clone());

        let (sandbox_policy, permissions) = turn_permissions_overrides(
            TurnPermissionsOverride::ActiveProfile(active_permission_profile),
            cwd.as_path(),
        );

        assert_eq!(sandbox_policy, None);
        assert_eq!(permissions, Some(expected_permissions));
    }

    #[test]
    fn embedded_turn_permissions_select_profile_id_only() {
        let cwd = test_path_buf("/workspace/project").abs();
        let active_permission_profile =
            ActivePermissionProfile::new(BUILT_IN_PERMISSION_PROFILE_WORKSPACE);

        let (sandbox_policy, permissions) = turn_permissions_overrides(
            TurnPermissionsOverride::ActiveProfile(active_permission_profile),
            cwd.as_path(),
        );

        assert_eq!(sandbox_policy, None);
        assert_eq!(
            permissions,
            Some(BUILT_IN_PERMISSION_PROFILE_WORKSPACE.to_string())
        );
    }

    #[test]
    fn turn_permissions_preserve_thread_permissions_without_override() {
        let cwd = test_path_buf("/workspace/project").abs();

        let (sandbox_policy, permissions) =
            turn_permissions_overrides(TurnPermissionsOverride::Preserve, cwd.as_path());

        assert_eq!(sandbox_policy, None);
        assert_eq!(permissions, None);
    }

    #[test]
    fn legacy_turn_permissions_project_to_sandbox_when_explicitly_overridden() {
        let cwd = test_path_buf("/workspace/project").abs();

        let (sandbox_policy, permissions) = turn_permissions_overrides(
            TurnPermissionsOverride::LegacySandbox(PermissionProfile::read_only()),
            cwd.as_path(),
        );

        assert_eq!(
            sandbox_policy,
            Some(codex_app_server_protocol::SandboxPolicy::ReadOnly {
                network_access: false
            })
        );
        assert_eq!(permissions, None);
    }

    #[test]
    fn remote_turn_permissions_preserve_active_profile_selection() {
        let cwd = test_path_buf("/workspace/project").abs();
        let active_permission_profile = ActivePermissionProfile::new("strict");
        let expected_permissions =
            permission_profile_id_from_active_profile(active_permission_profile.clone());

        let (sandbox_policy, permissions) = turn_permissions_overrides(
            TurnPermissionsOverride::ActiveProfile(active_permission_profile),
            cwd.as_path(),
        );

        assert_eq!(sandbox_policy, None);
        assert_eq!(permissions, Some(expected_permissions));
    }

    #[tokio::test]
    async fn thread_lifecycle_params_omit_cwd_without_remote_override_for_remote_sessions() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config = build_config(&temp_dir).await;
        let thread_id = ThreadId::new();
        let expected_sandbox = sandbox_mode_from_permission_profile(
            &config.permissions.effective_permission_profile(),
            config.cwd.as_path(),
        );
        let expected_runtime_workspace_roots = Some(config.workspace_roots.clone());

        let start = thread_start_params_from_config(
            &config,
            ThreadParamsMode::Remote,
            /*remote_cwd_override*/ None,
            /*session_start_source*/ None,
        );
        let resume = thread_resume_params_from_config(
            config.clone(),
            thread_id,
            ThreadParamsMode::Remote,
            /*remote_cwd_override*/ None,
            ResumeModelSettings::OverrideFromCurrentConfig,
        );
        let fork = thread_fork_params_from_config(
            config,
            thread_id,
            ThreadParamsMode::Remote,
            /*remote_cwd_override*/ None,
        );

        assert_eq!(start.cwd, None);
        assert_eq!(resume.cwd, None);
        assert_eq!(fork.cwd, None);
        assert_eq!(
            start.runtime_workspace_roots,
            expected_runtime_workspace_roots
        );
        assert_eq!(
            resume.runtime_workspace_roots,
            expected_runtime_workspace_roots
        );
        assert_eq!(
            fork.runtime_workspace_roots,
            expected_runtime_workspace_roots
        );
        assert_eq!(start.model_provider, None);
        assert_eq!(resume.model_provider, None);
        assert_eq!(fork.model_provider, None);
        assert_eq!(start.sandbox, expected_sandbox);
        assert_eq!(resume.sandbox, expected_sandbox);
        assert_eq!(fork.sandbox, expected_sandbox);
        assert_eq!(start.permissions, None);
        assert_eq!(resume.permissions, None);
        assert_eq!(fork.permissions, None);
        assert_eq!(start.thread_source, Some(ThreadSource::User));
        assert_eq!(fork.thread_source, Some(ThreadSource::User));
    }

    #[test]
    fn sandbox_mode_does_not_project_non_cwd_write_roots_for_remote_sessions() {
        let cwd = test_path_buf("/workspace/project").abs();
        let extra_root = test_path_buf("/workspace/cache").abs();
        let permission_profile: PermissionProfile = PermissionProfile::Managed {
            network: NetworkSandboxPolicy::Restricted,
            file_system: ManagedFileSystemPermissions::Restricted {
                entries: vec![
                    FileSystemSandboxEntry {
                        path: FileSystemPath::Special {
                            value: FileSystemSpecialPath::Root,
                        },
                        access: FileSystemAccessMode::Read,
                    },
                    FileSystemSandboxEntry {
                        path: FileSystemPath::Path { path: extra_root },
                        access: FileSystemAccessMode::Write,
                    },
                ],
                glob_scan_max_depth: None,
            },
        };

        assert_eq!(
            sandbox_mode_from_permission_profile(&permission_profile, cwd.as_path()),
            Some(codex_app_server_protocol::SandboxMode::ReadOnly)
        );
    }

    #[test]
    fn sandbox_mode_projects_cwd_write_for_remote_sessions() {
        let cwd = test_path_buf("/workspace/project").abs();
        let permission_profile: PermissionProfile = PermissionProfile::Managed {
            network: NetworkSandboxPolicy::Restricted,
            file_system: ManagedFileSystemPermissions::Restricted {
                entries: vec![
                    FileSystemSandboxEntry {
                        path: FileSystemPath::Special {
                            value: FileSystemSpecialPath::Root,
                        },
                        access: FileSystemAccessMode::Read,
                    },
                    FileSystemSandboxEntry {
                        path: FileSystemPath::Special {
                            value: FileSystemSpecialPath::ProjectRoots { subpath: None },
                        },
                        access: FileSystemAccessMode::Write,
                    },
                ],
                glob_scan_max_depth: None,
            },
        };

        assert_eq!(
            sandbox_mode_from_permission_profile(&permission_profile, cwd.as_path()),
            Some(codex_app_server_protocol::SandboxMode::WorkspaceWrite)
        );
    }

    #[tokio::test]
    async fn thread_lifecycle_params_forward_explicit_remote_cwd_override_for_remote_sessions() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config = build_config(&temp_dir).await;
        let thread_id = ThreadId::new();
        let remote_cwd = PathBuf::from("repo/on/server");
        let expected_sandbox = sandbox_mode_from_permission_profile(
            &config.permissions.effective_permission_profile(),
            config.cwd.as_path(),
        );

        let start = thread_start_params_from_config(
            &config,
            ThreadParamsMode::Remote,
            Some(remote_cwd.as_path()),
            /*session_start_source*/ None,
        );
        let resume = thread_resume_params_from_config(
            config.clone(),
            thread_id,
            ThreadParamsMode::Remote,
            Some(remote_cwd.as_path()),
            ResumeModelSettings::OverrideFromCurrentConfig,
        );
        let fork = thread_fork_params_from_config(
            config,
            thread_id,
            ThreadParamsMode::Remote,
            Some(remote_cwd.as_path()),
        );

        assert_eq!(start.cwd.as_deref(), Some("repo/on/server"));
        assert_eq!(resume.cwd.as_deref(), Some("repo/on/server"));
        assert_eq!(fork.cwd.as_deref(), Some("repo/on/server"));
        assert_eq!(start.model_provider, None);
        assert_eq!(resume.model_provider, None);
        assert_eq!(fork.model_provider, None);
        assert_eq!(start.sandbox, expected_sandbox);
        assert_eq!(resume.sandbox, expected_sandbox);
        assert_eq!(fork.sandbox, expected_sandbox);
        assert_eq!(start.permissions, None);
        assert_eq!(resume.permissions, None);
        assert_eq!(fork.permissions, None);
        assert_eq!(start.thread_source, Some(ThreadSource::User));
        assert_eq!(fork.thread_source, Some(ThreadSource::User));
    }

    #[tokio::test]
    async fn thread_lifecycle_params_forward_config_overrides_and_service_tier() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let mut config = build_config(&temp_dir).await;
        config.model_reasoning_effort = Some(ReasoningEffort::High);
        config.model_reasoning_summary = Some(ReasoningSummary::Detailed);
        config.model_verbosity = Some(Verbosity::Low);
        config.personality = Some(Personality::Pragmatic);
        config
            .web_search_mode
            .set(WebSearchMode::Disabled)
            .expect("test web search mode should be allowed");
        config.bypass_hook_trust = true;
        config.service_tier = Some(ServiceTier::Fast.request_value().to_string());
        let thread_id = ThreadId::new();

        let start = thread_start_params_from_config(
            &config,
            ThreadParamsMode::Embedded,
            /*remote_cwd_override*/ None,
            /*session_start_source*/ None,
        );
        let resume = thread_resume_params_from_config(
            config.clone(),
            thread_id,
            ThreadParamsMode::Embedded,
            /*remote_cwd_override*/ None,
            ResumeModelSettings::OverrideFromCurrentConfig,
        );
        let fork = thread_fork_params_from_config(
            config,
            thread_id,
            ThreadParamsMode::Embedded,
            /*remote_cwd_override*/ None,
        );

        let expected_service_tier = Some(Some(ServiceTier::Fast.request_value().to_string()));
        assert_eq!(start.service_tier, expected_service_tier);
        assert_eq!(resume.service_tier, expected_service_tier);
        assert_eq!(fork.service_tier, expected_service_tier);
        let string = |value: &str| serde_json::Value::String(value.to_string());
        let expected_config = HashMap::from([
            ("model_reasoning_effort".to_string(), string("high")),
            ("model_reasoning_summary".to_string(), string("detailed")),
            ("model_verbosity".to_string(), string("low")),
            ("personality".to_string(), string("pragmatic")),
            ("web_search".to_string(), string("disabled")),
            ("bypass_hook_trust".to_string(), true.into()),
        ]);
        assert_eq!(start.config, Some(expected_config.clone()));
        assert_eq!(resume.config, Some(expected_config.clone()));
        assert_eq!(fork.config, Some(expected_config));
    }

    #[tokio::test]
    async fn thread_resume_params_can_restore_persisted_model_settings() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let mut config = build_config(&temp_dir).await;
        config.model = Some("configured-model".to_string());
        config.model_provider_id = "configured-provider".to_string();
        config.model_reasoning_effort = Some(ReasoningEffort::Ultra);
        config.model_reasoning_summary = Some(ReasoningSummary::Detailed);

        let params = thread_resume_params_from_config(
            config,
            ThreadId::new(),
            ThreadParamsMode::Embedded,
            /*remote_cwd_override*/ None,
            ResumeModelSettings::RestoreFromThread,
        );

        assert_eq!(params.model, None);
        assert_eq!(params.model_provider, None);
        assert_eq!(
            params.config,
            Some(HashMap::from([
                (
                    "model_reasoning_summary".to_string(),
                    serde_json::Value::String("detailed".to_string()),
                ),
                (
                    "personality".to_string(),
                    serde_json::Value::String("pragmatic".to_string()),
                ),
                (
                    "web_search".to_string(),
                    serde_json::Value::String("cached".to_string()),
                ),
            ]))
        );
    }

    #[tokio::test]
    async fn persisted_resume_does_not_forward_implicit_service_tier() -> Result<()> {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let mut config = build_config(&codex_home).await;
        config.model = Some("gpt-5.4".to_string());
        config.service_tier = None;
        config
            .features
            .enable(Feature::FastMode)
            .expect("enable fast mode");
        let thread_id = ThreadId::from_string(
            &create_fake_rollout(
                codex_home.path(),
                "2025-01-05T12-00-00",
                "2025-01-05T12:00:00Z",
                "Saved user message",
                Some(config.model_provider_id.as_str()),
                /*git_info*/ None,
            )
            .expect("create source rollout"),
        )?;
        let mut app_server = crate::start_embedded_app_server_for_picker(&config).await?;
        let mut preset = crate::test_support::TEST_MODEL_PRESETS
            .iter()
            .find(|preset| preset.model == "gpt-5.4")
            .expect("gpt-5.4 test preset")
            .clone();
        preset.service_tiers = vec![ModelServiceTier {
            id: ServiceTier::Fast.request_value().to_string(),
            name: "fast".to_string(),
            description: "Fast tier".to_string(),
        }];
        preset.default_service_tier = Some(ServiceTier::Fast.request_value().to_string());
        app_server.available_models = vec![preset];

        let resumed = app_server
            .resume_thread(config, thread_id, ResumeModelSettings::RestoreFromThread)
            .await?;

        assert_eq!(resumed.session.service_tier, None);
        app_server.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn config_request_overrides_preserve_implicit_personality_default() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let mut config = build_config(&temp_dir).await;
        config.personality = None;

        let implicit_overrides =
            config_request_overrides_from_config(&config).expect("config overrides");

        assert!(!implicit_overrides.contains_key("personality"));

        config.personality = Some(Personality::None);
        let explicit_overrides =
            config_request_overrides_from_config(&config).expect("config overrides");

        assert_eq!(
            explicit_overrides.get("personality"),
            Some(&serde_json::Value::String("none".to_string()))
        );
    }

    #[tokio::test]
    async fn thread_fork_params_forward_instruction_overrides() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let mut config = build_config(&temp_dir).await;
        config.base_instructions = Some("Base override.".to_string());
        config.developer_instructions = Some("Developer override.".to_string());
        let thread_id = ThreadId::new();

        let params = thread_fork_params_from_config(
            config,
            thread_id,
            ThreadParamsMode::Embedded,
            /*remote_cwd_override*/ None,
        );

        assert_eq!(params.base_instructions.as_deref(), Some("Base override."));
        assert_eq!(
            params.developer_instructions.as_deref(),
            Some("Developer override.")
        );
    }

    #[tokio::test]
    async fn terminal_visualization_instructions_are_gated_for_all_tui_thread_flows() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let mut config = build_config(&temp_dir).await;
        config.developer_instructions = Some("Developer override.".to_string());
        let thread_id = ThreadId::new();

        let control_start = thread_start_params_from_config(
            &config,
            ThreadParamsMode::Embedded,
            /*remote_cwd_override*/ None,
            /*session_start_source*/ None,
        );
        let control_resume = thread_resume_params_from_config(
            config.clone(),
            thread_id,
            ThreadParamsMode::Embedded,
            /*remote_cwd_override*/ None,
            ResumeModelSettings::OverrideFromCurrentConfig,
        );
        let control_fork = thread_fork_params_from_config(
            config.clone(),
            thread_id,
            ThreadParamsMode::Embedded,
            /*remote_cwd_override*/ None,
        );

        assert_eq!(control_start.developer_instructions, None);
        assert_eq!(control_resume.developer_instructions, None);
        assert_eq!(
            control_fork.developer_instructions.as_deref(),
            Some("Developer override.")
        );

        let _ = config
            .features
            .enable(Feature::TerminalVisualizationInstructions);
        let treatment_start = thread_start_params_from_config(
            &config,
            ThreadParamsMode::Embedded,
            /*remote_cwd_override*/ None,
            /*session_start_source*/ None,
        );
        let treatment_resume = thread_resume_params_from_config(
            config.clone(),
            thread_id,
            ThreadParamsMode::Embedded,
            /*remote_cwd_override*/ None,
            ResumeModelSettings::OverrideFromCurrentConfig,
        );
        let treatment_fork = thread_fork_params_from_config(
            config,
            thread_id,
            ThreadParamsMode::Embedded,
            /*remote_cwd_override*/ None,
        );
        let expected = format!(
            "Developer override.\n\n{}",
            crate::terminal_visualization_instructions::TERMINAL_VISUALIZATION_INSTRUCTIONS
        );

        assert_eq!(
            treatment_start.developer_instructions.as_deref(),
            Some(expected.as_str())
        );
        assert_eq!(
            treatment_resume.developer_instructions.as_deref(),
            Some(expected.as_str())
        );
        assert_eq!(
            treatment_fork.developer_instructions.as_deref(),
            Some(expected.as_str())
        );
    }

    #[tokio::test]
    async fn resume_response_restores_turns_from_thread_items() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config = build_config(&temp_dir).await;
        let thread_id = ThreadId::new();
        let forked_from_id = ThreadId::new();
        let read_only_profile = PermissionProfile::read_only();
        let response = ThreadResumeResponse {
            thread: codex_app_server_protocol::Thread {
                id: thread_id.to_string(),
                extra: None,
                session_id: ThreadId::new().to_string(),
                forked_from_id: Some(forked_from_id.to_string()),
                parent_thread_id: None,
                preview: "hello".to_string(),
                ephemeral: false,
                history_mode: Default::default(),
                model_provider: "openai".to_string(),
                created_at: 1,
                updated_at: 2,
                recency_at: Some(2),
                status: ThreadStatus::Idle,
                path: None,
                cwd: test_path_buf("/tmp/project").abs(),
                cli_version: "0.0.0".to_string(),
                source: codex_app_server_protocol::SessionSource::Cli,
                thread_source: None,
                agent_nickname: None,
                agent_role: None,
                git_info: None,
                name: None,
                turns: vec![Turn {
                    id: "turn-1".to_string(),
                    items_view: codex_app_server_protocol::TurnItemsView::Full,
                    items: vec![
                        codex_app_server_protocol::ThreadItem::UserMessage {
                            id: "user-1".to_string(),
                            client_id: None,
                            content: vec![codex_app_server_protocol::UserInput::Text {
                                text: "hello from history".to_string(),
                                text_elements: Vec::new(),
                            }],
                        },
                        codex_app_server_protocol::ThreadItem::AgentMessage {
                            id: "assistant-1".to_string(),
                            text: "assistant reply".to_string(),
                            phase: None,
                            memory_citation: None,
                        },
                    ],
                    status: TurnStatus::Completed,
                    error: None,
                    started_at: None,
                    completed_at: None,
                    duration_ms: None,
                }],
            },
            model: "gpt-5.4".to_string(),
            model_provider: "openai".to_string(),
            service_tier: None,
            cwd: test_path_buf("/tmp/project").abs(),
            runtime_workspace_roots: vec![
                test_path_buf("/tmp/project").abs(),
                test_path_buf("/tmp/project/extra").abs(),
            ],
            instruction_sources: vec![LegacyAppPathString::from_abs_path(
                &test_path_buf("/tmp/project/AGENTS.md").abs(),
            )],
            approval_policy: codex_app_server_protocol::AskForApproval::Never,
            approvals_reviewer: codex_app_server_protocol::ApprovalsReviewer::User,
            sandbox: read_only_profile
                .to_legacy_sandbox_policy(test_path_buf("/tmp/project").as_path())
                .expect("read-only profile must be legacy-compatible")
                .into(),
            active_permission_profile: None,
            reasoning_effort: None,
            multi_agent_mode: Default::default(),
            initial_turns_page: None,
        };

        let started = started_thread_from_resume_response(
            response.clone(),
            &config,
            ThreadParamsMode::Remote,
        )
        .await
        .expect("resume response should map");
        assert_eq!(started.session.forked_from_id, Some(forked_from_id));
        assert_eq!(
            started.session.runtime_workspace_roots,
            response.runtime_workspace_roots
        );
        assert_eq!(
            started.session.instruction_source_paths,
            response.instruction_source_path_uris()
        );
        assert_eq!(started.session.permission_profile, read_only_profile);
        assert_eq!(started.turns.len(), 1);
        assert_eq!(started.turns[0], response.thread.turns[0]);

        let embedded_config = ConfigBuilder::default()
            .codex_home(temp_dir.path().join("embedded-codex-home"))
            .harness_overrides(ConfigOverrides {
                default_permissions: Some(BUILT_IN_PERMISSION_PROFILE_WORKSPACE.to_string()),
                ..ConfigOverrides::default()
            })
            .build()
            .await
            .expect("config should build");
        let started = started_thread_from_resume_response(
            response.clone(),
            &embedded_config,
            ThreadParamsMode::Embedded,
        )
        .await
        .expect("embedded resume response should map");
        assert_eq!(started.session.permission_profile, read_only_profile);

        let mut empty_roots_response = response;
        empty_roots_response.runtime_workspace_roots = Vec::new();
        let started = started_thread_from_resume_response(
            empty_roots_response,
            &config,
            ThreadParamsMode::Remote,
        )
        .await
        .expect("resume response should map");
        assert_eq!(started.session.runtime_workspace_roots, Vec::new());
    }

    #[tokio::test]
    async fn remote_thread_response_uses_legacy_sandbox_fallback() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config = build_config(&temp_dir).await;
        let cwd = test_path_buf("/tmp/project").abs();
        let sandbox = PermissionProfile::read_only()
            .to_legacy_sandbox_policy(cwd.as_path())
            .expect("read-only profile must be legacy-compatible")
            .into();

        assert_eq!(
            display_permission_profile_from_thread_response(
                &sandbox,
                cwd.as_path(),
                &config,
                ThreadParamsMode::Remote,
            ),
            PermissionProfile::read_only()
        );
    }

    #[tokio::test]
    async fn embedded_thread_response_uses_local_config_profile() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config = ConfigBuilder::default()
            .codex_home(temp_dir.path().to_path_buf())
            .harness_overrides(ConfigOverrides {
                default_permissions: Some(BUILT_IN_PERMISSION_PROFILE_READ_ONLY.to_string()),
                ..ConfigOverrides::default()
            })
            .build()
            .await
            .expect("config should build");
        let cwd = test_path_buf("/tmp/project").abs();

        assert_eq!(
            display_permission_profile_from_thread_response(
                &codex_app_server_protocol::SandboxPolicy::DangerFullAccess,
                cwd.as_path(),
                &config,
                ThreadParamsMode::Embedded,
            ),
            PermissionProfile::read_only()
        );
    }

    #[tokio::test]
    async fn session_configured_populates_history_metadata() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config = build_config(&temp_dir).await;
        let thread_id = ThreadId::new();

        let history_config =
            codex_message_history::HistoryConfig::new(config.codex_home.clone(), &config.history);

        codex_message_history::append_entry("older", &thread_id, &history_config)
            .await
            .expect("history append should succeed");
        codex_message_history::append_entry("newer", &thread_id, &history_config)
            .await
            .expect("history append should succeed");

        let session = thread_session_state_from_thread_response(
            &thread_id.to_string(),
            /*forked_from_id*/ None,
            Some("restore".to_string()),
            /*rollout_path*/ None,
            "gpt-5.4".to_string(),
            "openai".to_string(),
            /*service_tier*/ None,
            AskForApproval::Never,
            codex_protocol::config_types::ApprovalsReviewer::User,
            PermissionProfile::read_only(),
            /*active_permission_profile*/ None,
            test_path_buf("/tmp/project").abs(),
            Vec::new(),
            Vec::new(),
            /*reasoning_effort*/ None,
            &config,
        )
        .await
        .expect("session should map");

        let metadata = session
            .message_history
            .expect("session should include message-history metadata");
        assert_ne!(metadata.log_id, 0);
        assert_eq!(metadata.entry_count, 2);
    }

    #[tokio::test]
    async fn session_configured_preserves_fork_source_thread_id() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config = build_config(&temp_dir).await;
        let thread_id = ThreadId::new();
        let forked_from_id = ThreadId::new();

        let session = thread_session_state_from_thread_response(
            &thread_id.to_string(),
            Some(forked_from_id.to_string()),
            Some("restore".to_string()),
            /*rollout_path*/ None,
            "gpt-5.4".to_string(),
            "openai".to_string(),
            /*service_tier*/ None,
            AskForApproval::Never,
            codex_protocol::config_types::ApprovalsReviewer::User,
            PermissionProfile::read_only(),
            /*active_permission_profile*/ None,
            test_path_buf("/tmp/project").abs(),
            Vec::new(),
            Vec::new(),
            /*reasoning_effort*/ None,
            &config,
        )
        .await
        .expect("session should map");

        assert_eq!(session.forked_from_id, Some(forked_from_id));
    }

    #[test]
    fn status_account_display_from_auth_mode_uses_remapped_plan_labels() {
        let business = status_account_display_from_auth_mode(
            Some(AuthMode::Chatgpt),
            Some(codex_protocol::account::PlanType::EnterpriseCbpUsageBased),
        );
        assert!(matches!(
            business,
            Some(StatusAccountDisplay::ChatGpt {
                email: None,
                plan: Some(ref plan),
            }) if plan == "Enterprise"
        ));

        let team = status_account_display_from_auth_mode(
            Some(AuthMode::Chatgpt),
            Some(codex_protocol::account::PlanType::SelfServeBusinessUsageBased),
        );
        assert!(matches!(
            team,
            Some(StatusAccountDisplay::ChatGpt {
                email: None,
                plan: Some(ref plan),
            }) if plan == "Business"
        ));
    }
}
