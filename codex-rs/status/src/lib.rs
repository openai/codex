use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use reqwest::header::CONTENT_TYPE;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone, Display, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    pub fn is_operational(self) -> bool {
        self == Self::Operational
    }
}

pub async fn fetch_codex_health() -> Result<ComponentHealth> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()
        .context("building HTTP client")?;

    let response = client
        .get(STATUS_WIDGET_URL)
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

        bail!(
            "Expected JSON from {STATUS_WIDGET_URL}: Content-Type={content_type}. Body starts with: {snippet:?}"
        );
    }

    let payload = response
        .json::<StatusPayload>()
        .await
        .context("parsing status widget JSON")?;

    derive_component_health(&payload, CODEX_COMPONENT_NAME)
}

const STATUS_WIDGET_URL: &str = "https://status.openai.com/proxy/status.openai.com";
const CODEX_COMPONENT_NAME: &str = "Codex";

#[derive(Debug, Clone, Deserialize)]
struct StatusPayload {
    #[serde(default)]
    summary: Summary,
}

#[derive(Debug, Clone, Deserialize)]
struct Summary {
    #[serde(default)]
    components: Vec<Component>,
    #[serde(default)]
    affected_components: Vec<AffectedComponent>,
}

#[derive(Debug, Clone, Deserialize)]
struct Component {
    id: String,
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct AffectedComponent {
    component_id: String,
    #[serde(default = "ComponentHealth::operational")]
    status: ComponentHealth,
}

fn derive_component_health(
    payload: &StatusPayload,
    component_name: &str,
) -> Result<ComponentHealth> {
    let component = payload
        .summary
        .components
        .iter()
        .find(|component| component.name == component_name)
        .ok_or_else(|| anyhow!("Component {component_name:?} not found in status summary"))?;

    let status = payload
        .summary
        .affected_components
        .iter()
        .find(|affected| affected.component_id == component.id)
        .map(|affected| affected.status)
        .unwrap_or(ComponentHealth::Operational);

    Ok(status)
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

        let status = derive_component_health(&payload, "Codex").expect("codex component exists");

        assert_eq!(status, ComponentHealth::Operational);
        assert!(status.is_operational());
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

        let status = derive_component_health(&payload, "Codex").expect("codex component exists");

        assert_eq!(status, ComponentHealth::MajorOutage);
        assert!(!status.is_operational());
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

        let status = derive_component_health(&payload, "Codex").expect("codex component exists");

        assert_eq!(status, ComponentHealth::Unknown);
        assert!(!status.is_operational());
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

        let error =
            derive_component_health(&payload, "Codex").expect_err("missing component should error");

        assert!(
            error
                .to_string()
                .contains("Component \"Codex\" not found in status summary")
        );
    }
}
