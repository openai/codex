use anyhow::{bail, Context, Result};
use reqwest::{header::CONTENT_TYPE, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::fmt;
use std::time::Duration;

pub const DEFAULT_STATUS_WIDGET_URL: &str = "https://status.openai.com/proxy/status.openai.com";
pub const STATUS_WIDGET_ENV_VAR: &str = "STATUS_WIDGET_URL";
pub const CODEX_COMPONENT_NAME: &str = "Codex";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentHealth {
    Operational,
    DegradedPerformance,
    PartialOutage,
    MajorOutage,
    UnderMaintenance,
    #[serde(other)]
    Unknown,
}

impl ComponentHealth {
    fn operational() -> Self {
        Self::Operational
    }

    pub fn is_operational(self) -> bool {
        self == Self::Operational
    }

}

impl fmt::Display for ComponentHealth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = Value::from(self)
            .as_str()
            .ok_or(fmt::Error)?;

        f.write_str(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AffectedComponent {
    pub component_id: String,
    #[serde(default = "ComponentHealth::operational")]
    pub status: ComponentHealth,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
    pub id: String,
    pub name: String,
    pub status_page_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Summary {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub components: Vec<Component>,
    #[serde(default)]
    pub affected_components: Vec<AffectedComponent>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StatusPayload {
    pub summary: Summary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodexStatus {
    pub component_id: String,
    pub name: String,
    pub status: ComponentHealth,
    pub is_operational: bool,
    pub raw_affected: Option<AffectedComponent>,
}

#[derive(Debug, Clone)]
pub struct StatusClient {
    client: reqwest::Client,
    widget_url: Url,
}

impl StatusClient {
    pub fn new() -> Result<Self> {
        let widget_url = env::var(STATUS_WIDGET_ENV_VAR)
            .unwrap_or_else(|_| DEFAULT_STATUS_WIDGET_URL.to_string());
        Self::with_widget_url(Url::parse(&widget_url)?)
    }

    pub fn with_widget_url(widget_url: Url) -> Result<Self> {
        let version = env!("CARGO_PKG_VERSION");
        let user_agent = format!("codex-status/{version}");

        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .context("building HTTP client")?;

        Ok(Self { client, widget_url })
    }

    pub async fn fetch_status_payload(&self) -> Result<StatusPayload> {
        let response = self
            .client
            .get(self.widget_url.clone())
            .send()
            .await
            .context("requesting status widget")?
            .error_for_status()
            .context("status widget returned error")?;

        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_ascii_lowercase();

        if !content_type.contains("json") {
            let snippet = response
                .text()
                .await
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect::<String>();

            let url = &self.widget_url;
            bail!(
                "Expected JSON from {url}, got Content-Type={content_type}. Body starts with: {snippet:?}"
            );
        }

        response
            .json::<StatusPayload>()
            .await
            .context("parsing status widget JSON")
    }

    pub async fn fetch_codex_status(&self) -> Result<CodexStatus> {
        let payload = self.fetch_status_payload().await?;
        derive_component_status(&payload, CODEX_COMPONENT_NAME)
    }
}

pub fn derive_component_status(payload: &StatusPayload, component_name: &str) -> Result<CodexStatus> {
    let component = payload
        .summary
        .components
        .iter()
        .find(|component| component.name == component_name)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Component {component_name:?} not found in status summary"))?;

    let affected = payload
        .summary
        .affected_components
        .iter()
        .find(|affected| affected.component_id == component.id)
        .cloned();

    let status = affected
        .as_ref()
        .map(|affected| affected.status)
        .unwrap_or(ComponentHealth::Operational);

    let is_operational = status.is_operational();

    Ok(CodexStatus {
        component_id: component.id,
        name: component.name,
        status,
        is_operational,
        raw_affected: affected,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn defaults_to_operational_when_not_affected() {
        let payload = serde_json::from_value::<StatusPayload>(json!({
            "summary": {
                "id": "sum-1",
                "name": "OpenAI",
                "components": [
                    {"id": "cmp-1", "name": "Codex", "status_page_id": "page-1"},
                    {"id": "cmp-2", "name": "Chat", "status_page_id": "page-1"}
                ]
            }
        }))
        .expect("valid payload");

        let status = derive_component_status(&payload, "Codex").expect("codex component exists");

        assert_eq!(status.status, ComponentHealth::Operational);
        assert!(status.is_operational);
        assert!(status.raw_affected.is_none());
    }

    #[test]
    fn uses_affected_component_status() {
        let payload = serde_json::from_value::<StatusPayload>(json!({
            "summary": {
                "id": "sum-1",
                "name": "OpenAI",
                "components": [
                    {"id": "cmp-1", "name": "Codex", "status_page_id": "page-1"}
                ],
                "affected_components": [
                    {"component_id": "cmp-1", "status": "major_outage"}
                ]
            }
        }))
        .expect("valid payload");

        let status = derive_component_status(&payload, "Codex").expect("codex component exists");

        assert_eq!(status.status, ComponentHealth::MajorOutage);
        assert!(!status.is_operational);
        assert_eq!(
            status
                .raw_affected
                .as_ref()
                .map(|affected| affected.status),
            Some(ComponentHealth::MajorOutage)
        );
    }

    #[test]
    fn unknown_status_is_preserved_as_unknown() {
        let payload = serde_json::from_value::<StatusPayload>(json!({
            "summary": {
                "id": "sum-1",
                "name": "OpenAI",
                "components": [
                    {"id": "cmp-1", "name": "Codex", "status_page_id": "page-1"}
                ],
                "affected_components": [
                    {"component_id": "cmp-1", "status": "custom_status"}
                ]
            }
        }))
        .expect("valid payload");

        let status = derive_component_status(&payload, "Codex").expect("codex component exists");

        assert_eq!(status.status, ComponentHealth::Unknown);
        assert!(!status.is_operational);
    }

    #[test]
    fn missing_component_returns_error() {
        let payload = serde_json::from_value::<StatusPayload>(json!({
            "summary": {
                "id": "sum-1",
                "name": "OpenAI",
                "components": [],
                "affected_components": []
            }
        }))
        .expect("valid payload");

        let error = derive_component_status(&payload, "Codex").expect_err("missing component should error");

        assert!(error
            .to_string()
            .contains("Component \"Codex\" not found in status summary"));
    }
}
