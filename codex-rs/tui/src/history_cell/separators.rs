//! Subtle completion timestamps, elapsed durations, and runtime metrics for transcript history.

use super::*;
use crate::clock_format::ClockFormat;
use chrono::DateTime;
use chrono::Datelike;
use chrono::Local;
use chrono::NaiveDate;

/// Completion metadata shown after the assistant's final response.
///
/// The timestamp records when the turn actually finished, including when restored from history.
/// Times use the host's clock preference; other local days include the date and other years the year.
/// Durations are shown only above sixty seconds; shorter turns still show their timestamp.
/// Absent metadata occupies no transcript rows.
/// The display date is fixed at construction so crossing midnight cannot invalidate cached heights;
/// restoring the conversation constructs new cells and refreshes whether the timestamp needs a date.
#[derive(Debug)]
pub struct FinalMessageSeparator {
    elapsed_seconds: Option<u64>,
    runtime_metrics: Option<RuntimeMetricsSummary>,
    completed_at: Option<DateTime<Local>>,
    display_date: NaiveDate,
    clock_format: ClockFormat,
}
impl FinalMessageSeparator {
    /// Creates completion metadata using the protocol turn duration when available.
    pub(crate) fn new(
        elapsed_seconds: Option<u64>,
        runtime_metrics: Option<RuntimeMetricsSummary>,
    ) -> Self {
        Self {
            elapsed_seconds,
            runtime_metrics,
            completed_at: None,
            display_date: Local::now().date_naive(),
            clock_format: ClockFormat::TwelveHour,
        }
    }

    pub(crate) fn with_completed_at(
        mut self,
        completed_at: DateTime<Local>,
        clock_format: ClockFormat,
    ) -> Self {
        self.completed_at = Some(completed_at);
        self.clock_format = clock_format;
        self
    }

    pub(crate) fn with_runtime_metrics(
        mut self,
        runtime_metrics: Option<RuntimeMetricsSummary>,
    ) -> Self {
        self.runtime_metrics = runtime_metrics;
        self
    }

    fn label(&self, today: NaiveDate) -> Option<String> {
        let mut label_parts = Vec::new();
        if let Some(elapsed_seconds) = self.elapsed_seconds.filter(|seconds| *seconds > 60) {
            let hours = elapsed_seconds / 3_600;
            let minutes = (elapsed_seconds % 3_600) / 60;
            let seconds = elapsed_seconds % 60;
            let elapsed = if hours > 0 {
                format!("{hours}h {minutes}m {seconds}s")
            } else if minutes > 0 {
                format!("{minutes}m {seconds}s")
            } else {
                format!("{seconds}s")
            };
            label_parts.push(format!("Worked for {elapsed}"));
        }
        if let Some(completed_at) = self.completed_at {
            let date_format = if completed_at.date_naive() == today {
                ""
            } else if completed_at.year() == today.year() {
                "%b %-d at "
            } else {
                "%b %-d, %Y at "
            };
            label_parts.push(format!(
                "{}{}",
                completed_at.format(date_format),
                completed_at.format(self.clock_format.time_format()),
            ));
        }
        if let Some(metrics_label) = self.runtime_metrics.and_then(runtime_metrics_label) {
            label_parts.push(metrics_label);
        }
        (!label_parts.is_empty()).then(|| label_parts.join(" · "))
    }
}
impl HistoryCell for FinalMessageSeparator {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        if width == 0 {
            return Vec::new();
        }
        self.label(self.display_date)
            .map(|label| {
                let indent = if width > 2 { "  " } else { "" };
                let options = textwrap::Options::new(usize::from(width))
                    .initial_indent(indent)
                    .subsequent_indent(indent);
                textwrap::wrap(&label, options)
                    .into_iter()
                    .map(|line| Line::from(line.into_owned()).dim())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        self.label(self.display_date)
            .map(|label| vec![Line::from(label)])
            .unwrap_or_default()
    }
}

#[cfg(test)]
#[path = "separators_tests.rs"]
mod tests;

pub(crate) fn runtime_metrics_label(summary: RuntimeMetricsSummary) -> Option<String> {
    let mut parts = Vec::new();
    if summary.tool_calls.count > 0 {
        let duration = format_duration_ms(summary.tool_calls.duration_ms);
        let calls = pluralize(summary.tool_calls.count, "call", "calls");
        parts.push(format!(
            "Local tools: {} {calls} ({duration})",
            summary.tool_calls.count
        ));
    }
    if summary.api_calls.count > 0 {
        let duration = format_duration_ms(summary.api_calls.duration_ms);
        let calls = pluralize(summary.api_calls.count, "call", "calls");
        parts.push(format!(
            "Inference: {} {calls} ({duration})",
            summary.api_calls.count
        ));
    }
    if summary.websocket_calls.count > 0 {
        let duration = format_duration_ms(summary.websocket_calls.duration_ms);
        parts.push(format!(
            "WebSocket: {} events send ({duration})",
            summary.websocket_calls.count
        ));
    }
    if summary.streaming_events.count > 0 {
        let duration = format_duration_ms(summary.streaming_events.duration_ms);
        let stream_label = pluralize(summary.streaming_events.count, "Stream", "Streams");
        let events = pluralize(summary.streaming_events.count, "event", "events");
        parts.push(format!(
            "{stream_label}: {} {events} ({duration})",
            summary.streaming_events.count
        ));
    }
    if summary.websocket_events.count > 0 {
        let duration = format_duration_ms(summary.websocket_events.duration_ms);
        parts.push(format!(
            "{} events received ({duration})",
            summary.websocket_events.count
        ));
    }
    if summary.responses_api_overhead_ms > 0 {
        let duration = format_duration_ms(summary.responses_api_overhead_ms);
        parts.push(format!("Responses API overhead: {duration}"));
    }
    if summary.responses_api_inference_time_ms > 0 {
        let duration = format_duration_ms(summary.responses_api_inference_time_ms);
        parts.push(format!("Responses API inference: {duration}"));
    }
    if summary.responses_api_engine_iapi_ttft_ms > 0
        || summary.responses_api_engine_service_ttft_ms > 0
    {
        let mut ttft_parts = Vec::new();
        if summary.responses_api_engine_iapi_ttft_ms > 0 {
            let duration = format_duration_ms(summary.responses_api_engine_iapi_ttft_ms);
            ttft_parts.push(format!("{duration} (iapi)"));
        }
        if summary.responses_api_engine_service_ttft_ms > 0 {
            let duration = format_duration_ms(summary.responses_api_engine_service_ttft_ms);
            ttft_parts.push(format!("{duration} (service)"));
        }
        parts.push(format!("TTFT: {}", ttft_parts.join(" ")));
    }
    if summary.responses_api_engine_iapi_tbt_ms > 0.0
        || summary.responses_api_engine_service_tbt_ms > 0.0
    {
        let mut tbt_parts = Vec::new();
        if summary.responses_api_engine_iapi_tbt_ms > 0.0 {
            let duration =
                format_duration_ms(summary.responses_api_engine_iapi_tbt_ms.round() as u64);
            tbt_parts.push(format!("{duration} (iapi)"));
        }
        if summary.responses_api_engine_service_tbt_ms > 0.0 {
            let duration =
                format_duration_ms(summary.responses_api_engine_service_tbt_ms.round() as u64);
            tbt_parts.push(format!("{duration} (service)"));
        }
        parts.push(format!("TBT: {}", tbt_parts.join(" ")));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" • "))
    }
}

fn format_duration_ms(duration_ms: u64) -> String {
    if duration_ms >= 1_000 {
        let seconds = duration_ms as f64 / 1_000.0;
        format!("{seconds:.1}s")
    } else {
        format!("{duration_ms}ms")
    }
}

fn pluralize(count: u64, singular: &'static str, plural: &'static str) -> &'static str {
    if count == 1 { singular } else { plural }
}
