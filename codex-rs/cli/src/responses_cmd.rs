use clap::Parser;
use codex_core::config::Config;
use codex_model_provider::create_model_provider;
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
    let model_provider = create_model_provider(config.model_provider, Some(base_auth_manager));
    let api_provider = model_provider.api_provider().await?;
    let api_auth = model_provider.api_auth().await?;
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
            /*turn_state*/ None,
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
        codex_api::ResponseEvent::Created => {
            json!({ "type": "response.created", "response": {} })
        }
        codex_api::ResponseEvent::OutputItemDone(item) => {
            json!({ "type": "response.output_item.done", "item": item })
        }
        codex_api::ResponseEvent::OutputItemAdded(item) => {
            json!({ "type": "response.output_item.added", "item": item })
        }
        codex_api::ResponseEvent::ServerModel(model) => {
            json!({ "type": "response.server_model", "model": model })
        }
        codex_api::ResponseEvent::ModelVerifications(verifications) => {
            json!({ "type": "response.model_verifications", "verifications": verifications })
        }
        codex_api::ResponseEvent::ServerReasoningIncluded(included) => {
            json!({ "type": "response.server_reasoning_included", "included": included })
        }
        codex_api::ResponseEvent::Completed {
            response_id,
            token_usage,
            // FIXME(andrey): Make this actually work + test it, before merging.
            end_turn: _,
        } => {
            let response = match token_usage {
                Some(token_usage) => json!({
                    "id": response_id,
                    "usage": {
                        "input_tokens": token_usage.input_tokens,
                        "input_tokens_details": {
                            "cached_tokens": token_usage.cached_input_tokens,
                        },
                        "output_tokens": token_usage.output_tokens,
                        "output_tokens_details": {
                            "reasoning_tokens": token_usage.reasoning_output_tokens,
                        },
                        "total_tokens": token_usage.total_tokens,
                    },
                }),
                None => json!({ "id": response_id }),
            };
            json!({ "type": "response.completed", "response": response })
        }
        codex_api::ResponseEvent::OutputTextDelta(delta) => {
            json!({ "type": "response.output_text.delta", "delta": delta })
        }
        codex_api::ResponseEvent::ToolCallInputDelta {
            item_id,
            call_id,
            delta,
        } => {
            json!({
                "type": "response.tool_call_input.delta",
                "item_id": item_id,
                "call_id": call_id,
                "delta": delta,
            })
        }
        codex_api::ResponseEvent::ReasoningSummaryDelta {
            delta,
            summary_index,
        } => json!({
            "type": "response.reasoning_summary_text.delta",
            "delta": delta,
            "summary_index": summary_index,
        }),
        codex_api::ResponseEvent::ReasoningContentDelta {
            delta,
            content_index,
        } => json!({
            "type": "response.reasoning_text.delta",
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

#[cfg(test)]
mod tests {
    use super::response_event_to_json;
    use codex_protocol::protocol::TokenUsage;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn response_events_keep_replayable_response_envelopes() {
        let created = response_event_to_json(codex_api::ResponseEvent::Created);
        assert_eq!(created, json!({"type": "response.created", "response": {}}));

        let completed = response_event_to_json(codex_api::ResponseEvent::Completed {
            response_id: "resp-1".to_string(),
            token_usage: Some(TokenUsage {
                input_tokens: 10,
                cached_input_tokens: 4,
                output_tokens: 7,
                reasoning_output_tokens: 3,
                total_tokens: 17,
            }),
            end_turn: None,
        });
        assert_eq!(
            completed,
            json!({
                "type": "response.completed",
                "response": {
                    "id": "resp-1",
                    "usage": {
                        "input_tokens": 10,
                        "input_tokens_details": {
                            "cached_tokens": 4,
                        },
                        "output_tokens": 7,
                        "output_tokens_details": {
                            "reasoning_tokens": 3,
                        },
                        "total_tokens": 17,
                    },
                },
            })
        );

        let completed_without_usage = response_event_to_json(codex_api::ResponseEvent::Completed {
            response_id: "resp-2".to_string(),
            token_usage: None,
            end_turn: None,
        });
        assert_eq!(
            completed_without_usage,
            json!({"type": "response.completed", "response": {"id": "resp-2"}})
        );
    }

    #[test]
    fn reasoning_deltas_use_responses_event_names() {
        let summary = response_event_to_json(codex_api::ResponseEvent::ReasoningSummaryDelta {
            delta: "plan".to_string(),
            summary_index: 1,
        });
        assert_eq!(
            summary,
            json!({
                "type": "response.reasoning_summary_text.delta",
                "delta": "plan",
                "summary_index": 1,
            })
        );

        let content = response_event_to_json(codex_api::ResponseEvent::ReasoningContentDelta {
            delta: "detail".to_string(),
            content_index: 2,
        });
        assert_eq!(
            content,
            json!({
                "type": "response.reasoning_text.delta",
                "delta": "detail",
                "content_index": 2,
            })
        );
    }

    #[test]
    fn tool_call_input_delta_uses_responses_event_name() {
        let delta = response_event_to_json(codex_api::ResponseEvent::ToolCallInputDelta {
            item_id: "item-1".to_string(),
            call_id: Some("call-1".to_string()),
            delta: "patch".to_string(),
        });
        assert_eq!(
            delta,
            json!({
                "type": "response.tool_call_input.delta",
                "item_id": "item-1",
                "call_id": "call-1",
                "delta": "patch",
            })
        );
    }
}
