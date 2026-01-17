//! Formats rate limit and credit snapshots for the status card.
//!
//! This module converts protocol-level rate limit snapshots into display rows
//! with human-readable labels, reset times, and progress bars. It keeps the
//! formatting logic isolated from the status card renderer so the card can
//! focus on layout while this module handles data normalization and staleness
//! detection.
//!
//! The output is deterministic: rows are emitted in a fixed order (primary
//! window, secondary window, credits), and stale snapshots are tagged as such
//! based on the configured threshold. Reset timestamps are rendered in local
//! time using the capture time to decide whether to include the date.

use crate::chatwidget::get_limits_duration;
use crate::text_formatting::capitalize_first;

use super::helpers::format_reset_timestamp;
use chrono::DateTime;
use chrono::Duration as ChronoDuration;
use chrono::Local;
use chrono::Utc;
use codex_core::protocol::CreditsSnapshot as CoreCreditsSnapshot;
use codex_core::protocol::RateLimitSnapshot;
use codex_core::protocol::RateLimitWindow;

/// Number of segments rendered in the progress bar.
const STATUS_LIMIT_BAR_SEGMENTS: usize = 20;
/// Glyph used for filled progress bar segments.
const STATUS_LIMIT_BAR_FILLED: &str = "█";
/// Glyph used for empty progress bar segments.
const STATUS_LIMIT_BAR_EMPTY: &str = "░";

/// Display row for the status-card rate limit section.
#[derive(Debug, Clone)]
pub(crate) struct StatusRateLimitRow {
    /// Human-readable label, such as "5h limit" or "Credits".
    pub label: String,
    /// Renderable value, either a window bar or a free-form string.
    pub value: StatusRateLimitValue,
}

/// Renderable rate limit value shown in the status card.
#[derive(Debug, Clone)]
pub(crate) enum StatusRateLimitValue {
    /// Window-based limit with a progress bar and optional reset timestamp.
    Window {
        /// Percent of the window already used (0-100).
        percent_used: f64,
        /// Optional reset timestamp string.
        resets_at: Option<String>,
    },
    /// Plain text value, used for credits or non-window limits.
    Text(String),
}

/// Categorizes the availability of rate limit data for the status card.
#[derive(Debug, Clone)]
pub(crate) enum StatusRateLimitData {
    /// Fresh rate limit rows are available.
    Available(Vec<StatusRateLimitRow>),
    /// Rate limit rows are available but considered stale.
    Stale(Vec<StatusRateLimitRow>),
    /// No rate limit data is available.
    Missing,
}

/// Minutes after which a rate limit snapshot is considered stale.
///
/// Used when deciding whether to append the status-card staleness warning.
pub(crate) const RATE_LIMIT_STALE_THRESHOLD_MINUTES: i64 = 15;

/// Display-friendly version of a single rate limit window.
#[derive(Debug, Clone)]
pub(crate) struct RateLimitWindowDisplay {
    /// Percent of the window already used.
    pub used_percent: f64,
    /// Optional reset time string.
    pub resets_at: Option<String>,
    /// Optional window duration in minutes.
    pub window_minutes: Option<i64>,
}

impl RateLimitWindowDisplay {
    /// Converts a protocol window into a display snapshot.
    ///
    /// Reset timestamps are normalized into local time and formatted relative
    /// to the capture moment.
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

/// Display-ready snapshot of all rate limit data for the status card.
#[derive(Debug, Clone)]
pub(crate) struct RateLimitSnapshotDisplay {
    /// Timestamp when the snapshot was captured.
    pub captured_at: DateTime<Local>,
    /// Primary rate limit window, if present.
    pub primary: Option<RateLimitWindowDisplay>,
    /// Secondary rate limit window, if present.
    pub secondary: Option<RateLimitWindowDisplay>,
    /// Credits snapshot, if present.
    pub credits: Option<CreditsSnapshotDisplay>,
}

/// Display-ready credits snapshot for the status card.
#[derive(Debug, Clone)]
pub(crate) struct CreditsSnapshotDisplay {
    /// Whether the account tracks credits.
    pub has_credits: bool,
    /// Whether the account has unlimited credits.
    pub unlimited: bool,
    /// Optional string balance for finite credits.
    pub balance: Option<String>,
}

/// Convert a raw protocol snapshot into a display-ready snapshot.
///
/// This captures the local timestamp of the snapshot alongside any available
/// windows and credit details.
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
    /// Convert a core credits snapshot into its display wrapper.
    fn from(value: &CoreCreditsSnapshot) -> Self {
        Self {
            has_credits: value.has_credits,
            unlimited: value.unlimited,
            balance: value.balance.clone(),
        }
    }
}

/// Compose status-card rows from a rate limit snapshot and staleness threshold.
///
/// Rows are emitted in a fixed order and marked stale when the snapshot age
/// exceeds [`RATE_LIMIT_STALE_THRESHOLD_MINUTES`].
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

/// Render an ASCII progress bar for remaining rate limit capacity.
///
/// The `percent_remaining` value is clamped to 0-100 and rounded to the nearest
/// segment count.
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

/// Format the progress-bar summary text for a rate limit window.
///
/// The percentage is rounded to the nearest whole number for display.
pub(crate) fn format_status_limit_summary(percent_remaining: f64) -> String {
    format!("{percent_remaining:.0}% left")
}

/// Builds a single `StatusRateLimitRow` for credits when the snapshot indicates
/// that the account has credit tracking enabled.
///
/// When credits are unlimited we show that fact explicitly; otherwise we render
/// the rounded balance in credits. Accounts with credits = 0 skip this section
/// entirely.
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

/// Parse and normalize the credit balance string for display.
///
/// Integer and floating-point values are accepted when they are positive; any
/// empty or non-positive values are treated as missing.
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
