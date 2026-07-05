use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use tokio::sync::broadcast;

use crate::accessibility::node_center;
use crate::accessibility::node_is_attached;
use crate::actions::BrowserToolOutput;
use crate::actions::page_metadata;
use crate::cdp::CdpClient;
use crate::cdp::CdpEvent;
use crate::handles::BrowserHandles;

const DEFAULT_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_WAIT_TIMEOUT: Duration = Duration::from_secs(30);
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum LoadState {
    DomContentLoaded,
    #[default]
    Load,
}

impl LoadState {
    fn event_method(self) -> &'static str {
        match self {
            Self::DomContentLoaded => "Page.domContentEventFired",
            Self::Load => "Page.loadEventFired",
        }
    }

    fn is_reached(self, ready_state: &str) -> bool {
        match self {
            Self::DomContentLoaded => matches!(ready_state, "interactive" | "complete"),
            Self::Load => ready_state == "complete",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum NavigationAction {
    Goto,
    Back,
    Forward,
    Reload,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct NavigateRequest {
    pub(crate) action: NavigationAction,
    pub(crate) url: Option<String>,
    #[serde(default)]
    pub(crate) wait_until: LoadState,
    pub(crate) timeout_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum UrlMatch {
    #[default]
    Exact,
    Contains,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TextState {
    Present,
    Absent,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum NodeState {
    Visible,
    Hidden,
    Attached,
    Detached,
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum WaitRequest {
    Url {
        value: String,
        #[serde(default, rename = "match")]
        comparison: UrlMatch,
        timeout_ms: Option<u64>,
    },
    LoadState {
        state: LoadState,
        timeout_ms: Option<u64>,
    },
    Text {
        value: String,
        state: TextState,
        timeout_ms: Option<u64>,
    },
    Node {
        node_id: String,
        state: NodeState,
        timeout_ms: Option<u64>,
    },
}

pub(crate) async fn navigate(client: &CdpClient, url: &str) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(15), async {
        let events = client.subscribe_events();
        let result = client.call("Page.navigate", json!({ "url": url })).await?;
        if let Some(error) = result.get("errorText").and_then(Value::as_str) {
            anyhow::bail!("navigation failed: {error}");
        }
        if let Some(loader_id) = result.get("loaderId").and_then(Value::as_str) {
            wait_for_load_event(
                events,
                LoadState::DomContentLoaded,
                Duration::from_secs(15),
                Some(loader_id),
            )
            .await?;
        }
        Ok(())
    })
    .await
    .context("navigation_timeout: navigation did not complete within 15 seconds")?
}

pub(crate) async fn navigate_request(
    client: &CdpClient,
    request: &NavigateRequest,
) -> Result<BrowserToolOutput> {
    let timeout = wait_timeout(request.timeout_ms)?;
    tokio::time::timeout(timeout, navigate_request_inner(client, request, timeout))
        .await
        .context("navigation_timeout: navigation did not complete before timeoutMs")?
}

async fn navigate_request_inner(
    client: &CdpClient,
    request: &NavigateRequest,
    timeout: Duration,
) -> Result<BrowserToolOutput> {
    let mut events = client.subscribe_events();
    let (expected_loader, target_url) = match request.action {
        NavigationAction::Goto => {
            let url = request
                .url
                .as_deref()
                .context("navigate goto requires a URL")?;
            let result = client.call("Page.navigate", json!({ "url": url })).await?;
            if let Some(error) = result.get("errorText").and_then(Value::as_str) {
                anyhow::bail!("navigation failed: {error}");
            }
            (
                result
                    .get("loaderId")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                Some(url.to_string()),
            )
        }
        NavigationAction::Back => {
            anyhow::ensure!(request.url.is_none(), "navigate back does not accept a URL");
            (None, Some(navigate_history(client, /*offset*/ -1).await?))
        }
        NavigationAction::Forward => {
            anyhow::ensure!(
                request.url.is_none(),
                "navigate forward does not accept a URL"
            );
            (None, Some(navigate_history(client, /*offset*/ 1).await?))
        }
        NavigationAction::Reload => {
            anyhow::ensure!(
                request.url.is_none(),
                "navigate reload does not accept a URL"
            );
            client.call("Page.reload", json!({})).await?;
            (None, None)
        }
    };
    if expected_loader.is_none()
        && let Some(target_url) = target_url.as_deref()
    {
        wait_for_url_ready_state(client, target_url, request.wait_until).await?;
    } else {
        wait_for_load_event_receiver(
            &mut events,
            request.wait_until,
            timeout,
            expected_loader.as_deref(),
        )
        .await?;
    }
    let metadata = page_metadata(client).await?;
    Ok(BrowserToolOutput::Text(serde_json::to_string(&json!({
        "action": request.action,
        "url": metadata.url,
        "title": metadata.title,
        "loadState": request.wait_until,
    }))?))
}

pub(crate) async fn wait(
    client: &CdpClient,
    handles: &BrowserHandles,
    request: &WaitRequest,
) -> Result<BrowserToolOutput> {
    let timeout = match request {
        WaitRequest::Url { timeout_ms, .. }
        | WaitRequest::LoadState { timeout_ms, .. }
        | WaitRequest::Text { timeout_ms, .. }
        | WaitRequest::Node { timeout_ms, .. } => wait_timeout(*timeout_ms)?,
    };
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        anyhow::ensure!(
            !remaining.is_zero(),
            "navigation_timeout: wait condition timed out"
        );
        let matched = tokio::time::timeout(remaining, wait_condition_met(client, handles, request))
            .await
            .context("navigation_timeout: wait condition timed out")??;
        if matched {
            return Ok(BrowserToolOutput::Text(serde_json::to_string(&json!({
                "matched": true,
                "elapsedMs": timeout.saturating_sub(deadline.saturating_duration_since(Instant::now())).as_millis(),
            }))?));
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "navigation_timeout: wait condition timed out"
        );
        tokio::time::sleep(
            WAIT_POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
        )
        .await;
    }
}

async fn navigate_history(client: &CdpClient, offset: i64) -> Result<String> {
    let history = client.call("Page.getNavigationHistory", json!({})).await?;
    let current_index = history
        .get("currentIndex")
        .and_then(Value::as_i64)
        .context("navigation history omitted currentIndex")?;
    let target_index = current_index + offset;
    let entries = history
        .get("entries")
        .and_then(Value::as_array)
        .context("navigation history omitted entries")?;
    anyhow::ensure!(
        target_index >= 0 && usize::try_from(target_index).is_ok_and(|index| index < entries.len()),
        "navigation history has no entry in that direction"
    );
    let entry_id = entries[usize::try_from(target_index)?]
        .get("id")
        .and_then(Value::as_i64)
        .context("navigation history entry omitted id")?;
    let target_url = entries[usize::try_from(target_index)?]
        .get("url")
        .and_then(Value::as_str)
        .context("navigation history entry omitted URL")?
        .to_string();
    client
        .call(
            "Page.navigateToHistoryEntry",
            json!({ "entryId": entry_id }),
        )
        .await?;
    Ok(target_url)
}

async fn wait_for_url_ready_state(
    client: &CdpClient,
    target_url: &str,
    load_state: LoadState,
) -> Result<()> {
    loop {
        let state = client
            .evaluate("({ url: location.href, readyState: document.readyState })")
            .await?;
        if state.get("url").and_then(Value::as_str) == Some(target_url)
            && state
                .get("readyState")
                .and_then(Value::as_str)
                .is_some_and(|state| load_state.is_reached(state))
        {
            return Ok(());
        }
        tokio::time::sleep(WAIT_POLL_INTERVAL).await;
    }
}

async fn wait_for_load_event(
    mut events: broadcast::Receiver<CdpEvent>,
    load_state: LoadState,
    timeout: Duration,
    expected_loader: Option<&str>,
) -> Result<()> {
    wait_for_load_event_receiver(&mut events, load_state, timeout, expected_loader).await
}

async fn wait_for_load_event_receiver(
    events: &mut broadcast::Receiver<CdpEvent>,
    load_state: LoadState,
    wait_timeout: Duration,
    expected_loader: Option<&str>,
) -> Result<()> {
    tokio::time::timeout(wait_timeout, async {
        loop {
            match events.recv().await {
                Ok(CdpEvent::Message(message))
                    if navigation_event_matches(&message, load_state, expected_loader) =>
                {
                    return Ok(());
                }
                Ok(CdpEvent::Message(_)) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                Ok(CdpEvent::Disconnected(reason)) => anyhow::bail!(reason),
                Err(broadcast::error::RecvError::Closed) => {
                    anyhow::bail!("Carbonyl closed the DevTools connection")
                }
            }
        }
    })
    .await
    .context("navigation_timeout: navigation did not reach the requested load state")?
}

fn navigation_event_matches(
    message: &Value,
    load_state: LoadState,
    expected_loader: Option<&str>,
) -> bool {
    if message.get("method").and_then(Value::as_str) == Some("Page.lifecycleEvent") {
        let name = message.pointer("/params/name").and_then(Value::as_str);
        let reached = match load_state {
            LoadState::DomContentLoaded => matches!(name, Some("DOMContentLoaded" | "load")),
            LoadState::Load => name == Some("load"),
        };
        return reached
            && expected_loader.is_none_or(|loader| {
                message.pointer("/params/loaderId").and_then(Value::as_str) == Some(loader)
            });
    }
    expected_loader.is_none()
        && message.get("method").and_then(Value::as_str) == Some(load_state.event_method())
}

fn wait_timeout(timeout_ms: Option<u64>) -> Result<Duration> {
    let timeout = timeout_ms.map_or(DEFAULT_WAIT_TIMEOUT, Duration::from_millis);
    anyhow::ensure!(!timeout.is_zero(), "timeoutMs must be greater than zero");
    anyhow::ensure!(timeout <= MAX_WAIT_TIMEOUT, "timeoutMs cannot exceed 30000");
    Ok(timeout)
}

async fn wait_condition_met(
    client: &CdpClient,
    handles: &BrowserHandles,
    request: &WaitRequest,
) -> Result<bool> {
    match request {
        WaitRequest::Url {
            value, comparison, ..
        } => {
            anyhow::ensure!(value.len() <= 4 * 1024, "URL wait value exceeds 4 KiB");
            let current = client.evaluate("location.href").await?;
            let current = current.as_str().unwrap_or_default();
            Ok(match comparison {
                UrlMatch::Exact => current == value,
                UrlMatch::Contains => current.contains(value),
            })
        }
        WaitRequest::LoadState { state, .. } => {
            let ready_state = client.evaluate("document.readyState").await?;
            Ok(ready_state
                .as_str()
                .is_some_and(|value| state.is_reached(value)))
        }
        WaitRequest::Text { value, state, .. } => {
            anyhow::ensure!(value.len() <= 4 * 1024, "text wait value exceeds 4 KiB");
            let value = serde_json::to_string(value)?;
            let present = client
                .evaluate(&format!(
                    "Boolean(document.body?.innerText.includes({value}))"
                ))
                .await?
                .as_bool()
                .unwrap_or(/*default*/ false);
            Ok(match state {
                TextState::Present => present,
                TextState::Absent => !present,
            })
        }
        WaitRequest::Node { node_id, state, .. } => {
            let backend_node_id = handles.resolve(node_id)?;
            let attached = node_is_attached(client, backend_node_id).await?;
            let visible = attached && node_center(client, backend_node_id).await.is_ok();
            Ok(match state {
                NodeState::Visible => visible,
                NodeState::Hidden => attached && !visible,
                NodeState::Attached => attached,
                NodeState::Detached => !attached,
            })
        }
    }
}
