//! Queries SiWC per-turn estimates using the client's Codex or WHAM route and auth.
//! Always request settled response IDs and retain thread/turn association and missing amounts.

use super::Client;
use super::PathStyle;
use super::RequestError;
use http::Method;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

/// Workspace-visible estimated cost and settled responses for a ChatGPT turn.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ChatgptTurnCost {
    pub turn_id: String,
    pub model: Option<String>,
    pub estimated_usage_usd_micros: Option<i64>,
    pub settled_response_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ChatgptThreadTurnCosts {
    pub thread_id: String,
    pub turns: Vec<ChatgptTurnCost>,
}

#[derive(Serialize)]
struct ThreadTurnIds<'a> {
    thread_id: &'a str,
    turn_ids: &'a [String],
}

#[derive(Serialize)]
struct TurnCostsRequest<'a> {
    threads: Vec<ThreadTurnIds<'a>>,
    include_settled_response_ids: bool,
}

#[derive(Deserialize)]
struct TurnCostsResponse {
    threads: Vec<ChatgptThreadTurnCosts>,
}

impl Client {
    /// Queries per-turn estimates using the signed-in user's workspace permissions.
    pub async fn query_chatgpt_turn_costs(
        &self,
        threads: &BTreeMap<String, Vec<String>>,
    ) -> Result<Vec<ChatgptThreadTurnCosts>, RequestError> {
        let url = match self.path_style {
            PathStyle::CodexApi => {
                format!("{}/api/codex/usage/thread-estimates/query", self.base_url)
            }
            PathStyle::ChatGptApi => {
                format!("{}/wham/usage/thread-estimates/query", self.base_url)
            }
        };
        let request = self
            .request(Method::POST, &url)
            .headers(self.headers()?)
            .json(&TurnCostsRequest {
                threads: threads
                    .iter()
                    .map(|(thread_id, turn_ids)| ThreadTurnIds {
                        thread_id,
                        turn_ids,
                    })
                    .collect(),
                include_settled_response_ids: true,
            });
        let (body, content_type) = self.exec_request_detailed(request, "POST", &url).await?;
        let response: TurnCostsResponse = self
            .decode_json(&url, &content_type, &body)
            .map_err(RequestError::Other)?;
        Ok(response.threads)
    }
}

#[cfg(test)]
#[path = "chatgpt_turn_cost_tests.rs"]
mod tests;
