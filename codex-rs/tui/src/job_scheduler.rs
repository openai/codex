use crate::AppServerTarget;
use crate::start_app_server_for_picker;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_core::config::Config;
use codex_protocol::user_input::UserInput;
use serde::Deserialize;
use serde_json::json;
use tracing::warn;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedJobSpec {
    pub(crate) cron_expression: String,
    pub(crate) prompt: String,
    pub(crate) run_once: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ParsedJobSpecResponse {
    cron_expression: String,
    prompt: String,
    run_once: Option<bool>,
}

impl ParsedJobSpecResponse {
    fn into_parsed(self) -> std::result::Result<ParsedJobSpec, String> {
        let cron_expression = self.cron_expression.trim().to_string();
        let prompt = self.prompt.trim().to_string();
        if cron_expression.is_empty() {
            return Err("Could not determine a supported schedule from /loop input.".to_string());
        }
        if prompt.is_empty() {
            return Err("Could not determine a prompt from /loop input.".to_string());
        }
        Ok(ParsedJobSpec {
            cron_expression,
            prompt,
            run_once: self.run_once,
        })
    }
}

pub(crate) async fn parse_job_spec(
    config: Config,
    target: AppServerTarget,
    spec: String,
) -> std::result::Result<ParsedJobSpec, String> {
    let mut app_server = start_app_server_for_picker(&config, &target)
        .await
        .map_err(|err| format!("Failed to start parser session: {err}"))?;
    let result = async {
        let thread = app_server
            .start_ephemeral_thread(&config)
            .await
            .map_err(|err| format!("Failed to start parser thread: {err}"))?;

        let output_schema = json!({
            "type": "object",
            "properties": {
                "cronExpression": { "type": "string" },
                "prompt": { "type": "string" },
                "runOnce": { "type": ["boolean", "null"] }
            },
            "required": ["cronExpression", "prompt", "runOnce"],
            "additionalProperties": false
        });

        let parser_prompt = format!(
            "{}\n{}",
            concat!(
                "Convert the user's /loop spec into JSON with keys cronExpression, prompt, and runOnce.\n",
                "Only emit valid JSON matching the schema.\n",
                "Supported cronExpression values for this client are:\n",
                "- @after-turn\n",
                "- @every <N>s\n",
                "- @every <N>m\n",
                "- @every <N>h\n",
                "- @every <N>d\n",
                "Use @after-turn when no explicit interval is present.\n",
                "Set runOnce to true only when the user clearly asked for a one-shot run.\n",
                "Keep stop conditions and job intent inside prompt.\n",
                "If the schedule request is unsupported by the allowed cronExpression formats, return an empty cronExpression.\n",
                "User spec:"
            ),
            spec
        );

        app_server
            .turn_start(
                codex_protocol::ThreadId::from_string(&thread.id)
                    .map_err(|err| format!("Parser thread id was invalid: {err}"))?,
                vec![UserInput::Text {
                    text: parser_prompt,
                    text_elements: Vec::new(),
                }],
                config.cwd.to_path_buf(),
                config.permissions.approval_policy.value(),
                config.approvals_reviewer,
                config.permissions.sandbox_policy.get().clone(),
                config.model.clone().unwrap_or_default(),
                /*effort*/ None,
                /*summary*/ None,
                config.service_tier.map(Some),
                /*collaboration_mode*/ None,
                config.personality,
                Some(output_schema),
            )
            .await
            .map_err(|err| format!("Failed to run /loop parser prompt: {err}"))?;

        let mut final_message = None;
        while let Some(event) = app_server.next_event().await {
            match event {
                AppServerEvent::ServerNotification(ServerNotification::ItemCompleted(
                    notification,
                )) => {
                    if let ThreadItem::AgentMessage { text, .. } = notification.item {
                        final_message = Some(text);
                    }
                }
                AppServerEvent::ServerNotification(ServerNotification::TurnCompleted(
                    notification,
                )) => {
                    if notification.turn.error.is_some() {
                        return Err("The /loop parser turn failed.".to_string());
                    }
                    break;
                }
                AppServerEvent::Disconnected { message } => {
                    return Err(format!("The /loop parser session disconnected: {message}"));
                }
                AppServerEvent::Lagged { .. }
                | AppServerEvent::ServerNotification(_)
                | AppServerEvent::ServerRequest(_) => {}
            }
        }

        let response =
            final_message.ok_or_else(|| "The /loop parser returned no output.".to_string())?;
        let parsed: ParsedJobSpecResponse = serde_json::from_str(&response)
            .map_err(|err| format!("Invalid /loop parser JSON: {err}"))?;
        parsed.into_parsed()
    }
    .await;

    if let Err(err) = app_server.shutdown().await {
        warn!(%err, "failed to shut down temporary /loop parser session");
    }

    result
}

pub(crate) fn format_job_summary(cron_expression: &str, run_once: bool, prompt: &str) -> String {
    let mode = if run_once { "one-shot" } else { "recurring" };
    format!("{cron_expression} ({mode}) -> {prompt}")
}
