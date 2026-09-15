//! Authenticated account analytics reads. Attribution and missing amounts remain unmodified.

use super::Client;
use super::PathStyle;
use super::RequestError;
use codex_backend_openapi_models::models::analytics as models;
use http::Method;

/// The bounded set of reports used by consumer and workspace Analytics views.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AnalyticsReport {
    Usage,
    EnterpriseTokens,
    Credits,
    WorkspaceCredits,
    EnterpriseCredits { breakdown: &'static str },
    Messages,
    Plugins { limit: u8 },
    Skills { limit: u8 },
}

/// Endpoint-specific backend contracts; normalization belongs to the consumer.
#[derive(Clone, Debug, PartialEq)]
pub enum AnalyticsResponse {
    Usage(models::DailyProductSurfaceUsageResponse),
    Credits(models::CreditUsageEventsResponse),
    EnterpriseCredits(models::CurrentUserCreditUsageResponse),
    Messages(models::DailyWorkspaceUsageCountResponse),
    Plugins(models::PluginUsageMetricsResponse),
    Skills(models::DailySkillUsageMetricsResponse),
}

impl Client {
    /// Fetches one report for an inclusive UTC date range supplied by the caller.
    pub async fn get_account_analytics(
        &self,
        report: AnalyticsReport,
        start_date: &str,
        end_date: &str,
    ) -> Result<AnalyticsResponse, RequestError> {
        let route = match report {
            AnalyticsReport::Usage => "usage/daily-token-usage-breakdown",
            AnalyticsReport::Credits => "usage/credit-usage-events",
            AnalyticsReport::WorkspaceCredits | AnalyticsReport::EnterpriseTokens => {
                "usage/daily-workspace-user-token-usage-breakdown"
            }
            AnalyticsReport::EnterpriseCredits { .. } => "usage/daily-workspace-user-credit-usage",
            AnalyticsReport::Messages => "analytics/daily-workspace-usage-counts",
            AnalyticsReport::Plugins { .. } => "analytics/daily-plugin-usage-metrics",
            AnalyticsReport::Skills { .. } => "analytics/daily-skill-usage-metrics",
        };
        let prefix = match self.path_style {
            PathStyle::CodexApi => "api/codex",
            PathStyle::ChatGptApi => "wham",
        };
        let mut url = url::Url::parse(&format!("{}/{prefix}/{route}", self.base_url))
            .map_err(anyhow::Error::from)?;
        if report != AnalyticsReport::Credits {
            url.query_pairs_mut()
                .append_pair("start_date", start_date)
                .append_pair("end_date", end_date);
            if let AnalyticsReport::EnterpriseCredits { breakdown } = report {
                url.query_pairs_mut().append_pair("breakdown", breakdown);
            } else {
                url.query_pairs_mut().append_pair("group_by", "day");
            }
        }
        if matches!(
            report,
            AnalyticsReport::Messages
                | AnalyticsReport::Plugins { .. }
                | AnalyticsReport::Skills { .. }
        ) {
            url.query_pairs_mut().append_pair("workspace_user", "true");
        }
        match report {
            AnalyticsReport::EnterpriseTokens => {
                url.query_pairs_mut()
                    .append_pair("breakdown_by", "model")
                    .append_pair("modes", "codex")
                    .append_pair("modes", "work");
            }
            AnalyticsReport::Plugins { limit } => {
                url.query_pairs_mut()
                    .append_pair("top_plugin_limit", &limit.to_string());
            }
            AnalyticsReport::Skills { limit } => {
                url.query_pairs_mut()
                    .append_pair("top_skill_limit", &limit.to_string());
            }
            AnalyticsReport::Usage
            | AnalyticsReport::Credits
            | AnalyticsReport::WorkspaceCredits
            | AnalyticsReport::EnterpriseCredits { .. }
            | AnalyticsReport::Messages => {}
        }
        let request = self
            .request(Method::GET, url.as_str())
            .headers(self.headers());
        let (body, _) = self
            .exec_request_detailed(request, "GET", url.as_str())
            .await?;
        // Do not include an untyped billing response in errors or diagnostics.
        let response = match report {
            AnalyticsReport::Usage
            | AnalyticsReport::EnterpriseTokens
            | AnalyticsReport::WorkspaceCredits => {
                serde_json::from_str(&body).map(AnalyticsResponse::Usage)
            }
            AnalyticsReport::Credits => serde_json::from_str(&body).map(AnalyticsResponse::Credits),
            AnalyticsReport::EnterpriseCredits { .. } => {
                serde_json::from_str(&body).map(AnalyticsResponse::EnterpriseCredits)
            }
            AnalyticsReport::Messages => {
                serde_json::from_str(&body).map(AnalyticsResponse::Messages)
            }
            AnalyticsReport::Plugins { .. } => {
                serde_json::from_str(&body).map(AnalyticsResponse::Plugins)
            }
            AnalyticsReport::Skills { .. } => {
                serde_json::from_str(&body).map(AnalyticsResponse::Skills)
            }
        };
        response.map_err(|_| RequestError::Other(anyhow::anyhow!("Invalid analytics response.")))
    }
}

#[cfg(test)]
#[path = "analytics_tests.rs"]
mod tests;
