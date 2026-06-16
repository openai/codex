use std::path::PathBuf;
use std::sync::Arc;

use codex_extension_api::FunctionCallError;
use codex_extension_api::JsonToolOutput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolExecutorFuture;
use codex_extension_api::ToolName;
use codex_extension_api::ToolOutput;
use codex_extension_api::ToolSpec;
use codex_protocol::ThreadId;
use codex_protocol::openai_models::ReasoningEffort;
use codex_state::Automation as StateAutomation;
use codex_state::AutomationCreateParams;
use codex_state::AutomationDispatchSettings;
use codex_state::AutomationStatus as StateAutomationStatus;
use codex_state::AutomationTarget as StateAutomationTarget;
use codex_state::AutomationUpdateParams;
use serde::Deserialize;
use serde::Serialize;

use crate::extension::AutomationThreadContext;
use crate::spec::AUTOMATION_UPDATE_TOOL_NAME;
use crate::spec::create_automation_update_tool;

#[derive(Clone)]
pub(crate) struct AutomationUpdateTool {
    state_db: Arc<codex_state::StateRuntime>,
    context: AutomationThreadContext,
}

impl AutomationUpdateTool {
    pub(crate) fn new(
        state_db: Arc<codex_state::StateRuntime>,
        context: AutomationThreadContext,
    ) -> Self {
        Self { state_db, context }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct AutomationToolRequest {
    mode: AutomationToolMode,
    automation_id: Option<String>,
    kind: Option<AutomationToolKind>,
    name: Option<String>,
    prompt: Option<String>,
    rrule: Option<String>,
    model: Option<String>,
    reasoning_effort: Option<ReasoningEffort>,
    status: Option<AutomationToolStatus>,
    cwds: Option<Vec<PathBuf>>,
    thread_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum AutomationToolMode {
    List,
    Read,
    Create,
    Update,
    Delete,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum AutomationToolKind {
    Cron,
    Heartbeat,
}

#[derive(Debug, Deserialize)]
enum AutomationToolStatus {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "PAUSED")]
    Paused,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct AutomationToolResponse {
    automation: Option<AutomationResponse>,
    automations: Vec<AutomationResponse>,
    deleted: Option<bool>,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct AutomationResponse {
    id: String,
    name: String,
    prompt: String,
    status: String,
    rrule: String,
    next_run_at: Option<i64>,
    last_run_at: Option<i64>,
    created_at: i64,
    updated_at: i64,
    model: Option<String>,
    reasoning_effort: Option<ReasoningEffort>,
    target: AutomationTargetResponse,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
enum AutomationTargetResponse {
    Cron { cwds: Vec<PathBuf> },
    Heartbeat { thread_id: String },
}

impl ToolExecutor<ToolCall> for AutomationUpdateTool {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(AUTOMATION_UPDATE_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        create_automation_update_tool()
    }

    fn handle(&self, invocation: ToolCall) -> ToolExecutorFuture<'_> {
        Box::pin(async move {
            let request: AutomationToolRequest = parse_arguments(invocation.function_arguments()?)?;
            match request.mode {
                AutomationToolMode::List => self.handle_list().await,
                AutomationToolMode::Read => self.handle_read(request).await,
                AutomationToolMode::Create => self.handle_create(request).await,
                AutomationToolMode::Update => self.handle_update(request).await,
                AutomationToolMode::Delete => self.handle_delete(request).await,
            }
        })
    }
}

impl AutomationUpdateTool {
    async fn handle_list(&self) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let automations = self
            .state_db
            .list_automations()
            .await
            .map_err(storage_error)?
            .into_iter()
            .filter(|automation| automation.owner_thread_id == self.context.thread_id)
            .map(AutomationResponse::from)
            .collect();
        automation_response(/*automation*/ None, automations, /*deleted*/ None)
    }

    async fn handle_read(
        &self,
        request: AutomationToolRequest,
    ) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let automation_id = required(request.automation_id, "automation_id")?;
        let automation = self
            .state_db
            .get_automation(automation_id.as_str())
            .await
            .map_err(storage_error)?
            .filter(|automation| automation.owner_thread_id == self.context.thread_id)
            .map(AutomationResponse::from);
        automation_response(automation, Vec::new(), /*deleted*/ None)
    }

    async fn handle_create(
        &self,
        request: AutomationToolRequest,
    ) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let target = self.target_from_request(&request, /*existing*/ None)?;
        let dispatch_settings = dispatch_settings_for_target(&self.context, &target)?;
        let automation = self
            .state_db
            .create_automation(&AutomationCreateParams {
                owner_thread_id: self.context.thread_id,
                name: required(request.name, "name")?,
                prompt: required(request.prompt, "prompt")?,
                status: request
                    .status
                    .map(StateAutomationStatus::from)
                    .unwrap_or(StateAutomationStatus::Active),
                rrule: request.rrule,
                model: request.model,
                reasoning_effort: request.reasoning_effort,
                target,
                dispatch_settings,
            })
            .await
            .map_err(mutation_error)?;
        automation_response(
            Some(AutomationResponse::from(automation)),
            Vec::new(),
            /*deleted*/ None,
        )
    }

    async fn handle_update(
        &self,
        request: AutomationToolRequest,
    ) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let automation_id = required(request.automation_id.clone(), "automation_id")?;
        let Some(existing) = self
            .state_db
            .get_automation(automation_id.as_str())
            .await
            .map_err(storage_error)?
            .filter(|automation| automation.owner_thread_id == self.context.thread_id)
        else {
            return automation_response(
                /*automation*/ None,
                Vec::new(),
                /*deleted*/ None,
            );
        };

        let target = self.target_from_request(&request, Some(&existing.target))?;
        let dispatch_settings = dispatch_settings_for_target(&self.context, &target)?;
        let updated = self
            .state_db
            .update_automation_if_owner(
                &AutomationUpdateParams {
                    id: automation_id,
                    owner_thread_id: self.context.thread_id,
                    name: request.name.unwrap_or(existing.name),
                    prompt: request.prompt.unwrap_or(existing.prompt),
                    status: request.status.map(Into::into).unwrap_or(existing.status),
                    rrule: Some(request.rrule.unwrap_or(existing.rrule)),
                    model: request.model.or(existing.model),
                    reasoning_effort: request.reasoning_effort.or(existing.reasoning_effort),
                    target,
                    dispatch_settings,
                },
                self.context.thread_id,
            )
            .await
            .map_err(mutation_error)?
            .map(AutomationResponse::from);
        automation_response(updated, Vec::new(), /*deleted*/ None)
    }

    async fn handle_delete(
        &self,
        request: AutomationToolRequest,
    ) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let automation_id = required(request.automation_id, "automation_id")?;
        let deleted = self
            .state_db
            .delete_automation_if_owner(automation_id.as_str(), self.context.thread_id)
            .await
            .map_err(storage_error)?;
        automation_response(/*automation*/ None, Vec::new(), Some(deleted))
    }

    fn target_from_request(
        &self,
        request: &AutomationToolRequest,
        existing: Option<&StateAutomationTarget>,
    ) -> Result<StateAutomationTarget, FunctionCallError> {
        match request.kind.as_ref() {
            Some(AutomationToolKind::Cron) => {
                let cwds = request
                    .cwds
                    .clone()
                    .unwrap_or_else(|| vec![self.context.cwd.clone()]);
                Ok(StateAutomationTarget::Cron { cwds })
            }
            Some(AutomationToolKind::Heartbeat) => {
                let thread_id = match request.thread_id.as_deref() {
                    Some(thread_id) => ThreadId::from_string(thread_id)
                        .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?,
                    None => self.context.thread_id,
                };
                Ok(StateAutomationTarget::Heartbeat { thread_id })
            }
            None => match existing {
                Some(target) => Ok(target.clone()),
                None => Err(FunctionCallError::RespondToModel(
                    "kind is required when creating an automation".to_string(),
                )),
            },
        }
    }
}

impl From<AutomationToolStatus> for StateAutomationStatus {
    fn from(status: AutomationToolStatus) -> Self {
        match status {
            AutomationToolStatus::Active => Self::Active,
            AutomationToolStatus::Paused => Self::Paused,
        }
    }
}

impl From<StateAutomation> for AutomationResponse {
    fn from(automation: StateAutomation) -> Self {
        Self {
            id: automation.id,
            name: automation.name,
            prompt: automation.prompt,
            status: automation.status.as_str().to_string(),
            rrule: automation.rrule,
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
            model: automation.model,
            reasoning_effort: automation.reasoning_effort,
            target: automation.target.into(),
        }
    }
}

impl From<StateAutomationTarget> for AutomationTargetResponse {
    fn from(target: StateAutomationTarget) -> Self {
        match target {
            StateAutomationTarget::Cron { cwds } => Self::Cron { cwds },
            StateAutomationTarget::Heartbeat { thread_id } => Self::Heartbeat {
                thread_id: thread_id.to_string(),
            },
        }
    }
}

fn dispatch_settings_for_target(
    context: &AutomationThreadContext,
    target: &StateAutomationTarget,
) -> Result<Option<AutomationDispatchSettings>, FunctionCallError> {
    match target {
        StateAutomationTarget::Cron { cwds } => {
            if cwds_visible_under_roots(cwds, &context.workspace_roots) {
                Ok(Some(context.dispatch_settings.clone()))
            } else {
                Err(FunctionCallError::RespondToModel(
                    "cron automation cwds must be inside this thread's workspace roots".to_string(),
                ))
            }
        }
        StateAutomationTarget::Heartbeat { thread_id } => {
            if *thread_id == context.thread_id {
                Ok(None)
            } else {
                Err(FunctionCallError::RespondToModel(
                    "heartbeat automations can only target the current thread".to_string(),
                ))
            }
        }
    }
}

fn cwds_visible_under_roots(cwds: &[PathBuf], workspace_roots: &[PathBuf]) -> bool {
    !cwds.is_empty()
        && cwds.iter().all(|cwd| {
            cwd.is_absolute() && workspace_roots.iter().any(|root| cwd.starts_with(root))
        })
}

fn required(value: Option<String>, field: &str) -> Result<String, FunctionCallError> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| FunctionCallError::RespondToModel(format!("{field} is required")))
}

fn parse_arguments<T>(arguments: &str) -> Result<T, FunctionCallError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(arguments)
        .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))
}

fn automation_response(
    automation: Option<AutomationResponse>,
    automations: Vec<AutomationResponse>,
    deleted: Option<bool>,
) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
    let value = serde_json::to_value(AutomationToolResponse {
        automation,
        automations,
        deleted,
    })
    .map_err(|err| FunctionCallError::Fatal(err.to_string()))?;
    Ok(Box::new(JsonToolOutput::new(value)))
}

fn storage_error(err: impl std::fmt::Display) -> FunctionCallError {
    FunctionCallError::RespondToModel(format!("failed to read automations: {err}"))
}

fn mutation_error(err: impl std::fmt::Display) -> FunctionCallError {
    FunctionCallError::RespondToModel(format!("failed to update automations: {err}"))
}
