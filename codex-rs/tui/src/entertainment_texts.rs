use std::collections::VecDeque;
use std::sync::Arc;

use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_protocol::user_input::UserInput;
use ratatui::text::Line;
use serde::Deserialize;
use serde_json::Value;
use tracing::info;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::history_cell::HistoryCell;

const PROMPT_TEMPLATE: &str = include_str!("entertainment_prompt.md");
const HISTORY_LIMIT: usize = 10;

#[derive(Debug)]
pub(crate) struct EntertainmentTextManager {
    enabled: bool,
    history: VecDeque<String>,
}

#[derive(Debug, Deserialize)]
struct EntertainmentTextOutput {
    texts: Vec<String>,
}

impl EntertainmentTextManager {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            history: VecDeque::new(),
        }
    }

    pub(crate) fn record_history_cell(&mut self, cell: &dyn HistoryCell) {
        let lines = cell.transcript_lines(u16::MAX);
        let text = render_lines(&lines).join("\n");
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        self.history.push_back(text.to_string());
        while self.history.len() > HISTORY_LIMIT {
            self.history.pop_front();
        }
    }

    pub(crate) fn request_generation(&self, app_event_tx: &AppEventSender) {
        if !self.enabled {
            return;
        }
        let prompt = self.build_prompt();
        info!(
            history_len = self.history.len(),
            prompt_len = prompt.len(),
            "requesting entertainment text generation"
        );
        app_event_tx.send(AppEvent::GenerateEntertainmentTexts { prompt });
    }

    fn build_prompt(&self) -> String {
        let history = if self.history.is_empty() {
            "- (no recent history)".to_string()
        } else {
            let mut out = String::new();
            for entry in &self.history {
                out.push_str("- ");
                out.push_str(entry);
                out.push('\n');
            }
            out.trim_end().to_string()
        };

        PROMPT_TEMPLATE.replace("{{INSERT_CONTEXT_HERE}}", &history)
    }
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
    config.model = Some("gpt-4.1-nano".to_string());
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

fn render_lines(lines: &[Line<'static>]) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect()
}
