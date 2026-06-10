use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use crate::outgoing_message::ConnectionId;
use crate::request_processors::thread_processor::ThreadRequestProcessor;
use crate::request_processors::thread_processor::build_thread_from_snapshot;
use crate::request_processors::thread_summary::thread_started_notification;
use crate::thread_status::resolve_thread_status;
use chrono::Duration as ChronoDuration;
use chrono::Utc;
use codex_app_server_protocol::AutomationRunNowParams;
use codex_app_server_protocol::AutomationRunNowResponse;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadStatus;
use codex_core::StartThreadOptions;
use codex_core::config::ConfigOverrides;
use codex_extension_api::ExtensionDataInit;
use codex_features::Feature;
use codex_protocol::ThreadId;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::ThreadSource;
use codex_protocol::user_input::UserInput;
use codex_rollout::StateDbHandle;
use codex_state::AUTOMATION_HEARTBEAT_BLOCKED_RETRY_SECS;
use codex_state::Automation;
use codex_state::AutomationDispatchClaim;
use codex_state::AutomationDispatchMode;
use codex_state::AutomationDispatchOutcome;
use codex_state::AutomationDispatchSettings;
use codex_state::AutomationTarget;
use codex_utils_absolute_path::AbsolutePathBuf;
use uuid::Uuid;

impl ThreadRequestProcessor {
    pub(crate) async fn automation_run_now(
        &self,
        connection_id: ConnectionId,
        params: AutomationRunNowParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        if !self.config.features.enabled(Feature::Automations) {
            return Err(invalid_request("automations feature is disabled"));
        }
        let state_db = self
            .state_db
            .as_ref()
            .ok_or_else(|| internal_error("sqlite state db unavailable for automations"))?;

        let Some(automation) = state_db
            .get_automation(params.automation_id.as_str())
            .await
            .map_err(|err| internal_error(format!("failed to read automation: {err}")))?
        else {
            return Ok(Some(run_now_response(
                /*found*/ false, /*started_count*/ 0,
            )));
        };
        if !self
            .connection_can_run_automation(connection_id, &automation, params.thread_id.as_deref())
            .await?
        {
            return Ok(Some(run_now_response(
                /*found*/ false, /*started_count*/ 0,
            )));
        }

        let claimed_by = format!("app-server-run-now:{}", Uuid::now_v7());
        let outcome = state_db
            .claim_automation_run_now_if_owner(
                params.automation_id.as_str(),
                automation.owner_thread_id,
                claimed_by.as_str(),
            )
            .await
            .map_err(|err| internal_error(format!("failed to claim automation run: {err}")))?;
        let claim = match outcome {
            AutomationDispatchOutcome::NotFound => {
                return Ok(Some(run_now_response(
                    /*found*/ false, /*started_count*/ 0,
                )));
            }
            AutomationDispatchOutcome::AlreadyClaimed => {
                return Ok(Some(run_now_response(
                    /*found*/ true, /*started_count*/ 0,
                )));
            }
            AutomationDispatchOutcome::Claimed(claim) => claim,
        };

        let dispatch_result = match self.dispatch_claimed_automation(state_db, &claim).await {
            Ok(AutomationDispatchResult::Started(dispatch_result)) => dispatch_result,
            Ok(AutomationDispatchResult::Deferred) => {
                return Err(internal_error(
                    "automation runNow was deferred before dispatch",
                ));
            }
            Err(err) => {
                let _ = state_db
                    .mark_automation_dispatch_failed_terminal(
                        claim.automation.id.as_str(),
                        claim.ownership_token.as_str(),
                        err.message.as_str(),
                    )
                    .await;
                return Err(err);
            }
        };
        let completed = state_db
            .mark_automation_dispatch_completed(&claim, dispatch_result.last_error.as_deref())
            .await
            .map_err(|err| internal_error(format!("failed to complete automation run: {err}")))?;
        if !completed {
            return Err(internal_error(
                "automation run claim was lost before completion",
            ));
        }

        Ok(Some(run_now_response(
            /*found*/ true,
            u32::try_from(dispatch_result.started_count).unwrap_or(u32::MAX),
        )))
    }

    async fn connection_can_run_automation(
        &self,
        connection_id: ConnectionId,
        automation: &Automation,
        requested_thread_id: Option<&str>,
    ) -> Result<bool, JSONRPCErrorError> {
        if let Some(requested_thread_id) = requested_thread_id {
            let requested_thread_id =
                ThreadId::from_string(requested_thread_id).map_err(|err| {
                    invalid_request(format!("invalid automation runNow threadId: {err}"))
                })?;
            if requested_thread_id != automation.owner_thread_id {
                return Ok(false);
            }
        }
        Ok(self
            .thread_state_manager
            .thread_ids_for_connection(connection_id)
            .await
            .into_iter()
            .any(|thread_id| thread_id == automation.owner_thread_id))
    }

    pub(super) async fn dispatch_claimed_automation(
        &self,
        state_db: &StateDbHandle,
        claim: &AutomationDispatchClaim,
    ) -> Result<AutomationDispatchResult, JSONRPCErrorError> {
        let started = state_db
            .mark_automation_dispatch_started(
                claim.automation.id.as_str(),
                claim.ownership_token.as_str(),
            )
            .await
            .map_err(|err| internal_error(format!("failed to start automation dispatch: {err}")))?;
        if !started {
            return Err(internal_error(
                "automation run claim was lost before dispatch",
            ));
        }

        match &claim.automation.target {
            AutomationTarget::Cron { cwds } => {
                self.dispatch_cron_automation(state_db, claim, cwds).await
            }
            AutomationTarget::Heartbeat { thread_id } => {
                self.dispatch_heartbeat_automation(state_db, claim, *thread_id)
                    .await
            }
        }
    }

    async fn dispatch_cron_automation(
        &self,
        state_db: &StateDbHandle,
        claim: &AutomationDispatchClaim,
        cwds: &[PathBuf],
    ) -> Result<AutomationDispatchResult, JSONRPCErrorError> {
        let mut started_count = 0_usize;
        let mut last_error = None;
        for (cwd_index, cwd) in cwds.iter().enumerate().skip(claim.dispatch_cwd_index) {
            match self
                .spawn_and_submit_automation_thread(&claim.automation, cwd)
                .await
            {
                Ok(()) => {
                    started_count += 1;
                    let checkpointed = state_db
                        .checkpoint_automation_dispatch_progress(
                            claim.automation.id.as_str(),
                            claim.ownership_token.as_str(),
                            cwd_index + 1,
                            last_error.as_deref(),
                        )
                        .await
                        .map_err(|err| {
                            internal_error(format!(
                                "failed to checkpoint automation dispatch: {err}"
                            ))
                        })?;
                    if !checkpointed {
                        return Err(internal_error(
                            "automation run claim was lost before checkpoint",
                        ));
                    }
                }
                Err(err) => {
                    last_error = Some(err.message);
                }
            }
        }
        if started_count == 0
            && let Some(last_error) = last_error
        {
            return Err(internal_error(last_error));
        }
        Ok(AutomationDispatchResult::Started(
            StartedAutomationDispatch {
                started_count,
                last_error,
            },
        ))
    }

    async fn dispatch_heartbeat_automation(
        &self,
        state_db: &StateDbHandle,
        claim: &AutomationDispatchClaim,
        thread_id: ThreadId,
    ) -> Result<AutomationDispatchResult, JSONRPCErrorError> {
        match claim.dispatch_mode {
            AutomationDispatchMode::Manual => {
                self.dispatch_manual_heartbeat(claim, thread_id).await
            }
            AutomationDispatchMode::Scheduled => {
                self.dispatch_scheduled_heartbeat(state_db, claim, thread_id)
                    .await
            }
        }
    }

    async fn dispatch_manual_heartbeat(
        &self,
        claim: &AutomationDispatchClaim,
        thread_id: ThreadId,
    ) -> Result<AutomationDispatchResult, JSONRPCErrorError> {
        let loaded_status = self
            .thread_watch_manager
            .loaded_status_for_thread(&thread_id.to_string())
            .await;
        if matches!(loaded_status, ThreadStatus::Active { .. }) {
            return Err(invalid_request("heartbeat target thread is busy"));
        }

        let thread = self
            .thread_manager
            .get_thread(thread_id)
            .await
            .map_err(|_| invalid_request("heartbeat target thread is not loaded"))?;
        let thread_state = self.thread_state_manager.thread_state(thread_id).await;
        self.ensure_listener_task_running(thread_id, thread.clone(), thread_state)
            .await?;
        self.submit_automation_prompt(thread.as_ref(), build_heartbeat_prompt(&claim.automation))
            .await?;
        Ok(AutomationDispatchResult::Started(
            StartedAutomationDispatch {
                started_count: 1,
                last_error: None,
            },
        ))
    }

    async fn dispatch_scheduled_heartbeat(
        &self,
        state_db: &StateDbHandle,
        claim: &AutomationDispatchClaim,
        thread_id: ThreadId,
    ) -> Result<AutomationDispatchResult, JSONRPCErrorError> {
        let loaded_status = self
            .thread_watch_manager
            .loaded_status_for_thread(&thread_id.to_string())
            .await;
        if matches!(loaded_status, ThreadStatus::Active { .. }) {
            self.defer_scheduled_heartbeat(state_db, claim, "heartbeat target thread is busy")
                .await?;
            return Ok(AutomationDispatchResult::Deferred);
        }

        let thread = match self.thread_manager.get_thread(thread_id).await {
            Ok(thread) => {
                let thread_state = self.thread_state_manager.thread_state(thread_id).await;
                self.ensure_listener_task_running(thread_id, thread.clone(), thread_state)
                    .await?;
                thread
            }
            Err(_) => self.resume_automation_thread(thread_id).await?,
        };
        self.submit_automation_prompt(thread.as_ref(), build_heartbeat_prompt(&claim.automation))
            .await?;
        Ok(AutomationDispatchResult::Started(
            StartedAutomationDispatch {
                started_count: 1,
                last_error: None,
            },
        ))
    }

    async fn defer_scheduled_heartbeat(
        &self,
        state_db: &StateDbHandle,
        claim: &AutomationDispatchClaim,
        reason: &str,
    ) -> Result<(), JSONRPCErrorError> {
        let retry_at =
            Utc::now() + ChronoDuration::seconds(AUTOMATION_HEARTBEAT_BLOCKED_RETRY_SECS.max(0));
        let deferred = state_db
            .defer_scheduled_automation_dispatch(claim, retry_at, reason)
            .await
            .map_err(|err| internal_error(format!("failed to defer automation: {err}")))?;
        if deferred {
            Ok(())
        } else {
            Err(internal_error("automation run claim was lost before defer"))
        }
    }

    async fn spawn_and_submit_automation_thread(
        &self,
        automation: &Automation,
        cwd: &Path,
    ) -> Result<(), JSONRPCErrorError> {
        let dispatch_settings = automation
            .dispatch_settings
            .as_ref()
            .ok_or_else(|| invalid_request("cron automation is missing dispatch settings"))?;
        ensure_cwd_in_dispatch_scope(cwd, dispatch_settings)?;

        let config = self
            .config_manager
            .load_for_cwd(
                /*request_overrides*/ None,
                ConfigOverrides {
                    cwd: Some(cwd.to_path_buf()),
                    model: automation.model.clone(),
                    approval_policy: Some(dispatch_settings.approval_policy),
                    approvals_reviewer: Some(dispatch_settings.approvals_reviewer),
                    permission_profile: Some(dispatch_settings.permission_profile.clone()),
                    workspace_roots: Some(absolute_roots(&dispatch_settings.workspace_roots)?),
                    ..ConfigOverrides::default()
                },
                Some(cwd.to_path_buf()),
            )
            .await
            .map_err(|err| invalid_request(format!("failed to load automation config: {err}")))?;
        let environments = self
            .thread_manager
            .default_environment_selections(&config.cwd);
        let codex_core::NewThread {
            thread_id,
            thread,
            session_configured,
        } = self
            .thread_manager
            .start_thread_with_options(StartThreadOptions {
                config,
                initial_history: InitialHistory::New,
                session_source: Some(SessionSource::automation()),
                thread_source: Some(ThreadSource::User),
                dynamic_tools: Vec::new(),
                metrics_service_name: None,
                parent_trace: None,
                environments,
                thread_extension_init: ExtensionDataInit::default(),
            })
            .await
            .map_err(|err| internal_error(format!("failed to start automation thread: {err}")))?;

        let thread_state = self.thread_state_manager.thread_state(thread_id).await;
        self.ensure_listener_task_running(thread_id, thread.clone(), thread_state)
            .await?;
        let config_snapshot = thread.config_snapshot().await;
        let mut api_thread = build_thread_from_snapshot(
            thread_id,
            session_configured.session_id.to_string(),
            &config_snapshot,
            session_configured.rollout_path.clone(),
        );
        self.thread_watch_manager
            .upsert_thread_silently(api_thread.clone())
            .await;
        api_thread.status = resolve_thread_status(
            self.thread_watch_manager
                .loaded_status_for_thread(api_thread.id.as_str())
                .await,
            /*has_in_progress_turn*/ false,
        );
        self.outgoing
            .send_server_notification(ServerNotification::ThreadStarted(
                thread_started_notification(api_thread),
            ))
            .await;

        self.submit_automation_prompt(thread.as_ref(), build_cron_prompt(automation))
            .await
    }

    async fn submit_automation_prompt(
        &self,
        thread: &codex_core::CodexThread,
        prompt: String,
    ) -> Result<(), JSONRPCErrorError> {
        thread
            .submit(Op::UserInput {
                items: vec![UserInput::Text {
                    text: prompt,
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
                responsesapi_client_metadata: None,
                additional_context: BTreeMap::new(),
                thread_settings: ThreadSettingsOverrides::default(),
            })
            .await
            .map(|_| ())
            .map_err(|err| internal_error(format!("failed to submit automation prompt: {err}")))
    }
}

fn run_now_response(found: bool, started_count: u32) -> ClientResponsePayload {
    AutomationRunNowResponse {
        found,
        started_count,
    }
    .into()
}

pub(super) enum AutomationDispatchResult {
    Started(StartedAutomationDispatch),
    Deferred,
}

pub(super) struct StartedAutomationDispatch {
    pub(super) started_count: usize,
    pub(super) last_error: Option<String>,
}

fn ensure_cwd_in_dispatch_scope(
    cwd: &Path,
    dispatch_settings: &AutomationDispatchSettings,
) -> Result<(), JSONRPCErrorError> {
    if dispatch_settings
        .workspace_roots
        .iter()
        .any(|root| cwd.starts_with(root))
    {
        Ok(())
    } else {
        Err(invalid_request(
            "automation cwd is outside its approved workspace roots",
        ))
    }
}

fn absolute_roots(paths: &[PathBuf]) -> Result<Vec<AbsolutePathBuf>, JSONRPCErrorError> {
    paths
        .iter()
        .map(|path| {
            AbsolutePathBuf::from_absolute_path(path).map_err(|err| {
                invalid_request(format!(
                    "automation dispatch root must be absolute: {} ({err})",
                    path.display()
                ))
            })
        })
        .collect()
}

fn build_cron_prompt(automation: &Automation) -> String {
    let last_run_label = automation
        .last_run_at
        .map(|value| format!("{} ({})", value.to_rfc3339(), value.timestamp()))
        .unwrap_or_else(|| "never".to_string());
    format!(
        "Automation: {name}\nAutomation ID: {id}\nAutomation memory: $CODEX_HOME/automations/{id}/memory.md\nLast run: {last_run_label}\n\n{prompt}",
        name = automation.name,
        id = automation.id,
        prompt = automation.prompt,
    )
}

fn build_heartbeat_prompt(automation: &Automation) -> String {
    format!(
        r#"<heartbeat>
  <automation_id>{automation_id}</automation_id>
  <instructions>
{instructions}
  </instructions>
</heartbeat>

This heartbeat automation is being delivered over the normal user-input channel.
"#,
        automation_id = automation.id,
        instructions = escape_xml_text(&automation.prompt),
    )
}

fn escape_xml_text(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
