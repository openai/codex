use codex_core::config::Config;
use codex_http_state::HttpStateContext;
use codex_http_state::HttpStateSurface;
use serde::Deserialize;

use crate::chatgpt_client::chatgpt_get_request_with_http_state;

#[derive(Debug, Deserialize)]
pub struct GetTaskResponse {
    pub current_diff_task_turn: Option<AssistantTurn>,
}

// Only relevant fields for our extraction
#[derive(Debug, Deserialize)]
pub struct AssistantTurn {
    pub output_items: Vec<OutputItem>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum OutputItem {
    #[serde(rename = "pr")]
    Pr(PrOutputItem),

    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
pub struct PrOutputItem {
    pub output_diff: OutputDiff,
}

#[derive(Debug, Deserialize)]
pub struct OutputDiff {
    pub diff: String,
}

pub(crate) async fn get_task(config: &Config, task_id: String) -> anyhow::Result<GetTaskResponse> {
    let path = format!("/wham/tasks/{task_id}");
    chatgpt_get_request_with_http_state(
        config,
        path,
        HttpStateContext::new(config.codex_home.to_path_buf(), HttpStateSurface::CodexCli),
    )
    .await
}
