//! Adapts workspace-visible SiWC estimates to the shared turn-cost emission path.
//! Emit only nonnegative, visible amounts covering every observed completed response;
//! missing amounts/settlement data must never be interpreted as a zero-cost turn.

use super::ApiKeyTurnCost;
use super::ApiKeyTurnCostStatus;
use super::BackendClient;
use super::RequestError;
use super::WorkerRuntime;
use codex_login::CodexAuth;
use std::collections::BTreeMap;

impl WorkerRuntime {
    pub(super) async fn query_chatgpt_turn_costs(
        &self,
        auth: &CodexAuth,
        http_client_factory: codex_http_client::HttpClientFactory,
        turn_ids: &[String],
    ) -> Result<Vec<ApiKeyTurnCost>, RequestError> {
        let mut threads: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for turn_id in turn_ids {
            // Availability probes use an untracked, random thread/turn pair.
            let thread_id = self
                .turns
                .get(turn_id)
                .map(|entry| entry.thread_id)
                .unwrap_or_default();
            threads
                .entry(thread_id.to_string())
                .or_default()
                .push(turn_id.clone());
        }
        let client = BackendClient::from_auth(
            self.config.chatgpt_base_url.clone(),
            auth,
            http_client_factory,
        );
        let costs = client.query_chatgpt_turn_costs(&threads).await?;
        let mut settled_costs = Vec::new();
        for thread in costs {
            let Some(requested_turns) = threads.get(&thread.thread_id) else {
                continue;
            };
            for cost in thread.turns {
                if !requested_turns.contains(&cost.turn_id) {
                    continue;
                }
                let Some(entry) = self.turns.get(&cost.turn_id) else {
                    continue;
                };
                let (Some(micros), Some(settled_ids)) =
                    (cost.estimated_usage_usd_micros, cost.settled_response_ids)
                else {
                    continue;
                };
                // Missing/hidden dollars are not zero. A projection can appear before
                // all responses settle, so require every locally observed response.
                if micros < 0
                    || entry.expected_response_ids.is_empty()
                    || !entry
                        .expected_response_ids
                        .iter()
                        .all(|id| settled_ids.contains(id))
                {
                    continue;
                }
                // Reuse the existing emission path without a floating-point conversion.
                settled_costs.push(ApiKeyTurnCost {
                    turn_id: cost.turn_id,
                    status: ApiKeyTurnCostStatus::Priced,
                    total_usd: Some(format!("{}.{:06}", micros / 1_000_000, micros % 1_000_000)),
                    event_count: Some(settled_ids.len() as u64),
                    responses: None,
                    model: cost.model,
                    speed: None,
                    reasoning_effort: None,
                });
            }
        }
        Ok(settled_costs)
    }
}
