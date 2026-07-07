use std::sync::Arc;

use tokio::task::JoinHandle;
use url::Url;

use crate::actions;
use crate::cdp::CdpClient;
use crate::cdp::CdpEvent;
use crate::session::Inner;

pub(crate) fn is_allowed_browser_url(raw_url: &str) -> bool {
    matches!(
        Url::parse(raw_url).ok().as_ref().map(Url::scheme),
        Some("http" | "https")
    )
}

pub(crate) fn is_allowed_observed_url(raw_url: &str) -> bool {
    raw_url == "about:blank" || is_allowed_browser_url(raw_url)
}

pub(crate) fn spawn_navigation_policy_task(inner: Arc<Inner>, cdp: CdpClient) -> JoinHandle<()> {
    let mut events = cdp.subscribe_events();
    tokio::spawn(async move {
        loop {
            match events.recv().await {
                Ok(CdpEvent::Message(message)) => {
                    if is_page_load_event(&message) {
                        refresh_observed_page_metadata(&inner, &cdp).await;
                        continue;
                    }
                    let Some(url) = observed_main_frame_url(&message) else {
                        continue;
                    };
                    if is_allowed_observed_url(url) {
                        inner.update_view(|view| view.url = Some(url.to_string()));
                        continue;
                    }
                    tracing::warn!("blocked disallowed terminal-browser main-frame navigation");
                    let _ = cdp.call("Page.stopLoading", serde_json::json!({})).await;
                    let _ = cdp
                        .call("Page.navigate", serde_json::json!({ "url": "about:blank" }))
                        .await;
                    inner.update_view(|view| {
                        view.url = Some("about:blank".to_string());
                        view.title = Some("Blocked navigation".to_string());
                    });
                }
                Ok(CdpEvent::Disconnected(_))
                | Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            }
        }
    })
}

fn observed_main_frame_url(message: &serde_json::Value) -> Option<&str> {
    (message.get("method").and_then(serde_json::Value::as_str) == Some("Page.frameNavigated")
        && message.pointer("/params/frame/parentId").is_none())
    .then(|| message.pointer("/params/frame/url"))
    .flatten()
    .and_then(serde_json::Value::as_str)
}

fn is_page_load_event(message: &serde_json::Value) -> bool {
    message.get("method").and_then(serde_json::Value::as_str) == Some("Page.loadEventFired")
}

async fn refresh_observed_page_metadata(inner: &Inner, cdp: &CdpClient) {
    let Ok(metadata) = actions::page_metadata(cdp).await else {
        return;
    };
    if metadata.url.as_deref().is_none_or(is_allowed_observed_url) {
        inner.update_view(|view| {
            view.url = metadata.url;
            view.title = metadata.title;
        });
    }
}

#[cfg(test)]
#[path = "url_policy_tests.rs"]
mod tests;
