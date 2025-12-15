use crate::chatwidget::get_limits_duration;

use super::helpers::format_reset_timestamp;
use chrono::DateTime;
use chrono::Duration as ChronoDuration;
use chrono::Local;
use chrono::Utc;
use codex_core::protocol::CreditsSnapshot as CoreCreditsSnapshot;
use codex_core::protocol::RateLimitSnapshot;
use codex_core::protocol::RateLimitWindow;

const STATUS_LIMIT_BAR_SEGMENTS: usize = 20;
const STATUS_LIMIT_BAR_FILLED: &str = "█";
const STATUS_LIMIT_BAR_EMPTY: &str = "░";

#[derive(Debug, Clone)]
pub(crate) struct StatusRateLimitRow {
    pub label: String,
    pub value: StatusRateLimitValue,
}

#[derive(Debug, Clone)]
pub(crate) enum StatusRateLimitValue {
    Window {
        percent_used: f64,
        resets_at: Option<String>,
    },
    Text(String),
}

#[derive(Debug, Clone)]
pub(crate) enum StatusRateLimitData {
    Available(Vec<StatusRateLimitRow>),
    Stale(Vec<StatusRateLimitRow>),
    Missing,
}

pub(crate) const RATE_LIMIT_STALE_THRESHOLD_MINUTES: i64 = 15;

#[derive(Debug, Clone)]
pub(crate) struct RateLimitWindowDisplay {
    pub used_percent: f64,
    pub resets_at: Option<String>,
    pub window_minutes: Option<i64>,
}

impl RateLimitWindowDisplay {
    fn from_window(window: &RateLimitWindow, captured_at: DateTime<Local>) -> Self {
        let resets_at = window
            .resets_at
            .and_then(|seconds| DateTime::<Utc>::from_timestamp(seconds, 0))
            .map(|dt| dt.with_timezone(&Local))
            .map(|dt| format_reset_timestamp(dt, captured_at));

        Self {
            used_percent: window.used_percent,
            resets_at,
            window_minutes: window.window_minutes,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RateLimitSnapshotDisplay {
    pub captured_at: DateTime<Local>,
    pub primary: Option<RateLimitWindowDisplay>,
    pub secondary: Option<RateLimitWindowDisplay>,
    pub credits: Option<CreditsSnapshotDisplay>,
}

#[derive(Debug, Clone)]
pub(crate) struct CreditsSnapshotDisplay {
    pub has_credits: bool,
    pub unlimited: bool,
    pub balance: Option<String>,
}

pub(crate) fn rate_limit_snapshot_display(
    snapshot: &RateLimitSnapshot,
    captured_at: DateTime<Local>,
) -> RateLimitSnapshotDisplay {
    RateLimitSnapshotDisplay {
        captured_at,
        primary: snapshot
            .primary
            .as_ref()
            .map(|window| RateLimitWindowDisplay::from_window(window, captured_at)),
        secondary: snapshot
            .secondary
            .as_ref()
            .map(|window| RateLimitWindowDisplay::from_window(window, captured_at)),
        credits: snapshot.credits.as_ref().map(CreditsSnapshotDisplay::from),
    }
}

impl From<&CoreCreditsSnapshot> for CreditsSnapshotDisplay {
    fn from(value: &CoreCreditsSnapshot) -> Self {
        Self {
            has_credits: value.has_credits,
            unlimited: value.unlimited,
            balance: value.balance.clone(),
        }
    }
}

pub(crate) fn compose_rate_limit_data(
    snapshot: Option<&RateLimitSnapshotDisplay>,
    now: DateTime<Local>,
) -> StatusRateLimitData {
    match snapshot {
        Some(snapshot) => {
            let mut rows = Vec::with_capacity(3);

            if let Some(primary) = snapshot.primary.as_ref() {
                let label: String = primary
                    .window_minutes
                    .map(get_limits_duration)
                    .unwrap_or_else(|| "5h".to_string());
                let label = capitalize_first(&label);
                rows.push(StatusRateLimitRow {
                    label: format!("{label} limit"),
                    value: StatusRateLimitValue::Window {
                        percent_used: primary.used_percent,
                        resets_at: primary.resets_at.clone(),
                    },
                });
            }

            if let Some(secondary) = snapshot.secondary.as_ref() {
                let label: String = secondary
                    .window_minutes
                    .map(get_limits_duration)
                    .unwrap_or_else(|| "weekly".to_string());
                let label = capitalize_first(&label);
                rows.push(StatusRateLimitRow {
                    label: format!("{label} limit"),
                    value: StatusRateLimitValue::Window {
                        percent_used: secondary.used_percent,
                        resets_at: secondary.resets_at.clone(),
                    },
                });
            }

            if let Some(credits) = snapshot.credits.as_ref()
                && let Some(row) = credit_status_row(credits)
            {
                rows.push(row);
            }

            let is_stale = now.signed_duration_since(snapshot.captured_at)
                > ChronoDuration::minutes(RATE_LIMIT_STALE_THRESHOLD_MINUTES);

            if rows.is_empty() {
                StatusRateLimitData::Available(vec![])
            } else if is_stale {
                StatusRateLimitData::Stale(rows)
            } else {
                StatusRateLimitData::Available(rows)
            }
        }
        None => StatusRateLimitData::Missing,
    }
}

pub(crate) fn render_status_limit_progress_bar(percent_remaining: f64) -> String {
    let ratio = (percent_remaining / 100.0).clamp(0.0, 1.0);
    let filled = (ratio * STATUS_LIMIT_BAR_SEGMENTS as f64).round() as usize;
    let filled = filled.min(STATUS_LIMIT_BAR_SEGMENTS);
    let empty = STATUS_LIMIT_BAR_SEGMENTS.saturating_sub(filled);
    format!(
        "[{}{}]",
        STATUS_LIMIT_BAR_FILLED.repeat(filled),
        STATUS_LIMIT_BAR_EMPTY.repeat(empty)
    )
}

pub(crate) fn format_status_limit_summary(percent_remaining: f64) -> String {
    format!("{percent_remaining:.0}% left")
}

/// Builds a single `StatusRateLimitRow` for credits when the snapshot indicates
/// that the account has credit tracking enabled. When credits are unlimited we
/// show that fact explicitly; otherwise we render the rounded balance in
/// credits. Accounts with credits = 0 skip this section entirely.
fn credit_status_row(credits: &CreditsSnapshotDisplay) -> Option<StatusRateLimitRow> {
    if !credits.has_credits {
        return None;
    }
    if credits.unlimited {
        return Some(StatusRateLimitRow {
            label: "Credits".to_string(),
            value: StatusRateLimitValue::Text("Unlimited".to_string()),
        });
    }
    let balance = credits.balance.as_ref()?;
    let display_balance = format_credit_balance(balance)?;
    Some(StatusRateLimitRow {
        label: "Credits".to_string(),
        value: StatusRateLimitValue::Text(format!("{display_balance} credits")),
    })
}

fn format_credit_balance(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(int_value) = trimmed.parse::<i64>()
        && int_value > 0
    {
        return Some(int_value.to_string());
    }

    if let Ok(value) = trimmed.parse::<f64>()
        && value > 0.0
    {
        let rounded = value.round() as i64;
        return Some(rounded.to_string());
    }

    None
}

fn capitalize_first(label: &str) -> String {
    let mut chars = label.chars();
    match chars.next() {
        Some(first) => {
            let mut capitalized = first.to_uppercase().collect::<String>();
            capitalized.push_str(chars.as_str());
            capitalized
        }
        None => String::new(),
    }
}
