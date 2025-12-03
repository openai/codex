use std::sync::OnceLock;
use std::time::Duration;

use crate::codex::Session;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_client::HttpTransport;
use codex_client::Request;
use codex_client::ReqwestTransport;
use codex_client::RetryOn;
use codex_client::RetryPolicy;
use codex_client::run_with_retry;
use http::header::CONTENT_TYPE;
use reqwest::Method;
use serde::Deserialize;
use serde::Serialize;
use strum_macros::Display;

const STATUS_WIDGET_URL: &str = "https://status.openai.com/proxy/status.openai.com";
const CODEX_COMPONENT_NAME: &str = "Codex";

static TEST_STATUS_WIDGET_URL: OnceLock<String> = OnceLock::new();

#[derive(Debug, Clone, Display, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ComponentHealth {
    #[strum(to_string = "operational")]
    Operational,
    #[strum(to_string = "degraded performance")]
    DegradedPerformance,
    #[strum(to_string = "partial outage")]
    PartialOutage,
    #[strum(to_string = "major outage")]
    MajorOutage,
    #[strum(to_string = "under maintenance")]
    UnderMaintenance,
    #[serde(other)]
    #[strum(to_string = "unknown")]
    Unknown,
}

impl ComponentHealth {
    fn operational() -> Self {
        Self::Operational
    }

    pub(crate) fn is_operational(self) -> bool {
        self == Self::Operational
    }
}

pub(crate) async fn maybe_codex_status_warning(session: &Session) -> Option<String> {
    let Ok(status) = fetch_codex_health().await else {
        return None;
    };

    let previous = session.replace_codex_backend_status(status).await;
    if status.is_operational() || previous == Some(status) {
        return None;
    }

    Some(format!(
        "Codex is experiencing a {status}. If a response stalls, try again later. You can follow incident updates at status.openai.com."
    ))
}

async fn fetch_codex_health() -> Result<ComponentHealth> {
    let status_widget_url = status_widget_url();

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_millis(200))
        .timeout(Duration::from_millis(300))
        .build()
        .context("building HTTP client")?;

    let transport = ReqwestTransport::new(client);
    let policy = RetryPolicy {
        max_attempts: 0,
        base_delay: Duration::from_millis(100),
        retry_on: RetryOn {
            retry_429: true,
            retry_5xx: true,
            retry_transport: true,
        },
    };

    let response = run_with_retry(
        policy,
        || Request::new(Method::GET, status_widget_url.clone()),
        |req, _attempt| {
            let transport = transport.clone();
            async move { transport.execute(req).await }
        },
    )
    .await
    .context("requesting status widget")?;

    let content_type = response
        .headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if !content_type.contains("json") {
        let snippet = String::from_utf8_lossy(&response.body)
            .chars()
            .take(200)
            .collect::<String>();

        bail!(
            "Expected JSON from {status_widget_url}: Content-Type={content_type}. Body starts with: {snippet:?}"
        );
    }

    let payload: StatusPayload =
        serde_json::from_slice(&response.body).context("parsing status widget JSON")?;

    derive_component_health(&payload, CODEX_COMPONENT_NAME)
}

#[derive(Debug, Clone, Deserialize, Default)]
struct StatusPayload {
    #[serde(default)]
    summary: Summary,
}

#[derive(Debug, Clone, Deserialize, Default)]
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

fn status_widget_url() -> String {
    TEST_STATUS_WIDGET_URL
        .get()
        .cloned()
        .unwrap_or_else(|| STATUS_WIDGET_URL.to_string())
}

#[doc(hidden)]
#[cfg_attr(not(test), allow(dead_code))]
pub fn set_test_status_widget_url(url: impl Into<String>) {
    let _ = TEST_STATUS_WIDGET_URL.set(url.into());
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

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
