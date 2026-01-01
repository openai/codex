use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use futures::StreamExt;
use futures::stream::FuturesUnordered;
use serde::Deserialize;
use serde::Serialize;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::user_input::UserInput;

use crate::codex::Session;
use crate::codex::SessionSpawnOverrides;
use crate::codex::TurnContext;
use crate::codex_delegate::run_codex_conversation_one_shot_with_overrides;
use crate::features::Feature;
use crate::rollout::RolloutRecorder;
use crate::tools::policy::ShellPolicy;
use crate::tools::policy::ToolPolicy;

const SUBAGENT_MODEL: &str = "gpt-5.1-codex-mini";
const SUBAGENT_TIMEOUT: Duration = Duration::from_secs(300);
const SUBAGENT_MAX_CONCURRENCY: usize = 24;

const EXPLORE_PROMPT: &str = r#"Sos un subagente de exploracion read-only optimizado para velocidad.

TU OBJETIVO: Buscar y analizar codigo, devolver findings concisos.

REGLAS ESTRICTAS:
- Solo podes LEER, nunca modificar archivos
- No podes spawnear otros subagentes
- Se conciso y directo
- Devolve paths absolutos cuando referencies archivos
- Si no encontras algo, decilo claramente
- No copies archivos enteros, resumi lo relevante

FORMATO DE RESPUESTA:
1. Que encontraste (resumen ejecutivo)
2. Archivos relevantes (paths absolutos)
3. Detalles importantes
4. Lo que NO encontraste (si aplica)"#;

const PLAN_PROMPT: &str = r#"Sos un subagente de planificacion. Tu trabajo es investigar el codebase para que el agente principal pueda armar un plan informado.

OBJETIVO: Recolectar contexto sobre la estructura, dependencias y patrones existentes.

ENFOCATE EN:
- Estructura del proyecto
- Patrones existentes que deberian seguirse
- Dependencias relevantes
- Codigo relacionado a la tarea
- Tests existentes
- Configuraciones importantes

NO PODES:
- Modificar archivos
- Spawnear otros subagentes
- Ejecutar comandos destructivos

DEVOLVE: Un resumen estructurado que ayude a planificar la implementacion."#;

const GENERAL_PROMPT: &str = r#"Sos un subagente de proposito general para tareas complejas.

PODES: Leer, escribir, modificar archivos y ejecutar comandos.

REGLAS:
- No podes spawnear otros subagentes (prevenir recursion infinita)
- Documenta los cambios que hagas
- Si algo falla, reporta el error claramente
- Manten un log de acciones realizadas

DEVOLVE:
1. Resumen de acciones realizadas
2. Archivos modificados (con paths)
3. Resultado de cada accion
4. Errores encontrados (si hubo)"#;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SubagentType {
    #[default]
    Explore,
    Plan,
    General,
}

impl SubagentType {
    fn as_source(self) -> SubAgentSource {
        let label = match self {
            SubagentType::Explore => "explore",
            SubagentType::Plan => "plan",
            SubagentType::General => "general",
        };
        SubAgentSource::Other(label.to_string())
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SubagentThoroughness {
    Quick,
    #[default]
    Medium,
    Thorough,
}

impl SubagentThoroughness {
    fn as_str(self) -> &'static str {
        match self {
            SubagentThoroughness::Quick => "quick",
            SubagentThoroughness::Medium => "medium",
            SubagentThoroughness::Thorough => "thorough",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct SubagentTaskSpec {
    pub(crate) prompt: String,
    #[serde(rename = "type", default)]
    pub(crate) agent_type: SubagentType,
    #[serde(default)]
    pub(crate) thoroughness: SubagentThoroughness,
    #[serde(default)]
    pub(crate) resume: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct SubagentTask {
    pub(crate) prompt: String,
    pub(crate) agent_type: SubagentType,
    pub(crate) thoroughness: SubagentThoroughness,
    pub(crate) resume: Option<String>,
}

impl SubagentTaskSpec {
    pub(crate) fn into_task(self) -> SubagentTask {
        SubagentTask {
            prompt: self.prompt,
            agent_type: self.agent_type,
            thoroughness: self.thoroughness,
            resume: self.resume,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SubagentStatus {
    Success,
    Error,
    Timeout,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SubagentMetrics {
    #[serde(rename = "durationMs")]
    pub(crate) duration_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) token_usage: Option<TokenUsage>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SubagentResult {
    #[serde(rename = "agentId")]
    pub(crate) agent_id: String,
    pub(crate) result: String,
    pub(crate) status: SubagentStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) metrics: Option<SubagentMetrics>,
}

#[derive(Debug)]
struct SubagentOutcome {
    message: Option<String>,
    token_usage: Option<TokenUsage>,
    error_message: Option<String>,
    status: SubagentStatus,
}

pub(crate) struct SubagentManager {
    session: Arc<Session>,
    turn: Arc<TurnContext>,
}

impl SubagentManager {
    pub(crate) fn new(session: Arc<Session>, turn: Arc<TurnContext>) -> Self {
        Self { session, turn }
    }

    pub(crate) async fn run_parallel(&self, tasks: Vec<SubagentTask>) -> Vec<SubagentResult> {
        let total = tasks.len();
        self.notify_progress(0, total).await;

        let mut futures = FuturesUnordered::new();
        let limiter = Arc::new(tokio::sync::Semaphore::new(SUBAGENT_MAX_CONCURRENCY));

        for (index, task) in tasks.into_iter().enumerate() {
            let session = Arc::clone(&self.session);
            let turn = Arc::clone(&self.turn);
            let limiter = Arc::clone(&limiter);
            futures.push(async move {
                let _permit = match limiter.acquire_owned().await {
                    Ok(permit) => permit,
                    Err(_) => {
                        return (
                            index,
                            SubagentResult {
                                agent_id: task.resume.clone().unwrap_or_else(new_agent_id),
                                result: "subagent concurrency limiter closed unexpectedly"
                                    .to_string(),
                                status: SubagentStatus::Error,
                                metrics: None,
                            },
                        );
                    }
                };
                let result = run_subagent(session, turn, task).await;
                (index, result)
            });
        }

        let mut completed = 0;
        let mut results = vec![None; total];
        while let Some((index, result)) = futures.next().await {
            completed += 1;
            self.notify_progress(completed, total).await;
            results[index] = Some(result);
        }

        results
            .into_iter()
            .enumerate()
            .map(|(index, result)| {
                result.unwrap_or_else(|| SubagentResult {
                    agent_id: new_agent_id(),
                    result: format!("missing subagent result for task {index}"),
                    status: SubagentStatus::Error,
                    metrics: None,
                })
            })
            .collect()
    }

    pub(crate) async fn run_single(&self, task: SubagentTask) -> SubagentResult {
        run_subagent(Arc::clone(&self.session), Arc::clone(&self.turn), task).await
    }

    async fn notify_progress(&self, completed: usize, total: usize) {
        if total == 0 {
            return;
        }
        let message = if completed >= total {
            format!("Subagents completed ({completed}/{total})")
        } else {
            format!("Subagents running ({completed}/{total})")
        };
        self.session
            .notify_background_event(&self.turn, message)
            .await;
    }
}

async fn run_subagent(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    mut task: SubagentTask,
) -> SubagentResult {
    let started = Instant::now();
    task.prompt = task.prompt.trim().to_string();

    let agent_id = task.resume.clone().unwrap_or_else(new_agent_id);

    let mut metrics = SubagentMetrics {
        duration_ms: 0,
        token_usage: None,
    };

    if task.prompt.is_empty() {
        metrics.duration_ms = started.elapsed().as_millis();
        return SubagentResult {
            agent_id,
            result: "prompt must not be empty".to_string(),
            status: SubagentStatus::Error,
            metrics: Some(metrics),
        };
    }

    let config = turn.client.config();
    let (initial_history, rollout_override) = match resume_history(&config, &task, &agent_id).await
    {
        Ok(data) => data,
        Err(message) => {
            metrics.duration_ms = started.elapsed().as_millis();
            return SubagentResult {
                agent_id,
                result: message,
                status: SubagentStatus::Error,
                metrics: Some(metrics),
            };
        }
    };

    let mut sub_agent_config = (*config).clone();
    sub_agent_config.model = Some(SUBAGENT_MODEL.to_string());
    sub_agent_config.model_reasoning_effort = Some(ReasoningEffortConfig::Medium);
    sub_agent_config.user_instructions = None;
    sub_agent_config.developer_instructions = Some(build_subagent_instructions(&task));
    sub_agent_config.base_instructions = None;
    sub_agent_config.project_doc_max_bytes = 0;
    sub_agent_config.project_doc_fallback_filenames = Vec::new();
    sub_agent_config.features.disable(Feature::Skills);
    if matches!(task.agent_type, SubagentType::Explore | SubagentType::Plan) {
        sub_agent_config
            .features
            .disable(Feature::WebSearchRequest)
            .disable(Feature::ViewImageTool)
            .disable(Feature::UnifiedExec);
    }

    let tool_policy = build_tool_policy(&task);
    let spawn_overrides = SessionSpawnOverrides {
        tool_policy,
        rollout_path_override: rollout_override,
    };

    let cancel_token = CancellationToken::new();
    let cancel_token_for_run = cancel_token.clone();
    let input = vec![UserInput::Text {
        text: task.prompt.clone(),
    }];
    let session_source = SessionSource::SubAgent(task.agent_type.as_source());

    let run_result = tokio::time::timeout(SUBAGENT_TIMEOUT, async move {
        let io = run_codex_conversation_one_shot_with_overrides(
            sub_agent_config,
            Arc::clone(&session.services.auth_manager),
            Arc::clone(&session.services.models_manager),
            input,
            Arc::clone(&session),
            Arc::clone(&turn),
            cancel_token_for_run.clone(),
            initial_history,
            session_source,
            spawn_overrides,
        )
        .await?;

        let mut last_message = None;
        let mut token_usage = None;
        let mut error_message = None;

        while let Ok(event) = io.next_event().await {
            match event.msg {
                EventMsg::AgentMessage(ev) => {
                    last_message = Some(ev.message);
                }
                EventMsg::TokenCount(ev) => {
                    if let Some(info) = ev.info {
                        token_usage = Some(info.last_token_usage);
                    }
                }
                EventMsg::Error(ev) => {
                    error_message = Some(ev.message);
                }
                EventMsg::TaskComplete(ev) => {
                    let message = ev.last_agent_message.or(last_message);
                    return Ok::<SubagentOutcome, crate::error::CodexErr>(SubagentOutcome {
                        message,
                        token_usage,
                        error_message,
                        status: SubagentStatus::Success,
                    });
                }
                EventMsg::TurnAborted(_) => {
                    let message = error_message
                        .clone()
                        .or_else(|| Some("subagent aborted before completion".to_string()));
                    return Ok::<SubagentOutcome, crate::error::CodexErr>(SubagentOutcome {
                        message,
                        token_usage,
                        error_message,
                        status: SubagentStatus::Error,
                    });
                }
                _ => {}
            }
        }
        Ok::<SubagentOutcome, crate::error::CodexErr>(SubagentOutcome {
            message: last_message,
            token_usage,
            error_message,
            status: SubagentStatus::Error,
        })
    })
    .await;

    metrics.duration_ms = started.elapsed().as_millis();
    match run_result {
        Ok(Ok(outcome)) => {
            metrics.token_usage = outcome.token_usage;
            let result = outcome.message.unwrap_or_else(|| {
                outcome
                    .error_message
                    .unwrap_or_else(|| "subagent returned no output".to_string())
            });
            SubagentResult {
                agent_id,
                result,
                status: outcome.status,
                metrics: Some(metrics),
            }
        }
        Ok(Err(err)) => SubagentResult {
            agent_id,
            result: format!("subagent failed: {err}"),
            status: SubagentStatus::Error,
            metrics: Some(metrics),
        },
        Err(_) => {
            cancel_token.cancel();
            let timeout_seconds = SUBAGENT_TIMEOUT.as_secs();
            SubagentResult {
                agent_id,
                result: format!("subagent timed out after {timeout_seconds} seconds"),
                status: SubagentStatus::Timeout,
                metrics: Some(metrics),
            }
        }
    }
}

async fn resume_history(
    config: &crate::config::Config,
    task: &SubagentTask,
    agent_id: &str,
) -> Result<(Option<InitialHistory>, Option<std::path::PathBuf>), String> {
    let path = agent_transcript_path(config, agent_id);

    if let Some(resume_id) = task.resume.as_deref() {
        let resume_path = agent_transcript_path(config, resume_id);
        if tokio::fs::metadata(&resume_path).await.is_err() {
            let resume_path = resume_path.display();
            return Err(format!("resume transcript not found: {resume_path}"));
        }
        let history = RolloutRecorder::get_rollout_history(&resume_path)
            .await
            .map_err(|err| format!("failed to resume transcript: {err}"))?;
        if matches!(history, InitialHistory::New) {
            return Err("resume transcript is empty".to_string());
        }
        return Ok((Some(history), None));
    }

    if tokio::fs::metadata(&path).await.is_ok() {
        return Err(format!(
            "subagent transcript already exists for agent_id {agent_id}"
        ));
    }

    Ok((None, Some(path)))
}

fn agent_transcript_path(config: &crate::config::Config, agent_id: &str) -> std::path::PathBuf {
    config
        .codex_home
        .join("agents")
        .join(format!("agent-{agent_id}.jsonl"))
}

fn new_agent_id() -> String {
    Uuid::new_v4().to_string()
}

fn build_subagent_instructions(task: &SubagentTask) -> String {
    let base = match task.agent_type {
        SubagentType::Explore => EXPLORE_PROMPT,
        SubagentType::Plan => PLAN_PROMPT,
        SubagentType::General => GENERAL_PROMPT,
    };
    match task.agent_type {
        SubagentType::Explore | SubagentType::Plan => {
            let thoroughness = task.thoroughness.as_str();
            format!("{base}\n\nTHOROUGHNESS: {thoroughness}")
        }
        SubagentType::General => base.to_string(),
    }
}

fn build_tool_policy(task: &SubagentTask) -> ToolPolicy {
    let mut denied_tools = HashSet::new();
    denied_tools.extend(["spawn_subagents", "chain_subagents"].map(String::from));

    match task.agent_type {
        SubagentType::Explore | SubagentType::Plan => {
            let mut allowed_tools = HashSet::new();
            allowed_tools.extend(
                [
                    "glob",
                    "grep_files",
                    "read_file",
                    "list_dir",
                    "shell",
                    "shell_command",
                    "exec_command",
                    "local_shell",
                ]
                .map(String::from),
            );

            let mut extra_tools = HashSet::new();
            extra_tools.extend(["glob", "grep_files", "read_file", "list_dir"].map(String::from));

            ToolPolicy {
                allowed_tools: Some(allowed_tools),
                denied_tools,
                allow_mcp_tools: false,
                shell_policy: ShellPolicy::ReadOnly,
                extra_tools,
            }
        }
        SubagentType::General => ToolPolicy {
            allowed_tools: None,
            denied_tools,
            allow_mcp_tools: true,
            shell_policy: ShellPolicy::Unrestricted,
            extra_tools: HashSet::new(),
        },
    }
}
