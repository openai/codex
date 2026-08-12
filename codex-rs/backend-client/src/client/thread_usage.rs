//! Authoritative estimated credit and dollar usage for an individual Codex thread.

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
    thread_ids: [&'a str; 1],
}

#[derive(Deserialize)]
struct ThreadUsageQueryResponse {
    threads: Vec<ThreadUsage>,
}

impl Client {
    /// Reads authoritative estimated totals without maintaining a second usage ledger.
    pub async fn get_thread_usage(&self, thread_id: &str) -> Result<ThreadUsage, RequestError> {
        let url = self.thread_usage_url();
        let request = self
            .request(Method::POST, &url)
            .headers(self.headers())
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .json(&ThreadUsageQueryRequest {
                thread_ids: [thread_id],
            });
        let (body, content_type) = self.exec_request_detailed(request, "POST", &url).await?;
        let response = self
            .decode_json::<ThreadUsageQueryResponse>(&url, &content_type, &body)
            .map_err(RequestError::from)?;

        response
            .threads
            .into_iter()
            .find(|usage| usage.thread_id == thread_id)
            .ok_or_else(|| {
                RequestError::from(anyhow!(
                    "thread usage response did not contain requested thread {thread_id}"
                ))
            })
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
