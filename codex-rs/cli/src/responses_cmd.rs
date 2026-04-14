use clap::Parser;
use codex_core::config::Config;
use codex_utils_cli::CliConfigOverrides;
use serde_json::json;
use tokio::io::AsyncReadExt;

#[derive(Debug, Parser)]
pub(crate) struct ResponsesCommand {}

pub(crate) async fn run_responses_command(
    root_config_overrides: CliConfigOverrides,
) -> anyhow::Result<()> {
    let mut payload_text = String::new();
    tokio::io::stdin().read_to_string(&mut payload_text).await?;
    if payload_text.trim().is_empty() {
        anyhow::bail!("expected Responses API JSON payload on stdin");
    }

    let payload: serde_json::Value = serde_json::from_str(&payload_text)
        .map_err(|err| anyhow::anyhow!("failed to parse Responses API JSON payload: {err}"))?;
    if payload.get("stream").and_then(serde_json::Value::as_bool) != Some(true) {
        anyhow::bail!("codex responses expects a streaming payload with `\"stream\": true`");
    }

    let cli_overrides = root_config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;
    let config = Config::load_with_cli_overrides(cli_overrides).await?;
    let base_auth_manager = codex_login::AuthManager::shared_from_config(
        &config, /*enable_codex_api_key_env*/ true,
    );
    let auth_manager =
        codex_login::auth_manager_for_provider(Some(base_auth_manager), &config.model_provider);
    let auth = match auth_manager {
        Some(auth_manager) => auth_manager.auth().await,
        None => None,
    };
    let api_provider = config
        .model_provider
        .to_api_provider(auth.as_ref().map(codex_login::CodexAuth::auth_mode))?;
    let api_auth = codex_login::auth_provider_from_auth(auth, &config.model_provider)?;
    let client = codex_api::ResponsesClient::new(
        codex_api::ReqwestTransport::new(codex_login::default_client::build_reqwest_client()),
        api_provider,
        api_auth,
    );

    let mut stream = client
        .stream(
            payload,
            Default::default(),
            codex_api::Compression::None,
            None,
        )
        .await?;
    while let Some(event) = stream.rx_event.recv().await {
        let event = event?;
        println!("{}", serde_json::to_string(&response_event_to_json(event))?);
    }

    Ok(())
}

fn response_event_to_json(event: codex_api::ResponseEvent) -> serde_json::Value {
    match event {
        codex_api::ResponseEvent::Created => json!({ "type": "response.created" }),
        codex_api::ResponseEvent::OutputItemDone(item) => {
            json!({ "type": "response.output_item.done", "item": item })
        }
        codex_api::ResponseEvent::OutputItemAdded(item) => {
            json!({ "type": "response.output_item.added", "item": item })
        }
        codex_api::ResponseEvent::ServerModel(model) => {
            json!({ "type": "response.server_model", "model": model })
        }
        codex_api::ResponseEvent::ServerReasoningIncluded(included) => {
            json!({ "type": "response.server_reasoning_included", "included": included })
        }
        codex_api::ResponseEvent::Completed {
            response_id,
            token_usage,
        } => json!({
            "type": "response.completed",
            "response_id": response_id,
            "token_usage": token_usage,
        }),
        codex_api::ResponseEvent::OutputTextDelta(delta) => {
            json!({ "type": "response.output_text.delta", "delta": delta })
        }
        codex_api::ResponseEvent::ReasoningSummaryDelta {
            delta,
            summary_index,
        } => json!({
            "type": "response.reasoning_summary.delta",
            "delta": delta,
            "summary_index": summary_index,
        }),
        codex_api::ResponseEvent::ReasoningContentDelta {
            delta,
            content_index,
        } => json!({
            "type": "response.reasoning_content.delta",
            "delta": delta,
            "content_index": content_index,
        }),
        codex_api::ResponseEvent::ReasoningSummaryPartAdded { summary_index } => {
            json!({
                "type": "response.reasoning_summary_part.added",
                "summary_index": summary_index,
            })
        }
        codex_api::ResponseEvent::RateLimits(rate_limits) => {
            json!({ "type": "response.rate_limits", "rate_limits": rate_limits })
        }
        codex_api::ResponseEvent::ModelsEtag(etag) => {
            json!({ "type": "response.models_etag", "etag": etag })
        }
    }
}
