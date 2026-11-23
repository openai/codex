//! Fetches server-evaluated Statsig values using the existing Codex backend identity.

use std::time::Duration;

use anyhow::Result;
use anyhow::anyhow;
use http::Method;
use serde::Deserialize;
use serde::Serialize;

use super::Client;
use super::PathStyle;

/// Client-supplied targeting fields. The backend adds authenticated account and user fields.
#[derive(Clone, Serialize)]
pub struct StatsigBootstrapContext {
    /// Statsig Target App name. The backend maps it to a client SDK key.
    pub target_app: &'static str,
    pub stable_id: String,
    pub app_version: String,
    pub locale: Option<String>,
    pub system_name: String,
    pub system_version: String,
    pub window_type: &'static str,
    pub brand_name: &'static str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatsigBootstrapResponse {
    statsig_payload: String,
}

impl Client {
    pub async fn statsig_bootstrap(&self, context: &StatsigBootstrapContext) -> Result<String> {
        let path = match self.path_style {
            PathStyle::CodexApi => "/api/codex/statsig/bootstrap",
            PathStyle::ChatGptApi => "/wham/statsig/bootstrap",
        };
        let url = format!("{}{path}", self.base_url);
        self.auth.validate_destination(&self.base_url, &url)?;
        let response = self
            .request(Method::POST, &url)
            .headers(self.headers().await?)
            .timeout(Duration::from_secs(10))
            .json(context)
            .send()
            .await
            .map_err(codex_http_client::RouteAwareRequestError::without_url)?;
        let status = response.status();
        anyhow::ensure!(status.is_success(), "Statsig bootstrap returned {status}");
        let response: StatsigBootstrapResponse = response
            .json()
            .await
            .map_err(|_| anyhow!("Could not read the Statsig bootstrap response"))?;
        Ok(response.statsig_payload)
    }
}
