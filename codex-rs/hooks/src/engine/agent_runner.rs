use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use futures::future::BoxFuture;
use tokio::time::timeout;

use super::ConfiguredHandler;
use super::ConfiguredHandlerKind;
use super::command_runner::CommandRunResult;
use super::prompt_runner::model_hook_output_to_command_stdout;
use super::prompt_runner::model_hook_run_result;
use super::prompt_runner::render_model_hook_prompt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentHookRequest {
    pub prompt: String,
    pub model: String,
}

#[derive(Clone)]
pub struct AgentHookRunner {
    run: Arc<dyn Fn(AgentHookRequest) -> BoxFuture<'static, anyhow::Result<String>> + Send + Sync>,
}

impl AgentHookRunner {
    pub fn new<F, Fut>(run: F) -> Self
    where
        F: Fn(AgentHookRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = anyhow::Result<String>> + Send + 'static,
    {
        Self {
            run: Arc::new(move |request| Box::pin(run(request))),
        }
    }

    async fn run(&self, request: AgentHookRequest) -> anyhow::Result<String> {
        (self.run)(request).await
    }
}

pub(crate) async fn run_agent(
    runner: Option<&AgentHookRunner>,
    handler: &ConfiguredHandler,
    input_json: &str,
    default_model: String,
) -> CommandRunResult {
    let started_at = chrono::Utc::now().timestamp();
    let started = std::time::Instant::now();

    let ConfiguredHandlerKind::Agent {
        prompt,
        model,
        timeout_sec,
        continue_on_block,
    } = &handler.kind
    else {
        return model_hook_run_result(
            started_at,
            started,
            /*exit_code*/ None,
            String::new(),
            Some("non-agent handler cannot run as an agent hook".to_string()),
        );
    };
    let Some(runner) = runner else {
        return model_hook_run_result(
            started_at,
            started,
            /*exit_code*/ None,
            String::new(),
            Some("agent hook cannot run because no agent runner is configured".to_string()),
        );
    };

    let request = AgentHookRequest {
        prompt: render_model_hook_prompt(prompt, input_json),
        model: model.clone().unwrap_or(default_model),
    };

    match timeout(Duration::from_secs(*timeout_sec), runner.run(request)).await {
        Ok(Ok(output)) => match model_hook_output_to_command_stdout(
            "agent",
            handler.event_name,
            *continue_on_block,
            &output,
        ) {
            Ok(stdout) => {
                model_hook_run_result(started_at, started, Some(0), stdout, /*error*/ None)
            }
            Err(error) => model_hook_run_result(
                started_at,
                started,
                /*exit_code*/ None,
                String::new(),
                Some(error),
            ),
        },
        Ok(Err(error)) => model_hook_run_result(
            started_at,
            started,
            /*exit_code*/ None,
            String::new(),
            Some(error.to_string()),
        ),
        Err(_) => model_hook_run_result(
            started_at,
            started,
            /*exit_code*/ None,
            String::new(),
            Some(format!("agent hook timed out after {timeout_sec}s")),
        ),
    }
}

#[cfg(test)]
#[path = "agent_runner_tests.rs"]
mod tests;
