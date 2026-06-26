use std::sync::Arc;

use tokio::task::JoinHandle;
use url::Url;

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
                    let is_main_frame = message.get("method").and_then(serde_json::Value::as_str)
                        == Some("Page.frameNavigated")
                        && message.pointer("/params/frame/parentId").is_none();
                    let Some(url) = is_main_frame
                        .then(|| message.pointer("/params/frame/url"))
                        .flatten()
                        .and_then(serde_json::Value::as_str)
                    else {
                        continue;
                    };
                    if is_allowed_observed_url(url) {
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
