use crate::chatwidget::get_limits_duration;

use super::helpers::format_reset_timestamp;
use chrono::DateTime;
use chrono::Local;
use chrono::Utc;
use codex_core::protocol::RateLimitSnapshot;
use codex_core::protocol::RateLimitWindow;

const STATUS_LIMIT_BAR_SEGMENTS: usize = 20;
const STATUS_LIMIT_BAR_FILLED: &str = "█";
const STATUS_LIMIT_BAR_EMPTY: &str = "░";

#[derive(Debug, Clone)]
pub(crate) struct StatusRateLimitRow {
    pub label: String,
    pub percent_used: f64,
    pub resets_at: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) enum StatusRateLimitData {
    Available(Vec<StatusRateLimitRow>),
    Missing,
}

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
    pub primary: Option<RateLimitWindowDisplay>,
    pub secondary: Option<RateLimitWindowDisplay>,
}

pub(crate) fn rate_limit_snapshot_display(
    snapshot: &RateLimitSnapshot,
    captured_at: DateTime<Local>,
) -> RateLimitSnapshotDisplay {
    RateLimitSnapshotDisplay {
        primary: snapshot
            .primary
            .as_ref()
            .map(|window| RateLimitWindowDisplay::from_window(window, captured_at)),
        secondary: snapshot
            .secondary
            .as_ref()
            .map(|window| RateLimitWindowDisplay::from_window(window, captured_at)),
    }
}

pub(crate) fn compose_rate_limit_data(
    snapshot: Option<&RateLimitSnapshotDisplay>,
) -> StatusRateLimitData {
    match snapshot {
        Some(snapshot) => {
            let mut rows = Vec::with_capacity(2);

            if let Some(primary) = snapshot.primary.as_ref() {
                let label: String = primary
                    .window_minutes
                    .map(get_limits_duration)
                    .unwrap_or_else(|| "5h".to_string());
                let label = capitalize_first(&label);
                rows.push(StatusRateLimitRow {
                    label: format!("{label} limit"),
                    percent_used: primary.used_percent,
                    resets_at: primary.resets_at.clone(),
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
                    percent_used: secondary.used_percent,
                    resets_at: secondary.resets_at.clone(),
                });
            }

            if rows.is_empty() {
                StatusRateLimitData::Available(vec![])
            } else {
                StatusRateLimitData::Available(rows)
            }
        }
        None => StatusRateLimitData::Missing,
    }
}

pub(crate) fn render_status_limit_progress_bar(percent_used: f64) -> String {
    let ratio = (percent_used / 100.0).clamp(0.0, 1.0);
    let filled = (ratio * STATUS_LIMIT_BAR_SEGMENTS as f64).round() as usize;
    let filled = filled.min(STATUS_LIMIT_BAR_SEGMENTS);
    let empty = STATUS_LIMIT_BAR_SEGMENTS.saturating_sub(filled);
    format!(
        "[{}{}]",
        STATUS_LIMIT_BAR_FILLED.repeat(filled),
        STATUS_LIMIT_BAR_EMPTY.repeat(empty)
    )
}

pub(crate) fn format_status_limit_summary(percent_used: f64) -> String {
    format!("{percent_used:.0}% used")
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
