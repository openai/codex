//! Demand-driven dashboard usage. Only the selected task is fetched on a one-minute
//! refresh cadence, including after failures. Account changes and reconnects invalidate
//! pending billing results and disabled capability state.

use super::App;
use super::agents_overview::AGENTS_OVERVIEW_VIEW_ID;
use super::background_requests::THREAD_USAGE_FETCH_TIMEOUT;
use super::background_requests::fetch_thread_usage;
use crate::app_event::AppEvent;
use crate::app_server_session::AppServerSession;
use crate::chatwidget::ThreadUsageOutcome;
use crate::status::format_credit_micros;
use crate::status::format_estimated_usd_micros;
use crate::status::format_tokens_compact;
use crate::tui::FrameRequester;
use codex_app_server_protocol::ThreadUsage;
use codex_app_server_protocol::ThreadUsageBreakdownGroup;
use codex_app_server_protocol::TokenUsageBreakdown;
use codex_protocol::ThreadId;
use codex_protocol::account::PlanType;
use ratatui::style::Stylize;
use ratatui::text::Line;
use std::time::Duration;
use std::time::Instant;
use uuid::Uuid;

const REFRESH_INTERVAL: Duration = Duration::from_secs(/*secs*/ 60);

#[derive(Default)]
pub(super) struct AgentsOverviewUsage {
    pub(super) tokens: Option<TokenUsageBreakdown>,
    pub(super) estimate: Option<ThreadUsage>,
    fetched_at: Option<Instant>,
}

impl App {
    pub(super) fn refresh_agents_overview_usage(
        &mut self,
        app_server: &AppServerSession,
        frame_requester: FrameRequester,
    ) {
        if self.reconnect.offline
            || self.agents_overview.usage_disabled
            || self.agents_overview.pending_usage.is_some()
            || !self.chat_widget.has_codex_backend_auth()
            || !matches!(
                self.chat_widget.current_plan_type(),
                Some(
                    PlanType::Business
                        | PlanType::EnterpriseCbpUsageBased
                        | PlanType::EnterpriseCbpAutomation
                )
            )
        {
            return;
        }
        let Some(thread_id) = self
            .chat_widget
            .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
            .and_then(|index| self.agents_overview.visible_thread_ids.get(index))
            .copied()
        else {
            return;
        };
        if let Some(age) = self
            .agents_overview
            .usage
            .get(&thread_id)
            .and_then(|usage| usage.fetched_at)
            .map(|fetched| fetched.elapsed())
            && age < REFRESH_INTERVAL
        {
            frame_requester.schedule_frame_in(REFRESH_INTERVAL - age);
            return;
        }
        let request_id = Uuid::new_v4();
        self.agents_overview.pending_usage = Some((thread_id, request_id));
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = tokio::time::timeout(
                THREAD_USAGE_FETCH_TIMEOUT,
                fetch_thread_usage(request_handle, thread_id),
            )
            .await
            .map_err(|_| "dashboard usage request timed out".to_string())
            .and_then(|result| result.map_err(|error| error.to_string()));
            app_event_tx.send(AppEvent::AgentsOverviewUsageLoaded {
                thread_id,
                request_id,
                result,
            });
        });
    }

    pub(super) fn finish_agents_overview_usage(
        &mut self,
        thread_id: ThreadId,
        request_id: Uuid,
        result: Result<ThreadUsageOutcome, String>,
    ) {
        if self.agents_overview.pending_usage != Some((thread_id, request_id)) {
            return;
        }
        self.agents_overview.pending_usage = None;
        if self.agents_overview.threads.contains_key(&thread_id) {
            let usage = self.agents_overview.usage.entry(thread_id).or_default();
            usage.fetched_at = Some(Instant::now());
            match result {
                Ok(ThreadUsageOutcome::Available(mut estimate))
                    if estimate.thread_id == thread_id.to_string() =>
                {
                    if let Some(previous) = &usage.estimate {
                        let zero_credits = estimate.estimated_usage_credits_micros == 0
                            && previous.estimated_usage_credits_micros > 0;
                        let zero_cost = estimate.estimated_usage_usd_micros == Some(0)
                            && previous
                                .estimated_usage_usd_micros
                                .is_some_and(|cost| cost > 0);
                        if zero_credits {
                            estimate.estimated_usage_credits_micros =
                                previous.estimated_usage_credits_micros;
                        }
                        if zero_cost {
                            estimate.estimated_usage_usd_micros =
                                previous.estimated_usage_usd_micros;
                        }
                        if (zero_credits || zero_cost) && estimate.groups.is_empty() {
                            estimate.groups.clone_from(&previous.groups);
                        }
                    }
                    usage.estimate = Some(estimate);
                }
                Ok(ThreadUsageOutcome::Disabled) => {
                    self.agents_overview.usage_disabled = true;
                    for usage in self.agents_overview.usage.values_mut() {
                        usage.estimate = None;
                    }
                }
                Ok(ThreadUsageOutcome::Available(_)) | Err(_) => {}
            }
        }
        self.repaint_agents_overview();
    }
}

pub(super) fn usage_lines(usage: &AgentsOverviewUsage) -> Vec<Line<'static>> {
    let sum = |count: fn(&ThreadUsageBreakdownGroup) -> Option<i64>| {
        let groups = &usage.estimate.as_ref()?.groups;
        if groups.is_empty() {
            return None;
        }
        groups.iter().try_fold(/*init*/ 0_i64, |total, group| {
            count(group)
                .filter(|tokens| *tokens >= 0)
                .map(|tokens| total.saturating_add(tokens))
        })
    };
    let input = usage
        .tokens
        .as_ref()
        .map(|tokens| tokens.input_tokens)
        .or_else(|| sum(|group| group.input_tokens));
    let output = usage
        .tokens
        .as_ref()
        .map(|tokens| tokens.output_tokens)
        .or_else(|| sum(|group| group.output_tokens));
    let mut lines = Vec::new();
    let mut tokens = Vec::new();
    if let Some(input) = input {
        tokens.push(format!("{} in", format_tokens_compact(input)));
    }
    if let Some(output) = output {
        tokens.push(format!("{} out", format_tokens_compact(output)));
    }
    if !tokens.is_empty() {
        lines.push(vec!["Tokens: ".dim(), tokens.join(" · ").into()].into());
    }
    if let Some(estimate) = &usage.estimate {
        let mut values = Vec::new();
        if estimate.estimated_usage_credits_micros >= 0 {
            values.push(format!(
                "{} credits",
                format_credit_micros(estimate.estimated_usage_credits_micros)
            ));
        }
        if let Some(cost) = estimate
            .estimated_usage_usd_micros
            .and_then(format_estimated_usd_micros)
        {
            values.push(cost);
        }
        if !values.is_empty() {
            lines.push(vec!["Est. usage: ".dim(), values.join(" · ").into()].into());
        }
    }
    lines
}
