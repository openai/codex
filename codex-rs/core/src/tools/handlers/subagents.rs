use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;

use crate::function_tool::FunctionCallError;
use crate::subagents::SubagentManager;
use crate::subagents::SubagentResult;
use crate::subagents::SubagentTask;
use crate::subagents::SubagentTaskSpec;
use crate::subagents::SubagentThoroughness;
use crate::subagents::SubagentType;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct SubagentsHandler;

#[derive(Deserialize)]
struct SpawnSubagentsArgs {
    tasks: Vec<SubagentTaskSpec>,
}

#[derive(Deserialize)]
struct ChainSubagentsArgs {
    steps: Vec<ChainStepSpec>,
}

#[derive(Deserialize)]
struct ChainStepSpec {
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    parallel: Option<Vec<SubagentTaskSpec>>,
    #[serde(rename = "type", default)]
    agent_type: SubagentType,
    #[serde(default)]
    thoroughness: SubagentThoroughness,
    #[serde(default)]
    resume: Option<String>,
}

#[derive(Serialize)]
struct ChainOutput {
    final_result: String,
    chain: Vec<ChainStepResult>,
}

#[derive(Serialize)]
struct ChainStepResult {
    step: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<SubagentResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    results: Option<Vec<SubagentResult>>,
}

#[async_trait]
impl ToolHandler for SubagentsHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            tool_name,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "subagents handler received unsupported payload".to_string(),
                ));
            }
        };

        let manager = SubagentManager::new(Arc::clone(&session), Arc::clone(&turn));

        match tool_name.as_str() {
            "spawn_subagents" => {
                let args: SpawnSubagentsArgs = serde_json::from_str(&arguments).map_err(|err| {
                    FunctionCallError::RespondToModel(format!(
                        "failed to parse spawn_subagents arguments: {err:?}"
                    ))
                })?;
                let tasks = normalize_tasks(args.tasks)?;
                let results = manager.run_parallel(tasks).await;
                let output = serde_json::to_string(&results).map_err(|err| {
                    FunctionCallError::RespondToModel(format!(
                        "failed to serialize spawn_subagents output: {err:?}"
                    ))
                })?;
                Ok(ToolOutput::Function {
                    content: output,
                    content_items: None,
                    success: Some(true),
                })
            }
            "chain_subagents" => {
                let args: ChainSubagentsArgs = serde_json::from_str(&arguments).map_err(|err| {
                    FunctionCallError::RespondToModel(format!(
                        "failed to parse chain_subagents arguments: {err:?}"
                    ))
                })?;
                let output = run_chain(manager, args.steps).await?;
                Ok(ToolOutput::Function {
                    content: output,
                    content_items: None,
                    success: Some(true),
                })
            }
            other => Err(FunctionCallError::RespondToModel(format!(
                "unsupported subagent tool: {other}"
            ))),
        }
    }
}

async fn run_chain(
    manager: SubagentManager,
    steps: Vec<ChainStepSpec>,
) -> Result<String, FunctionCallError> {
    if steps.is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "steps must not be empty".to_string(),
        ));
    }

    let mut chain_results = Vec::with_capacity(steps.len());
    let mut previous_output = String::new();

    for (index, step) in steps.into_iter().enumerate() {
        let step_number = index + 1;
        if let Some(parallel_tasks) = step.parallel {
            if step.prompt.is_some() {
                return Err(FunctionCallError::RespondToModel(
                    "chain steps cannot include both prompt and parallel".to_string(),
                ));
            }
            let tasks = normalize_tasks(parallel_tasks)?;
            let results = manager.run_parallel(tasks).await;
            previous_output = serialize_results(&results)?;
            chain_results.push(ChainStepResult {
                step: step_number,
                result: None,
                results: Some(results),
            });
            continue;
        }

        let Some(prompt) = step.prompt else {
            return Err(FunctionCallError::RespondToModel(
                "each chain step must include prompt or parallel".to_string(),
            ));
        };
        let prompt = apply_previous_output(&prompt, &previous_output);
        let task = SubagentTask {
            prompt,
            agent_type: step.agent_type,
            thoroughness: step.thoroughness,
            resume: step.resume,
        };
        let result = manager.run_single(task).await;
        previous_output = result.result.clone();
        chain_results.push(ChainStepResult {
            step: step_number,
            result: Some(result),
            results: None,
        });
    }

    let output = ChainOutput {
        final_result: previous_output,
        chain: chain_results,
    };
    serde_json::to_string(&output).map_err(|err| {
        FunctionCallError::RespondToModel(format!(
            "failed to serialize chain_subagents output: {err:?}"
        ))
    })
}

fn normalize_tasks(tasks: Vec<SubagentTaskSpec>) -> Result<Vec<SubagentTask>, FunctionCallError> {
    if tasks.is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "tasks must not be empty".to_string(),
        ));
    }
    let mut normalized = Vec::with_capacity(tasks.len());
    for task in tasks {
        let prompt = task.prompt.trim().to_string();
        if prompt.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "task prompt must not be empty".to_string(),
            ));
        }
        let mut task = task.into_task();
        task.prompt = prompt;
        normalized.push(task);
    }
    Ok(normalized)
}

fn apply_previous_output(prompt: &str, previous_output: &str) -> String {
    prompt.replace("{{previous_output}}", previous_output)
}

fn serialize_results(results: &[SubagentResult]) -> Result<String, FunctionCallError> {
    serde_json::to_string(results).map_err(|err| {
        FunctionCallError::RespondToModel(format!("failed to serialize subagent output: {err:?}"))
    })
}
