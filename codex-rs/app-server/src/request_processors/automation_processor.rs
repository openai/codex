use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use crate::outgoing_message::ConnectionId;
use crate::thread_state::ThreadStateManager;
use codex_app_server_protocol::Automation as ApiAutomation;
use codex_app_server_protocol::AutomationCreateParams as ApiAutomationCreateParams;
use codex_app_server_protocol::AutomationCreateResponse;
use codex_app_server_protocol::AutomationDeleteParams;
use codex_app_server_protocol::AutomationDeleteResponse;
use codex_app_server_protocol::AutomationListParams;
use codex_app_server_protocol::AutomationListResponse;
use codex_app_server_protocol::AutomationReadParams;
use codex_app_server_protocol::AutomationReadResponse;
use codex_app_server_protocol::AutomationStatus as ApiAutomationStatus;
use codex_app_server_protocol::AutomationTarget as ApiAutomationTarget;
use codex_app_server_protocol::AutomationUpdateParams as ApiAutomationUpdateParams;
use codex_app_server_protocol::AutomationUpdateResponse;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_features::Feature;
use codex_protocol::ThreadId;
use codex_rollout::StateDbHandle;
use codex_state::Automation as StateAutomation;
use codex_state::AutomationCreateParams as StateAutomationCreateParams;
use codex_state::AutomationDispatchSettings as StateAutomationDispatchSettings;
use codex_state::AutomationStatus as StateAutomationStatus;
use codex_state::AutomationTarget as StateAutomationTarget;
use codex_state::AutomationUpdateParams as StateAutomationUpdateParams;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

const DEFAULT_AUTOMATION_LIST_LIMIT: usize = 50;

#[derive(Clone)]
pub(crate) struct AutomationRequestProcessor {
    config: Arc<Config>,
    state_db: Option<StateDbHandle>,
    thread_manager: Arc<ThreadManager>,
    thread_state_manager: ThreadStateManager,
}

impl AutomationRequestProcessor {
    pub(crate) fn new(
        config: Arc<Config>,
        state_db: Option<StateDbHandle>,
        thread_manager: Arc<ThreadManager>,
        thread_state_manager: ThreadStateManager,
    ) -> Self {
        Self {
            config,
            state_db,
            thread_manager,
            thread_state_manager,
        }
    }

    pub(crate) async fn automation_list(
        &self,
        connection_id: ConnectionId,
        params: AutomationListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.ensure_enabled()?;
        let state_db = self.state_db()?;
        let visible_thread_ids = self.visible_thread_ids(connection_id).await;
        let automations = state_db
            .list_automations()
            .await
            .map_err(map_automation_storage_error)?
            .into_iter()
            .filter(|automation| visible_thread_ids.contains(&automation.owner_thread_id))
            .collect::<Vec<_>>();
        let total = automations.len();
        let start = match params.cursor {
            Some(cursor) => cursor
                .parse::<usize>()
                .map_err(|_| invalid_request(format!("invalid cursor: {cursor}")))?,
            None => 0,
        };
        if start > total {
            return Err(invalid_request(format!(
                "cursor {start} exceeds total automations {total}"
            )));
        }

        let effective_limit = params
            .limit
            .unwrap_or(DEFAULT_AUTOMATION_LIST_LIMIT as u32)
            .max(1) as usize;
        let end = start.saturating_add(effective_limit).min(total);
        let data = automations[start..end]
            .iter()
            .map(api_automation_from_state)
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = (end < total).then(|| end.to_string());

        Ok(Some(AutomationListResponse { data, next_cursor }.into()))
    }

    pub(crate) async fn automation_read(
        &self,
        connection_id: ConnectionId,
        params: AutomationReadParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.ensure_enabled()?;
        let state_db = self.state_db()?;
        let visible_thread_ids = self.visible_thread_ids(connection_id).await;
        let automation = state_db
            .get_automation(params.automation_id.as_str())
            .await
            .map_err(map_automation_storage_error)?
            .filter(|automation| visible_thread_ids.contains(&automation.owner_thread_id))
            .map(|automation| api_automation_from_state(&automation))
            .transpose()?;
        Ok(Some(AutomationReadResponse { automation }.into()))
    }

    pub(crate) async fn automation_create(
        &self,
        connection_id: ConnectionId,
        params: ApiAutomationCreateParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.ensure_enabled()?;
        let state_db = self.state_db()?;
        let visible_thread_ids = self.visible_thread_ids(connection_id).await;
        let target = state_target_from_api(params.target)?;
        let (owner_thread_id, dispatch_settings) = self
            .owner_and_dispatch_settings_for_target(&visible_thread_ids, &target)
            .await?;
        let created = state_db
            .create_automation(&StateAutomationCreateParams {
                owner_thread_id,
                name: required_non_empty("name", params.name.as_str())?,
                prompt: required_non_empty("prompt", params.prompt.as_str())?,
                status: params
                    .status
                    .map(state_status_from_api)
                    .unwrap_or(StateAutomationStatus::Active),
                rrule: trim_to_option(params.rrule.as_deref()),
                model: trim_to_option(params.model.as_deref()),
                reasoning_effort: params.reasoning_effort,
                target,
                dispatch_settings,
            })
            .await
            .map_err(map_automation_mutation_error)?;
        Ok(Some(
            AutomationCreateResponse {
                automation: api_automation_from_state(&created)?,
            }
            .into(),
        ))
    }

    pub(crate) async fn automation_update(
        &self,
        connection_id: ConnectionId,
        params: ApiAutomationUpdateParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.ensure_enabled()?;
        let state_db = self.state_db()?;
        let visible_thread_ids = self.visible_thread_ids(connection_id).await;
        let Some(existing) = state_db
            .get_automation(params.automation_id.as_str())
            .await
            .map_err(map_automation_storage_error)?
        else {
            return Ok(Some(AutomationUpdateResponse { automation: None }.into()));
        };
        if !visible_thread_ids.contains(&existing.owner_thread_id) {
            return Ok(Some(AutomationUpdateResponse { automation: None }.into()));
        }

        let target = match params.target {
            Some(target) => state_target_from_api(target)?,
            None => existing.target.clone(),
        };
        let (owner_thread_id, dispatch_settings) = self
            .owner_and_dispatch_settings_for_target(&visible_thread_ids, &target)
            .await?;
        let updated = state_db
            .update_automation_if_owner(
                &StateAutomationUpdateParams {
                    id: params.automation_id,
                    owner_thread_id,
                    name: match params.name {
                        Some(name) => required_non_empty("name", name.as_str())?,
                        None => existing.name.clone(),
                    },
                    prompt: match params.prompt {
                        Some(prompt) => required_non_empty("prompt", prompt.as_str())?,
                        None => existing.prompt.clone(),
                    },
                    status: params
                        .status
                        .map(state_status_from_api)
                        .unwrap_or(existing.status),
                    rrule: match params.rrule {
                        None => Some(existing.rrule.clone()),
                        Some(rrule) => trim_to_option(rrule.as_deref()),
                    },
                    model: match params.model {
                        None => existing.model.clone(),
                        Some(model) => trim_to_option(model.as_deref()),
                    },
                    reasoning_effort: match params.reasoning_effort {
                        None => existing.reasoning_effort,
                        Some(reasoning_effort) => reasoning_effort,
                    },
                    target,
                    dispatch_settings,
                },
                existing.owner_thread_id,
            )
            .await
            .map_err(map_automation_mutation_error)?
            .map(|automation| api_automation_from_state(&automation))
            .transpose()?;

        Ok(Some(
            AutomationUpdateResponse {
                automation: updated,
            }
            .into(),
        ))
    }

    pub(crate) async fn automation_delete(
        &self,
        connection_id: ConnectionId,
        params: AutomationDeleteParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.ensure_enabled()?;
        let state_db = self.state_db()?;
        let visible_thread_ids = self.visible_thread_ids(connection_id).await;
        let Some(automation) = state_db
            .get_automation(params.automation_id.as_str())
            .await
            .map_err(map_automation_storage_error)?
        else {
            return Ok(Some(AutomationDeleteResponse { deleted: false }.into()));
        };
        if !visible_thread_ids.contains(&automation.owner_thread_id) {
            return Ok(Some(AutomationDeleteResponse { deleted: false }.into()));
        }
        let deleted = state_db
            .delete_automation_if_owner(params.automation_id.as_str(), automation.owner_thread_id)
            .await
            .map_err(map_automation_storage_error)?;
        Ok(Some(AutomationDeleteResponse { deleted }.into()))
    }

    fn ensure_enabled(&self) -> Result<(), JSONRPCErrorError> {
        if self.config.features.enabled(Feature::Automations) {
            Ok(())
        } else {
            Err(invalid_request("automations feature is disabled"))
        }
    }

    fn state_db(&self) -> Result<&StateDbHandle, JSONRPCErrorError> {
        self.state_db
            .as_ref()
            .ok_or_else(|| internal_error("sqlite state db unavailable for automations"))
    }

    async fn visible_thread_ids(&self, connection_id: ConnectionId) -> HashSet<ThreadId> {
        self.thread_state_manager
            .thread_ids_for_connection(connection_id)
            .await
            .into_iter()
            .collect()
    }

    async fn owner_and_dispatch_settings_for_target(
        &self,
        visible_thread_ids: &HashSet<ThreadId>,
        target: &StateAutomationTarget,
    ) -> Result<(ThreadId, Option<StateAutomationDispatchSettings>), JSONRPCErrorError> {
        match target {
            StateAutomationTarget::Heartbeat { thread_id } => {
                if visible_thread_ids.contains(thread_id) {
                    Ok((*thread_id, None))
                } else {
                    Err(invalid_request(
                        "heartbeat automations must target a subscribed thread",
                    ))
                }
            }
            StateAutomationTarget::Cron { cwds } => {
                for thread_id in visible_thread_ids {
                    let Ok(thread) = self.thread_manager.get_thread(*thread_id).await else {
                        continue;
                    };
                    let snapshot = thread.config_snapshot().await;
                    if snapshot.ephemeral || snapshot.session_source.is_automation() {
                        continue;
                    }
                    let workspace_roots = if snapshot.workspace_roots.is_empty() {
                        vec![snapshot.cwd().clone().into_path_buf()]
                    } else {
                        snapshot
                            .workspace_roots
                            .iter()
                            .cloned()
                            .map(codex_utils_absolute_path::AbsolutePathBuf::into_path_buf)
                            .collect::<Vec<_>>()
                    };
                    if cwds_visible_under_roots(cwds, &workspace_roots) {
                        return Ok((
                            *thread_id,
                            Some(StateAutomationDispatchSettings {
                                workspace_roots,
                                approval_policy: snapshot.approval_policy,
                                approvals_reviewer: snapshot.approvals_reviewer,
                                permission_profile: snapshot.permission_profile,
                            }),
                        ));
                    }
                }
                Err(invalid_request(
                    "cron automations must target cwd paths under a loaded subscribed thread",
                ))
            }
        }
    }
}

fn api_automation_from_state(
    automation: &StateAutomation,
) -> Result<ApiAutomation, JSONRPCErrorError> {
    Ok(ApiAutomation {
        id: automation.id.clone(),
        name: automation.name.clone(),
        prompt: automation.prompt.clone(),
        status: api_status_from_state(automation.status),
        rrule: automation.rrule.clone(),
        next_run_at: automation
            .next_run_at
            .as_ref()
            .map(chrono::DateTime::timestamp),
        last_run_at: automation
            .last_run_at
            .as_ref()
            .map(chrono::DateTime::timestamp),
        created_at: automation.created_at.timestamp(),
        updated_at: automation.updated_at.timestamp(),
        model: automation.model.clone(),
        reasoning_effort: automation.reasoning_effort.clone(),
        target: api_target_from_state(&automation.target),
    })
}

fn api_target_from_state(target: &StateAutomationTarget) -> ApiAutomationTarget {
    match target {
        StateAutomationTarget::Cron { cwds } => ApiAutomationTarget::Cron { cwds: cwds.clone() },
        StateAutomationTarget::Heartbeat { thread_id } => ApiAutomationTarget::Heartbeat {
            thread_id: thread_id.to_string(),
        },
    }
}

fn state_target_from_api(
    target: ApiAutomationTarget,
) -> Result<StateAutomationTarget, JSONRPCErrorError> {
    match target {
        ApiAutomationTarget::Cron { cwds } => Ok(StateAutomationTarget::Cron { cwds }),
        ApiAutomationTarget::Heartbeat { thread_id } => ThreadId::from_string(&thread_id)
            .map(|thread_id| StateAutomationTarget::Heartbeat { thread_id })
            .map_err(|err| invalid_request(format!("invalid thread id: {err}"))),
    }
}

fn api_status_from_state(status: StateAutomationStatus) -> ApiAutomationStatus {
    match status {
        StateAutomationStatus::Active => ApiAutomationStatus::Active,
        StateAutomationStatus::Paused => ApiAutomationStatus::Paused,
    }
}

fn state_status_from_api(status: ApiAutomationStatus) -> StateAutomationStatus {
    match status {
        ApiAutomationStatus::Active => StateAutomationStatus::Active,
        ApiAutomationStatus::Paused => StateAutomationStatus::Paused,
    }
}

fn required_non_empty(field: &str, value: &str) -> Result<String, JSONRPCErrorError> {
    trim_to_option(Some(value)).ok_or_else(|| invalid_request(format!("{field} must not be empty")))
}

fn trim_to_option(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn cwds_visible_under_roots(cwds: &[PathBuf], workspace_roots: &[PathBuf]) -> bool {
    !cwds.is_empty()
        && cwds
            .iter()
            .all(|cwd| workspace_roots.iter().any(|root| cwd.starts_with(root)))
}

fn map_automation_storage_error(err: anyhow::Error) -> JSONRPCErrorError {
    internal_error(format!("automation storage failure: {err}"))
}

fn map_automation_mutation_error(err: anyhow::Error) -> JSONRPCErrorError {
    let message = err.to_string();
    if is_automation_user_error(message.as_str()) {
        invalid_request(message)
    } else {
        internal_error(format!("automation mutation failure: {message}"))
    }
}

fn is_automation_user_error(message: &str) -> bool {
    [
        "changing automation kind is not supported",
        "active heartbeat already exists for thread",
        "automation name is required",
        "automation name is too long",
        "automation prompt is required",
        "automation prompt is too long",
        "cron automations require at least one cwd",
        "cron automation cwd must not be empty",
        "cron automation cwd must be absolute",
        "cron automations require dispatch settings",
        "cron automations require at least one workspace root",
        "cron automation dispatch settings must align with target cwds",
        "cron automation workspace roots must be absolute",
        "cron automation workspace roots must be frozen resolved paths",
        "cron automation dispatch settings must contain each target cwd within the approved workspace root and permission scope",
        "cannot change automation target or dispatch scope while a dispatch is in flight",
        "heartbeat automations must be owned by their target thread",
        "heartbeat automations do not use dispatch settings",
        "rrule ",
        "invalid rrule",
        "unsupported rrule frequency:",
        "MINUTELY rrules only support",
        "HOURLY rrules only support",
        "DAILY rrules require",
        "DAILY rrules do not support",
        "WEEKLY rrules only support",
        "WEEKLY rrules require",
        "failed to compute weekly next run",
    ]
    .iter()
    .any(|prefix| message.starts_with(prefix))
}
