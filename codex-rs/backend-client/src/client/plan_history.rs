//! Seven-day allowance snapshots, preserving historical limits and unknown accounting.

use super::Client;
use super::PathStyle;
use super::RequestError;
use http::Method;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PlanLimitHistory {
    pub data_as_of: Option<String>,
    pub coverage_start: Option<String>,
    pub coverage_complete: bool,
    #[serde(default = "approximate_by_default")]
    pub approximate: bool,
    pub boundary_tolerance_seconds: Option<u32>,
    pub periods: Vec<PlanLimitPeriod>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PlanLimitPeriod {
    pub id: String,
    pub window_minutes: u32,
    pub plan_type: String,
    pub starts_at: String,
    pub ends_at: String,
    pub accounting_complete: bool,
    /// Hundredths of a percent of this period's historical allowance; null is unknown.
    pub used_basis_points: Option<f64>,
    pub breakdowns: Option<Vec<PlanLimitBreakdown>>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanLimitDimension {
    ThreadSource,
    TurnTrigger,
    Model,
    Surface,
    /// Additive server dimensions must not discard the supported breakdowns.
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PlanLimitBreakdown {
    pub dimension: PlanLimitDimension,
    pub rows: Vec<PlanLimitValue>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PlanLimitValue {
    pub key: String,
    pub basis_points: f64,
}

fn approximate_by_default() -> bool {
    true
}

impl Client {
    /// Fetches seven days of snapshot history. An undeployed route has no report.
    pub async fn get_plan_limit_history(&self) -> Result<Option<PlanLimitHistory>, RequestError> {
        let prefix = match self.path_style {
            PathStyle::CodexApi => "api/codex",
            PathStyle::ChatGptApi => "wham",
        };
        let url = format!("{}/{prefix}/usage/plan_limit_history?days=7", self.base_url);
        let request = self.request(Method::GET, &url).headers(self.headers());
        let (body, _) = match self.exec_request_detailed(request, "GET", &url).await {
            Ok(response) => response,
            Err(error) if error.status().is_some_and(|status| status.as_u16() == 404) => {
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        serde_json::from_str(&body)
            .map(Some)
            .map_err(|_| RequestError::Other(anyhow::anyhow!("Invalid plan history response.")))
    }
}

#[cfg(test)]
#[path = "plan_history_tests.rs"]
mod tests;
