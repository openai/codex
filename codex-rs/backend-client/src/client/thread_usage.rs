//! Authoritative estimated credit and dollar usage for bounded batches of Codex threads.

use super::Client;
use super::PathStyle;
use super::RequestError;
use anyhow::anyhow;
use http::Method;
use http::header::CONTENT_TYPE;
use http::header::HeaderValue;
use serde::Deserialize;
use serde::Serialize;

/// Backend usage grouped by model, reasoning effort, and response speed.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ThreadUsageBreakdownGroup {
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub speed: Option<String>,
    pub estimated_usage_credits_micros: i64,
    pub net_new_input_tokens: Option<i64>,
    pub cached_input_tokens: Option<i64>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
}

/// Backend-estimated usage totals expressed in integer millionths.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ThreadUsage {
    pub thread_id: String,
    pub estimated_usage_credits_micros: i64,
    pub estimated_usage_usd_micros: Option<i64>,
    #[serde(default)]
    pub groups: Vec<ThreadUsageBreakdownGroup>,
}

#[derive(Serialize)]
struct ThreadUsageQueryRequest<'a> {
    thread_ids: &'a [&'a str],
}

#[derive(Deserialize)]
struct ThreadUsageQueryResponse {
    threads: Vec<ThreadUsageEstimate>,
}

// Preserve unavailable rows until all returned IDs have been validated.
#[derive(Deserialize)]
struct ThreadUsageEstimate {
    thread_id: String,
    estimated_usage_credits_micros: Option<i64>,
    estimated_usage_usd_micros: Option<i64>,
    groups: Option<Vec<ThreadUsageBreakdownGroup>>,
}

impl Client {
    /// Reads authoritative estimated totals without maintaining a second usage ledger.
    pub async fn get_thread_usage(&self, thread_id: &str) -> Result<ThreadUsage, RequestError> {
        self.get_threads_usage(&[thread_id])
            .await?
            .into_iter()
            .find(|usage| usage.thread_id == thread_id)
            .ok_or_else(|| {
                RequestError::from(anyhow!("thread usage response omitted requested thread"))
            })
    }

    /// Reads at most 100 distinct threads; omitted results are unavailable, not zero.
    pub async fn get_threads_usage(
        &self,
        thread_ids: &[&str],
    ) -> Result<Vec<ThreadUsage>, RequestError> {
        let requested: std::collections::HashSet<_> = thread_ids.iter().copied().collect();
        if thread_ids.is_empty() || thread_ids.len() > 100 || requested.len() != thread_ids.len() {
            return Err(RequestError::from(anyhow!(
                "expected 1–100 distinct thread IDs"
            )));
        }
        let url = self.thread_usage_url();
        let request = self
            .request(Method::POST, &url)
            .headers(self.headers())
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .json(&ThreadUsageQueryRequest { thread_ids });
        let (body, _) = self.exec_request_detailed(request, "POST", &url).await?;
        let response: ThreadUsageQueryResponse = serde_json::from_str(&body)
            .map_err(|_| RequestError::Other(anyhow!("Invalid thread usage response.")))?;
        let mut seen = std::collections::HashSet::new();
        if response
            .threads
            .iter()
            .any(|row| !requested.contains(row.thread_id.as_str()) || !seen.insert(&row.thread_id))
        {
            return Err(RequestError::from(anyhow!(
                "thread usage returned unexpected threads"
            )));
        }
        Ok(response
            .threads
            .into_iter()
            .filter_map(|row| {
                Some(ThreadUsage {
                    thread_id: row.thread_id,
                    estimated_usage_credits_micros: row.estimated_usage_credits_micros?,
                    estimated_usage_usd_micros: row.estimated_usage_usd_micros,
                    groups: row.groups.unwrap_or_default(),
                })
            })
            .collect())
    }

    fn thread_usage_url(&self) -> String {
        match self.path_style {
            PathStyle::CodexApi => {
                format!("{}/api/codex/usage/thread_usage/query", self.base_url)
            }
            PathStyle::ChatGptApi => {
                format!("{}/wham/usage/thread_usage/query", self.base_url)
            }
        }
    }
}

#[cfg(test)]
#[path = "thread_usage_tests.rs"]
mod tests;
