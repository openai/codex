//! Account quota state and composer notice, independent of one-time warning history.
//! Rolling updates retain metadata; fresh account reads replace windows and confirm recovery.
//! Optional details yield to the quota summary when the composer gap is narrow.

use super::ChatWidget;
use super::rate_limits::RateLimitSnapshotSource;
use super::rate_limits::is_approximate_window;
use super::rate_limits::limit_label_for_window;
use crate::footer_hint::first_fitting_line;
use chrono::DateTime;
use chrono::Local;
use codex_app_server_protocol::CreditsSnapshot;
use codex_app_server_protocol::RateLimitSnapshot;
use codex_app_server_protocol::RateLimitWindow;
use codex_protocol::account::PlanType;
use ratatui::text::Line;

#[derive(Default)]
pub(super) struct UsageNoticeState {
    primary: Option<RateLimitWindow>,
    secondary: Option<RateLimitWindow>,
    plan_type: Option<PlanType>,
    credits: Option<CreditsSnapshot>,
    latest_read_id: u64,
    stale_through: Option<u64>,
    applying_stale_read: bool,
}

impl UsageNoticeState {
    fn line(&self, width: u16, now: DateTime<Local>) -> Option<Line<'static>> {
        let (window, is_secondary) = self.current()?;
        let label = limit_label_for_window(window.window_duration_mins, is_secondary);
        // Percentages are rounded by the protocol; 100% alone does not prove a hard stop.
        let remaining = if window.used_percent >= 100 {
            "<1%".to_string()
        } else {
            format!("{}%", 100 - window.used_percent)
        };
        let emphasis = if window.used_percent >= 95 {
            "only "
        } else {
            ""
        };
        let summary = format!("⚠ {label} limit: {emphasis}{remaining} left");
        let reset = window
            .resets_at
            .and_then(|timestamp| DateTime::from_timestamp(timestamp, /*nsecs*/ 0))
            .filter(|reset| *reset > now)
            .map(|reset| {
                format!(
                    " · resets at {}",
                    crate::status::format_reset_timestamp(reset.with_timezone(&Local), now)
                )
            })
            .unwrap_or_default();
        let mut style = crate::style::warning_notice_style();
        if window.used_percent >= 90 {
            style = style.bold();
        }
        let line = first_fitting_line(
            [
                format!("{summary}{reset} · /status"),
                format!("{summary} · /status"),
                summary,
                format!("⚠ {label} {remaining} left"),
            ]
            .map(Line::from),
            width,
        );
        (!line.spans.is_empty()).then_some(line.style(style))
    }

    pub(super) fn update(
        &mut self,
        snapshot: &RateLimitSnapshot,
        source: RateLimitSnapshotSource,
        plan_type: Option<PlanType>,
    ) -> bool {
        if matches!(source, RateLimitSnapshotSource::AccountUsage) && self.applying_stale_read {
            return false;
        }
        let previous = self.current();
        let rolling = matches!(source, RateLimitSnapshotSource::RollingUpdate);
        if rolling {
            self.stale_through = Some(self.latest_read_id);
        }
        self.plan_type = snapshot.plan_type.or(self.plan_type).or(plan_type);
        for (current, incoming) in [
            (&mut self.primary, &snapshot.primary),
            (&mut self.secondary, &snapshot.secondary),
        ] {
            if !rolling {
                *current = incoming.clone();
                continue;
            }
            let Some(incoming) = incoming else {
                continue;
            };
            let mut window = incoming.clone();
            if let Some(previous) = current {
                window.window_duration_mins = window
                    .window_duration_mins
                    .or(previous.window_duration_mins);
                window.resets_at = window.resets_at.or(previous.resets_at);
                let threshold = warning_threshold(self.plan_type, window.window_duration_mins);
                if previous.used_percent >= threshold && window.used_percent < threshold {
                    continue;
                }
            }
            *current = Some(window);
        }
        if !rolling || snapshot.credits.is_some() {
            self.credits = snapshot.credits.clone();
        }
        self.current() != previous
    }

    /// Select the lowest remaining capacity and whether it is the secondary window.
    pub(super) fn current(&self) -> Option<(RateLimitWindow, bool)> {
        if self
            .credits
            .as_ref()
            .is_some_and(|credits| credits.unlimited || credits.has_credits)
        {
            return None;
        }
        // max_by_key chooses the last equal entry, giving the primary window priority.
        [(&self.secondary, true), (&self.primary, false)]
            .into_iter()
            .filter_map(|(window, secondary)| Some((window.as_ref()?, secondary)))
            .filter(|(window, _)| {
                window.used_percent
                    >= warning_threshold(self.plan_type, window.window_duration_mins)
            })
            .max_by_key(|(window, _)| window.used_percent)
            .map(|(window, secondary)| (window.clone(), secondary))
    }
}

pub(super) fn warning_threshold(plan_type: Option<PlanType>, window_minutes: Option<i64>) -> i32 {
    if matches!(plan_type, Some(PlanType::Plus | PlanType::Team))
        && window_minutes.is_some_and(|minutes| {
            is_approximate_window(minutes, /*expected_minutes*/ 5 * 60)
        })
    {
        50
    } else {
        75
    }
}

impl ChatWidget {
    pub(crate) fn usage_notice(&self, width: u16) -> Option<Line<'static>> {
        if self.has_applicable_backend_banner()
            || self.codex_rate_limit_reached_type.is_some()
            || self.codex_spend_control_reached == Some(true)
        {
            return None;
        }
        self.usage_notice_state.line(width, Local::now())
    }

    pub(crate) fn start_usage_notice_read(&mut self, request_id: u64) {
        self.usage_notice_state.latest_read_id = request_id;
    }

    pub(crate) fn apply_usage_notice_read(&mut self, request_id: u64) {
        // A newer failed read does not invalidate an older successful one. Only rolling
        // updates received since that read began make it too old to confirm recovery.
        self.usage_notice_state.applying_stale_read = self
            .usage_notice_state
            .stale_through
            .is_some_and(|id| request_id <= id);
    }
}

#[cfg(test)]
#[path = "usage_notice_tests.rs"]
mod tests;
