use std::sync::Arc;

use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_protocol::user_input::UserInput;
use serde::Deserialize;
use serde_json::Value;
use tracing::info;

#[derive(Debug, Deserialize)]
struct EntertainmentTextOutput {
    texts: Vec<String>,
}

pub(crate) async fn generate_entertainment_texts(
    server: Arc<ThreadManager>,
    config: Config,
    prompt: String,
) -> anyhow::Result<Vec<String>> {
    info!(
        prompt_len = prompt.len(),
        "starting entertainment text generation thread"
    );
    let mut config = config;
    config.model = Some("gpt-5-nano".to_string());
    config.model_reasoning_effort = None;
    let new_thread = server.start_thread(config).await?;
    let schema = entertainment_output_schema();
    let input = vec![UserInput::Text { text: prompt }];
    new_thread
        .thread
        .submit(Op::UserInput {
            items: input,
            final_output_json_schema: Some(schema),
        })
        .await?;

    let mut output = String::new();
    while let Ok(event) = new_thread.thread.next_event().await {
        match event.msg {
            EventMsg::AgentMessage(msg) => {
                output.push_str(&msg.message);
                break;
            }
            EventMsg::Error(err) => {
                return Err(anyhow::anyhow!(err.message));
            }
            EventMsg::TaskComplete(task) => {
                if output.trim().is_empty() {
                    if let Some(message) = task.last_agent_message {
                        output = message;
                    }
                }
                break;
            }
            _ => {}
        }
    }

    let _ = new_thread.thread.submit(Op::Shutdown).await;

    if output.trim().is_empty() {
        return Err(anyhow::anyhow!(
            "entertainment generation returned empty output"
        ));
    }

    let parsed: EntertainmentTextOutput = serde_json::from_str(output.trim())?;
    let mut texts: Vec<String> = parsed
        .texts
        .into_iter()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .collect();

    info!(
        texts_len = texts.len(),
        "parsed entertainment text generation output"
    );

    if !(5..=7).contains(&texts.len()) {
        return Err(anyhow::anyhow!(
            "expected 5-7 entertainment texts, got {}",
            texts.len()
        ));
    }

    for text in &mut texts {
        *text = text.trim().to_string();
    }

    Ok(texts)
}

fn entertainment_output_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "texts": {
                "type": "array",
                "minItems": 5,
                "maxItems": 7,
                "items": { "type": "string" }
            }
        },
        "required": ["texts"],
        "additionalProperties": false
    })
}
