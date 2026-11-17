//! Projects the account values displayed by the built-in status line.

use super::ChatWidget;
use crate::status::RateLimitWindowDisplay;
use codex_protocol::protocol::RateLimitWindow;
use serde::Serialize;

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export_to = "statusline.d.ts"))]
pub(super) struct Account {
    rate_limits: Vec<RateLimit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    workspace: Option<Workspace>,
}

#[derive(Serialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "statusline.d.ts", rename = "AccountWorkspace")
)]
struct Workspace {
    headline: String,
}

#[derive(Serialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "statusline.d.ts", rename = "RateLimitSnapshot")
)]
struct RateLimit {
    limit_id: &'static str,
    limit_name: Option<String>,
    #[cfg_attr(test, ts(type = "RateLimitWindow | null"))]
    primary_limit: Option<RateLimitWindow>,
    #[cfg_attr(test, ts(type = "RateLimitWindow | null"))]
    secondary_limit: Option<RateLimitWindow>,
}

impl From<&RateLimitWindowDisplay> for RateLimitWindow {
    fn from(window: &RateLimitWindowDisplay) -> Self {
        Self {
            used_percent: window.used_percent,
            window_minutes: window.window_minutes,
            resets_at: window.resets_at_unix,
        }
    }
}

impl From<&ChatWidget> for Account {
    fn from(chat: &ChatWidget) -> Self {
        let codex = chat
            .rate_limit_snapshots_by_limit_id
            .get("codex")
            .filter(|sample| sample.primary.is_some() || sample.secondary.is_some());
        Self {
            rate_limits: codex
                .into_iter()
                .map(|sample| RateLimit {
                    limit_id: "codex",
                    limit_name: (!sample.limit_name.is_empty()).then(|| sample.limit_name.clone()),
                    primary_limit: sample.primary.as_ref().map(Into::into),
                    secondary_limit: sample.secondary.as_ref().map(Into::into),
                })
                .collect(),
            workspace: chat
                .status_line_workspace_headline
                .clone()
                .map(|headline| Workspace { headline }),
        }
    }
}
