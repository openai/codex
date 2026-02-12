//! Rate-limit and credits display shaping for status surfaces.
//!
//! This module maps `RateLimitSnapshot` protocol payloads into display-oriented rows that the TUI
//! can render in `/status` and status-line contexts without duplicating formatting logic.
//!
//! The key contract is that time-sensitive values are interpreted relative to a caller-provided
//! capture timestamp so stale detection and reset labels remain coherent for a given draw cycle.
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

const STATUS_LIMIT_BAR_SEGMENTS: usize = 20;
const STATUS_LIMIT_BAR_FILLED: &str = "█";
const STATUS_LIMIT_BAR_EMPTY: &str = "░";

#[derive(Debug, Clone)]
pub(crate) struct StatusRateLimitRow {
    /// Human-readable row label, such as `"5h limit"` or `"Credits"`.
    pub label: String,
    /// Value payload for the row.
    pub value: StatusRateLimitValue,
}

/// Display value variants for a single rate-limit row.
#[derive(Debug, Clone)]
pub(crate) enum StatusRateLimitValue {
    /// Percent-based usage window with optional reset timestamp text.
    Window {
        /// Percent of the window that has been consumed.
        percent_used: f64,
        /// Localized reset string, or `None` when unknown.
        resets_at: Option<String>,
    },
    /// Plain text value used for non-window rows.
    Text(String),
}

/// Availability state for rate-limit data shown in status output.
#[derive(Debug, Clone)]
pub(crate) enum StatusRateLimitData {
    /// Snapshot data is recent enough for normal rendering.
    Available(Vec<StatusRateLimitRow>),
    /// Snapshot data exists but is older than the staleness threshold.
    Stale(Vec<StatusRateLimitRow>),
    /// No snapshot data is currently available.
    Missing,
}

/// Maximum age before a snapshot is considered stale in status output.
pub(crate) const RATE_LIMIT_STALE_THRESHOLD_MINUTES: i64 = 15;

/// Display-friendly representation of one usage window from a snapshot.
#[derive(Debug, Clone)]
pub(crate) struct RateLimitWindowDisplay {
    /// Percent used for the window.
    pub used_percent: f64,
    /// Human-readable local reset time.
    pub resets_at: Option<String>,
    /// Window length in minutes when provided by the server.
    pub window_minutes: Option<i64>,
}

impl RateLimitWindowDisplay {
    fn from_window(window: &RateLimitWindow, captured_at: DateTime<Local>) -> Self {
        let resets_at_utc = window
            .resets_at
            .and_then(|seconds| DateTime::<Utc>::from_timestamp(seconds, 0))
            .map(|dt| dt.with_timezone(&Local));
        let resets_at = resets_at_utc.map(|dt| format_reset_timestamp(dt, captured_at));

        Self {
            used_percent: window.used_percent,
            resets_at,
            window_minutes: window.window_minutes,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RateLimitSnapshotDisplay {
    /// Canonical limit identifier (for example: `codex` or `codex_other`).
    pub limit_name: String,
    /// Local timestamp representing when this display snapshot was captured.
    pub captured_at: DateTime<Local>,
    /// Primary usage window (typically short duration).
    pub primary: Option<RateLimitWindowDisplay>,
    /// Secondary usage window (typically weekly).
    pub secondary: Option<RateLimitWindowDisplay>,
    /// Optional credits metadata when available.
    pub credits: Option<CreditsSnapshotDisplay>,
}

/// Display-ready credits state extracted from protocol snapshots.
#[derive(Debug, Clone)]
pub(crate) struct CreditsSnapshotDisplay {
    /// Whether credits tracking is enabled for the account.
    pub has_credits: bool,
    /// Whether the account has unlimited credits.
    pub unlimited: bool,
    /// Raw balance text as provided by the backend.
    pub balance: Option<String>,
}

/// Converts a protocol snapshot into UI-friendly display data.
///
/// Pass the timestamp from the same observation point as `snapshot`; supplying a significantly
/// older or newer `captured_at` can produce misleading reset labels and stale classification.
#[cfg(test)]
pub(crate) fn rate_limit_snapshot_display(
    snapshot: &RateLimitSnapshot,
    captured_at: DateTime<Local>,
) -> RateLimitSnapshotDisplay {
    rate_limit_snapshot_display_for_limit(snapshot, "codex".to_string(), captured_at)
}

pub(crate) fn rate_limit_snapshot_display_for_limit(
    snapshot: &RateLimitSnapshot,
    limit_name: String,
    captured_at: DateTime<Local>,
) -> RateLimitSnapshotDisplay {
    RateLimitSnapshotDisplay {
        limit_name,
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

/// Builds display rows from a snapshot and marks stale data by capture age.
///
/// Callers should pass `Local::now()` for `now` at render time; using a cached timestamp can make
/// fresh data appear stale or prevent stale warnings from appearing.
pub(crate) fn compose_rate_limit_data(
    snapshot: Option<&RateLimitSnapshotDisplay>,
    now: DateTime<Local>,
) -> StatusRateLimitData {
    match snapshot {
        Some(snapshot) => compose_rate_limit_data_many(std::slice::from_ref(snapshot), now),
        None => StatusRateLimitData::Missing,
    }
}

pub(crate) fn compose_rate_limit_data_many(
    snapshots: &[RateLimitSnapshotDisplay],
    now: DateTime<Local>,
) -> StatusRateLimitData {
    if snapshots.is_empty() {
        return StatusRateLimitData::Missing;
    }

    let mut rows = Vec::with_capacity(snapshots.len().saturating_mul(3));
    let mut stale = false;

    for snapshot in snapshots {
        stale |= now.signed_duration_since(snapshot.captured_at)
            > ChronoDuration::minutes(RATE_LIMIT_STALE_THRESHOLD_MINUTES);

        let limit_bucket_label = snapshot.limit_name.clone();
        let show_limit_prefix = !limit_bucket_label.eq_ignore_ascii_case("codex");
        let primary_label = snapshot
            .primary
            .as_ref()
            .map(|window| {
                window
                    .window_minutes
                    .map(get_limits_duration)
                    .unwrap_or_else(|| "5h".to_string())
            })
            .map(|label| capitalize_first(&label));
        let secondary_label = snapshot
            .secondary
            .as_ref()
            .map(|window| {
                window
                    .window_minutes
                    .map(get_limits_duration)
                    .unwrap_or_else(|| "weekly".to_string())
            })
            .map(|label| capitalize_first(&label));
        let window_count =
            usize::from(snapshot.primary.is_some()) + usize::from(snapshot.secondary.is_some());
        let combine_non_codex_single_limit = show_limit_prefix && window_count == 1;

        if show_limit_prefix && !combine_non_codex_single_limit {
            rows.push(StatusRateLimitRow {
                label: format!("{limit_bucket_label} limit"),
                value: StatusRateLimitValue::Text(String::new()),
            });
        }

        if let Some(primary) = snapshot.primary.as_ref() {
            let label = if combine_non_codex_single_limit {
                format!(
                    "{} {} limit",
                    limit_bucket_label,
                    primary_label.clone().unwrap_or_else(|| "5h".to_string())
                )
            } else {
                format!(
                    "{} limit",
                    primary_label.clone().unwrap_or_else(|| "5h".to_string())
                )
            };
            rows.push(StatusRateLimitRow {
                label,
                value: StatusRateLimitValue::Window {
                    percent_used: primary.used_percent,
                    resets_at: primary.resets_at.clone(),
                },
            });
        }

        if let Some(secondary) = snapshot.secondary.as_ref() {
            let label = if combine_non_codex_single_limit {
                format!(
                    "{} {} limit",
                    limit_bucket_label,
                    secondary_label
                        .clone()
                        .unwrap_or_else(|| "weekly".to_string())
                )
            } else {
                format!(
                    "{} limit",
                    secondary_label
                        .clone()
                        .unwrap_or_else(|| "weekly".to_string())
                )
            };
            rows.push(StatusRateLimitRow {
                label,
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
    }

    if rows.is_empty() {
        StatusRateLimitData::Available(vec![])
    } else if stale {
        StatusRateLimitData::Stale(rows)
    } else {
        StatusRateLimitData::Available(rows)
    }
}

/// Renders a fixed-width progress bar from remaining percentage.
///
/// This function expects a remaining value in the `0..=100` range and clamps out-of-range input.
/// Passing a used percentage by mistake will invert the bar and mislead users.
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

/// Formats a compact textual summary from remaining percentage.
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

#[cfg(test)]
mod tests {
    use super::CreditsSnapshotDisplay;
    use super::RateLimitSnapshotDisplay;
    use super::RateLimitWindowDisplay;
    use super::StatusRateLimitData;
    use super::compose_rate_limit_data_many;
    use chrono::Local;
    use pretty_assertions::assert_eq;

    fn window(used_percent: f64) -> RateLimitWindowDisplay {
        RateLimitWindowDisplay {
            used_percent,
            resets_at: Some("soon".to_string()),
            window_minutes: Some(300),
        }
    }

    #[test]
    fn non_codex_single_limit_renders_combined_row() {
        let now = Local::now();
        let codex = RateLimitSnapshotDisplay {
            limit_name: "codex".to_string(),
            captured_at: now,
            primary: Some(window(10.0)),
            secondary: None,
            credits: Some(CreditsSnapshotDisplay {
                has_credits: true,
                unlimited: false,
                balance: Some("25".to_string()),
            }),
        };
        let other = RateLimitSnapshotDisplay {
            limit_name: "codex-other".to_string(),
            captured_at: now,
            primary: Some(window(20.0)),
            secondary: None,
            credits: Some(CreditsSnapshotDisplay {
                has_credits: true,
                unlimited: false,
                balance: Some("99".to_string()),
            }),
        };

        let rows = match compose_rate_limit_data_many(&[codex, other], now) {
            StatusRateLimitData::Available(rows) => rows,
            other => panic!("unexpected status: {other:?}"),
        };

        let labels: Vec<String> = rows.iter().map(|row| row.label.clone()).collect();
        assert_eq!(
            labels,
            vec![
                "5h limit".to_string(),
                "Credits".to_string(),
                "codex-other 5h limit".to_string(),
                "Credits".to_string(),
            ]
        );
        assert_eq!(rows.iter().filter(|row| row.label == "Credits").count(), 2);
    }

    #[test]
    fn non_codex_multi_limit_keeps_group_row() {
        let now = Local::now();
        let other = RateLimitSnapshotDisplay {
            limit_name: "codex-other".to_string(),
            captured_at: now,
            primary: Some(RateLimitWindowDisplay {
                used_percent: 20.0,
                resets_at: Some("soon".to_string()),
                window_minutes: Some(60),
            }),
            secondary: Some(RateLimitWindowDisplay {
                used_percent: 40.0,
                resets_at: Some("later".to_string()),
                window_minutes: None,
            }),
            credits: None,
        };

        let rows = match compose_rate_limit_data_many(&[other], now) {
            StatusRateLimitData::Available(rows) => rows,
            other => panic!("unexpected status: {other:?}"),
        };
        let labels: Vec<String> = rows.iter().map(|row| row.label.clone()).collect();
        assert_eq!(
            labels,
            vec![
                "codex-other limit".to_string(),
                "1h limit".to_string(),
                "Weekly limit".to_string(),
            ]
        );
    }
}
