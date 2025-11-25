use anyhow::Context;
use reqwest::Url;
use serde::Deserialize;
use serde::Serialize;
use std::time::Duration;

pub const STATUS_SUMMARY_URL: &str = "https://status.openai.com/api/v2/summary.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodexStatusReport {
    pub overall_description: String,
    pub overall_indicator: String,
    pub updated_at: String,
    pub components: Vec<ComponentStatus>,
    pub incidents: Vec<IncidentStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ComponentStatus {
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IncidentStatus {
    pub name: String,
    pub status: String,
    pub impact: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct StatusClient {
    client: reqwest::Client,
    summary_url: Url,
}

impl StatusClient {
    pub fn new() -> anyhow::Result<Self> {
        Self::with_summary_url(Url::parse(STATUS_SUMMARY_URL)?)
    }

    pub fn with_summary_url(summary_url: Url) -> anyhow::Result<Self> {
        let user_agent = format!("codex-status/{}", env!("CARGO_PKG_VERSION"));
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .context("building HTTP client")?;

        Ok(Self {
            client,
            summary_url,
        })
    }

    pub async fn fetch_codex_status(&self) -> anyhow::Result<CodexStatusReport> {
        let response = self
            .client
            .get(self.summary_url.clone())
            .send()
            .await
            .context("requesting status summary")?;

        let summary: StatusSummary = response
            .error_for_status()
            .context("status summary returned error")?
            .json()
            .await
            .context("parsing status summary JSON")?;

        Ok(CodexStatusReport::from_summary(summary))
    }
}

impl CodexStatusReport {
    fn from_summary(summary: StatusSummary) -> Self {
        let components = summary
            .components
            .into_iter()
            .filter(|component| is_codex_name(&component.name))
            .map(ComponentStatus::from)
            .collect();

        let incidents = summary
            .incidents
            .into_iter()
            .filter(|incident| is_codex_name(&incident.name))
            .map(IncidentStatus::from)
            .collect();

        CodexStatusReport {
            overall_description: summary.status.description,
            overall_indicator: summary.status.indicator,
            updated_at: summary.page.updated_at,
            components,
            incidents,
        }
    }
}

fn is_codex_name(name: &str) -> bool {
    name.to_ascii_lowercase().contains("codex")
}

#[derive(Debug, Deserialize)]
struct StatusSummary {
    #[serde(default)]
    page: Page,
    #[serde(default)]
    status: OverallStatus,
    #[serde(default)]
    components: Vec<Component>,
    #[serde(default)]
    incidents: Vec<Incident>,
}

#[derive(Debug, Deserialize, Default)]
struct Page {
    #[serde(default)]
    updated_at: String,
}

#[derive(Debug, Deserialize, Default)]
struct OverallStatus {
    #[serde(default)]
    indicator: String,
    #[serde(default)]
    description: String,
}

#[derive(Debug, Deserialize)]
struct Component {
    name: String,
    status: String,
}

#[derive(Debug, Deserialize)]
struct Incident {
    name: String,
    status: String,
    #[serde(default = "default_impact")]
    impact: String,
    #[serde(default)]
    updated_at: String,
}

fn default_impact() -> String {
    "unknown".to_string()
}

impl From<Component> for ComponentStatus {
    fn from(value: Component) -> Self {
        Self {
            name: value.name,
            status: value.status,
        }
    }
}

impl From<Incident> for IncidentStatus {
    fn from(value: Incident) -> Self {
        Self {
            name: value.name,
            status: value.status,
            impact: value.impact,
            updated_at: value.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn filters_non_codex_components_and_incidents() {
        let summary = serde_json::from_value::<StatusSummary>(json!({
            "page": {"updated_at": "2025-11-07T21:55:20Z"},
            "status": {"description": "All Systems Operational", "indicator": "none"},
            "components": [
                {"name": "Codex", "status": "operational"},
                {"name": "Chat Completions", "status": "operational"}
            ],
            "incidents": [
                {"name": "Codex degraded performance", "status": "investigating", "impact": "minor", "updated_at": "2025-11-07T21:50:00Z"},
                {"name": "Chat downtime", "status": "resolved", "impact": "critical", "updated_at": "2025-11-07T21:00:00Z"}
            ]
        }))
        .expect("valid summary");

        let report = CodexStatusReport::from_summary(summary);

        assert_eq!(report.overall_description, "All Systems Operational");
        assert_eq!(report.overall_indicator, "none");
        assert_eq!(report.updated_at, "2025-11-07T21:55:20Z");
        assert_eq!(report.components.len(), 1);
        assert_eq!(report.components[0].name, "Codex");
        assert_eq!(report.components[0].status, "operational");
        assert_eq!(report.incidents.len(), 1);
        assert_eq!(report.incidents[0].name, "Codex degraded performance");
        assert_eq!(report.incidents[0].status, "investigating");
        assert_eq!(report.incidents[0].impact, "minor");
        assert_eq!(report.incidents[0].updated_at, "2025-11-07T21:50:00Z");
    }

    #[test]
    fn handles_missing_fields_with_defaults() {
        let summary =
            serde_json::from_value::<StatusSummary>(json!({})).expect("valid empty summary");

        let report = CodexStatusReport::from_summary(summary);

        assert_eq!(report.overall_description, "");
        assert_eq!(report.overall_indicator, "");
        assert_eq!(report.updated_at, "");
        assert!(report.components.is_empty());
        assert!(report.incidents.is_empty());
    }

    #[test]
    fn is_codex_name_matches_case_insensitive() {
        assert!(is_codex_name("Codex"));
        assert!(is_codex_name("my-codex-component"));
        assert!(!is_codex_name("Chat"));
    }
}
