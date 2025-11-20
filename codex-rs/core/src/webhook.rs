use std::sync::Arc;

use crate::config::types::WebhookConfig;
use crate::config::types::WebhookFormat;
use crate::protocol::TurnAbortReason;
use crate::state::TaskKind;
use codex_protocol::ConversationId;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use serde::Serialize;
use serde_json::json;
use tracing::error;
use tracing::info;
use tracing::warn;

#[derive(Debug, Clone)]
pub(crate) struct WebhookDispatcher {
    client: reqwest::Client,
    config: WebhookConfig,
}

impl WebhookDispatcher {
    pub(crate) fn new(config: WebhookConfig) -> Arc<Self> {
        let mut builder = reqwest::Client::builder();
        if let Some(timeout) = config.timeout {
            builder = builder.timeout(timeout);
        }
        let client = builder.build().unwrap_or_else(|_| reqwest::Client::new());
        Arc::new(Self { client, config })
    }

    pub(crate) async fn send(&self, payload: &WebhookPayload) {
        let mut headers = HeaderMap::new();
        for (name, value) in &self.config.headers {
            let Ok(header_name) = HeaderName::from_bytes(name.as_bytes()) else {
                warn!("invalid webhook header name: {name}");
                continue;
            };
            let Ok(header_value) = HeaderValue::from_str(value) else {
                warn!("invalid webhook header value for {name}");
                continue;
            };
            headers.insert(header_name, header_value);
        }

        let body = match self.config.format {
            Some(WebhookFormat::Dingtalk) => to_dingtalk_text(payload),
            None => json!(payload),
        };

        let mut request = self.client.post(&self.config.url).json(&body);
        if !headers.is_empty() {
            request = request.headers(headers);
        }

        let event_name = payload.event_name();
        let target_url = redact_url(&self.config.url);
        info!(%event_name, %target_url, "dispatching webhook");

        match request.send().await {
            Ok(response) => {
                let status = response.status();
                if let Err(err) = response.error_for_status_ref() {
                    let body = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "<unavailable>".to_string());
                    error!(
                        %event_name,
                        %target_url,
                        %status,
                        error = ?err,
                        %body,
                        "webhook POST returned error status"
                    );
                } else {
                    info!(%event_name, %target_url, %status, "webhook dispatched");
                }
            }
            Err(err) => error!(%event_name, %target_url, "failed to POST webhook payload: {err}"),
        }
    }
}

fn to_dingtalk_text(payload: &WebhookPayload) -> serde_json::Value {
    let text = match payload {
        WebhookPayload::TaskCompleted {
            turn_id,
            task_kind,
            cwd,
            last_agent_message,
            ..
        } => format!(
            "[codex] 任务完成 | turn={} | task={} | cwd={} | {}",
            turn_id,
            task_kind,
            cwd,
            last_agent_message
                .clone()
                .unwrap_or_else(|| "无总结".to_string())
        ),
        WebhookPayload::TaskAborted {
            turn_id,
            task_kind,
            cwd,
            reason,
            ..
        } => format!(
            "[codex] 任务中止 | turn={turn_id} | task={task_kind} | cwd={cwd} | reason={reason:?}"
        ),
    };
    json!({ "msgtype": "text", "text": { "content": text } })
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub(crate) enum WebhookPayload {
    TaskCompleted {
        thread_id: ConversationId,
        turn_id: String,
        task_kind: WebhookTaskKind,
        cwd: String,
        last_agent_message: Option<String>,
    },
    TaskAborted {
        thread_id: ConversationId,
        turn_id: String,
        task_kind: WebhookTaskKind,
        cwd: String,
        reason: TurnAbortReason,
    },
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WebhookTaskKind {
    Regular,
    Review,
    Compact,
}

impl From<TaskKind> for WebhookTaskKind {
    fn from(kind: TaskKind) -> Self {
        match kind {
            TaskKind::Regular => Self::Regular,
            TaskKind::Review => Self::Review,
            TaskKind::Compact => Self::Compact,
        }
    }
}

impl std::fmt::Display for WebhookTaskKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebhookTaskKind::Regular => write!(f, "regular"),
            WebhookTaskKind::Review => write!(f, "review"),
            WebhookTaskKind::Compact => write!(f, "compact"),
        }
    }
}

fn redact_url(url: &str) -> String {
    let without_query = url.split('?').next().unwrap_or(url);
    reqwest::Url::parse(without_query)
        .map(|parsed| parsed.to_string())
        .unwrap_or_else(|_| without_query.to_string())
}

impl WebhookPayload {
    fn event_name(&self) -> &'static str {
        match self {
            WebhookPayload::TaskCompleted { .. } => "task-completed",
            WebhookPayload::TaskAborted { .. } => "task-aborted",
        }
    }
}
