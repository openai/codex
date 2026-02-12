//! Session-wide mutable state.

use std::time::Instant;

use codex_protocol::models::ResponseItem;
use std::collections::HashMap;
use std::collections::HashSet;

use crate::codex::SessionConfiguration;
use crate::config::AutoContinueConfig;
use crate::context_manager::ContextManager;
use crate::protocol::RateLimitSnapshot;
use crate::protocol::TokenUsage;
use crate::protocol::TokenUsageInfo;
use crate::tasks::RegularTask;
use crate::truncate::TruncationPolicy;

/// Persistent, session-scoped state previously stored directly on `Session`.
pub(crate) struct SessionState {
    pub(crate) session_configuration: SessionConfiguration,
    pub(crate) history: ContextManager,
    pub(crate) latest_rate_limits: Option<RateLimitSnapshot>,
    pub(crate) auto_continue: AutoContinueRuntimeState,
    pub(crate) server_reasoning_included: bool,
    pub(crate) dependency_env: HashMap<String, String>,
    pub(crate) mcp_dependency_prompted: HashSet<String>,
    /// Whether the session's initial context has been seeded into history.
    ///
    /// TODO(owen): This is a temporary solution to avoid updating a thread's updated_at
    /// timestamp when resuming a session. Remove this once SQLite is in place.
    pub(crate) initial_context_seeded: bool,
    /// Previous model seen by the session, used for model-switch handling on task start.
    previous_model: Option<String>,
    /// Startup regular task pre-created during session initialization.
    pub(crate) startup_regular_task: Option<RegularTask>,
    pub(crate) active_mcp_tool_selection: Option<Vec<String>>,
}

impl SessionState {
    /// Create a new session state mirroring previous `State::default()` semantics.
    pub(crate) fn new(session_configuration: SessionConfiguration) -> Self {
        let history = ContextManager::new();
        let auto_continue =
            AutoContinueRuntimeState::new(session_configuration.auto_continue.clone());
        Self {
            session_configuration,
            history,
            latest_rate_limits: None,
            auto_continue,
            server_reasoning_included: false,
            dependency_env: HashMap::new(),
            mcp_dependency_prompted: HashSet::new(),
            initial_context_seeded: false,
            previous_model: None,
            startup_regular_task: None,
            active_mcp_tool_selection: None,
        }
    }

    // History helpers
    pub(crate) fn record_items<I>(&mut self, items: I, policy: TruncationPolicy)
    where
        I: IntoIterator,
        I::Item: std::ops::Deref<Target = ResponseItem>,
    {
        self.history.record_items(items, policy);
    }

    pub(crate) fn previous_model(&self) -> Option<String> {
        self.previous_model.clone()
    }
    pub(crate) fn set_previous_model(&mut self, previous_model: Option<String>) {
        self.previous_model = previous_model;
    }

    pub(crate) fn clone_history(&self) -> ContextManager {
        self.history.clone()
    }

    pub(crate) fn replace_history(&mut self, items: Vec<ResponseItem>) {
        self.history.replace(items);
    }

    pub(crate) fn set_token_info(&mut self, info: Option<TokenUsageInfo>) {
        self.history.set_token_info(info);
    }

    // Token/rate limit helpers
    pub(crate) fn update_token_info_from_usage(
        &mut self,
        usage: &TokenUsage,
        model_context_window: Option<i64>,
    ) {
        self.history.update_token_info(usage, model_context_window);
    }

    pub(crate) fn token_info(&self) -> Option<TokenUsageInfo> {
        self.history.token_info()
    }

    pub(crate) fn set_rate_limits(&mut self, snapshot: RateLimitSnapshot) {
        self.latest_rate_limits = Some(merge_rate_limit_fields(
            self.latest_rate_limits.as_ref(),
            snapshot,
        ));
    }

    pub(crate) fn token_info_and_rate_limits(
        &self,
    ) -> (Option<TokenUsageInfo>, Option<RateLimitSnapshot>) {
        (self.token_info(), self.latest_rate_limits.clone())
    }

    pub(crate) fn set_token_usage_full(&mut self, context_window: i64) {
        self.history.set_token_usage_full(context_window);
    }

    pub(crate) fn get_total_token_usage(&self, server_reasoning_included: bool) -> i64 {
        self.history
            .get_total_token_usage(server_reasoning_included)
    }

    pub(crate) fn set_server_reasoning_included(&mut self, included: bool) {
        self.server_reasoning_included = included;
    }

    pub(crate) fn server_reasoning_included(&self) -> bool {
        self.server_reasoning_included
    }

    pub(crate) fn record_mcp_dependency_prompted<I>(&mut self, names: I)
    where
        I: IntoIterator<Item = String>,
    {
        self.mcp_dependency_prompted.extend(names);
    }

    pub(crate) fn mcp_dependency_prompted(&self) -> HashSet<String> {
        self.mcp_dependency_prompted.clone()
    }

    pub(crate) fn set_dependency_env(&mut self, values: HashMap<String, String>) {
        for (key, value) in values {
            self.dependency_env.insert(key, value);
        }
    }

    pub(crate) fn dependency_env(&self) -> HashMap<String, String> {
        self.dependency_env.clone()
    }

    pub(crate) fn set_startup_regular_task(&mut self, task: RegularTask) {
        self.startup_regular_task = Some(task);
    }

    pub(crate) fn take_startup_regular_task(&mut self) -> Option<RegularTask> {
        self.startup_regular_task.take()
    }

    pub(crate) fn merge_mcp_tool_selection(&mut self, tool_names: Vec<String>) -> Vec<String> {
        if tool_names.is_empty() {
            return self.active_mcp_tool_selection.clone().unwrap_or_default();
        }

        let mut merged = self.active_mcp_tool_selection.take().unwrap_or_default();
        let mut seen: HashSet<String> = merged.iter().cloned().collect();

        for tool_name in tool_names {
            if seen.insert(tool_name.clone()) {
                merged.push(tool_name);
            }
        }

        self.active_mcp_tool_selection = Some(merged.clone());
        merged
    }

    pub(crate) fn get_mcp_tool_selection(&self) -> Option<Vec<String>> {
        self.active_mcp_tool_selection.clone()
    }

    pub(crate) fn clear_mcp_tool_selection(&mut self) {
        self.active_mcp_tool_selection = None;
    }

    pub(crate) fn auto_continue(&self) -> &AutoContinueRuntimeState {
        &self.auto_continue
    }

    pub(crate) fn auto_continue_mut(&mut self) -> &mut AutoContinueRuntimeState {
        &mut self.auto_continue
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AutoContinueRuntimeState {
    config: AutoContinueConfig,
    started_at: Instant,
    turns_spawned: u64,
    last_normalized_message: Option<String>,
    repeated_message_streak: u8,
    last_stop_reason: Option<AutoContinueStopReason>,
}

impl AutoContinueRuntimeState {
    pub(crate) fn new(config: AutoContinueConfig) -> Self {
        Self {
            config,
            started_at: Instant::now(),
            turns_spawned: 0,
            last_normalized_message: None,
            repeated_message_streak: 0,
            last_stop_reason: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_with_start(config: AutoContinueConfig, started_at: Instant) -> Self {
        let mut state = Self::new(config);
        state.started_at = started_at;
        state
    }

    pub(crate) fn config(&self) -> &AutoContinueConfig {
        &self.config
    }

    pub(crate) fn disable(&mut self, reason: AutoContinueStopReason) {
        self.config.enabled = false;
        self.last_stop_reason = Some(reason);
    }

    pub(crate) fn force_enable(&mut self) {
        self.config.enabled = true;
        self.started_at = Instant::now();
        self.turns_spawned = 0;
        self.last_normalized_message = None;
        self.repeated_message_streak = 0;
        self.last_stop_reason = None;
    }

    pub(crate) fn turns_spawned(&self) -> u64 {
        self.turns_spawned
    }

    pub(crate) fn elapsed(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }

    pub(crate) fn decide(&mut self, last_agent_message: Option<&str>) -> AutoContinueDecision {
        if !self.config.enabled {
            return AutoContinueDecision::Stop(AutoContinueStopReason::Disabled);
        }

        if let Some(limit) = self.config.max_turns
            && self.turns_spawned >= limit.get()
        {
            self.disable(AutoContinueStopReason::MaxTurns);
            return AutoContinueDecision::Stop(AutoContinueStopReason::MaxTurns);
        }

        if let Some(max_duration) = self.config.max_duration
            && self.started_at.elapsed() >= max_duration
        {
            self.disable(AutoContinueStopReason::MaxDuration);
            return AutoContinueDecision::Stop(AutoContinueStopReason::MaxDuration);
        }

        let Some(message) = last_agent_message.map(str::trim).filter(|m| !m.is_empty()) else {
            self.disable(AutoContinueStopReason::EmptyMessage);
            return AutoContinueDecision::Stop(AutoContinueStopReason::EmptyMessage);
        };

        let lower = message.to_ascii_lowercase();

        if !message_has_next_step_signal(&lower) {
            if message_requires_user_input(&lower) {
                self.disable(AutoContinueStopReason::AwaitingUserInput);
                return AutoContinueDecision::Stop(AutoContinueStopReason::AwaitingUserInput);
            }
            if has_negative_stop_phrase(&lower) {
                self.disable(AutoContinueStopReason::StopPhrase);
                return AutoContinueDecision::Stop(AutoContinueStopReason::StopPhrase);
            }
        }

        let normalized = normalize_for_repetition(message);
        if self.update_repetition(normalized) {
            self.disable(AutoContinueStopReason::RepeatedOutput);
            return AutoContinueDecision::Stop(AutoContinueStopReason::RepeatedOutput);
        }

        self.last_stop_reason = None;
        self.turns_spawned = self.turns_spawned.saturating_add(1);
        AutoContinueDecision::Continue {
            prompt: self.config.prompt.clone(),
        }
    }

    fn update_repetition(&mut self, normalized: String) -> bool {
        if let Some(prev) = &self.last_normalized_message {
            if prev == &normalized {
                self.repeated_message_streak = self.repeated_message_streak.saturating_add(1);
            } else {
                self.last_normalized_message = Some(normalized);
                self.repeated_message_streak = 0;
            }
        } else {
            self.last_normalized_message = Some(normalized);
            self.repeated_message_streak = 0;
        }
        self.repeated_message_streak > 0
    }

    pub(crate) fn rearm_if_allowed(&mut self) -> bool {
        if self.config.enabled {
            return false;
        }

        match self.last_stop_reason {
            Some(AutoContinueStopReason::Disabled) | None => false,
            Some(_) => {
                self.force_enable();
                true
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum AutoContinueDecision {
    Continue { prompt: String },
    Stop(AutoContinueStopReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AutoContinueStopReason {
    Disabled,
    MaxTurns,
    MaxDuration,
    EmptyMessage,
    StopPhrase,
    RepeatedOutput,
    Interrupted,
    /// Assistant is waiting on user-provided inputs (e.g.,
    /// "I cannot proceed without the sessions and config path").
    AwaitingUserInput,
}

fn normalize_for_repetition(message: &str) -> String {
    message
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn has_negative_stop_phrase(lower: &str) -> bool {
    AUTO_CONTINUE_NEGATIVE_PATTERNS
        .iter()
        .any(|pattern| lower.contains(pattern))
}

fn message_has_next_step_signal(lower: &str) -> bool {
    AUTO_CONTINUE_NEXT_PATTERNS
        .iter()
        .any(|pattern| lower.contains(pattern))
}

fn message_requires_user_input(lower: &str) -> bool {
    AUTO_CONTINUE_INPUT_BLOCKED_PATTERNS
        .iter()
        .any(|pattern| lower.contains(pattern))
}

const AUTO_CONTINUE_NEGATIVE_PATTERNS: &[&str] = &[
    "no further action",
    "no further actions",
    "no further steps",
    "nothing else to do",
    "nothing more to do",
    "nothing else remains",
    "let me know if you need anything else",
    "let me know if you need more help",
    "all done",
    "we're done",
    "were done",
    "that concludes",
    "that should conclude",
    "we are finished",
    "i am finished",
    "this completes",
    "this should complete",
    "no remaining tasks",
    "nothing outstanding",
];

// Phrases that indicate the assistant is blocked until the user provides
// required information or confirmation.
const AUTO_CONTINUE_INPUT_BLOCKED_PATTERNS: &[&str] = &[
    "cannot proceed without",
    "can't proceed without",
    "cannot continue without",
    "can't continue without",
    "unable to proceed without",
    "blocked until you provide",
    "blocked until you share",
    "i'm blocked until you",
    "i am blocked until you",
    "i need the",
    "i need",
    "need the",
    "need you to",
    "please provide",
    "please share",
    "provide them to continue",
    "provide them to proceed",
    "share them to continue",
    "share them to proceed",
    "standing by for",
    "standing by",
    "on standby",
    "waiting for you to",
    "awaiting your",
    "waiting on your",
    "ready to proceed as soon as you",
    "ready to execute as soon as you",
];

const AUTO_CONTINUE_NEXT_PATTERNS: &[&str] = &[
    "next step",
    "next steps",
    "plan:",
    "plan for next",
    "pending work",
    "action items",
    "follow-up",
    "follow up",
    "todo",
    "to-do",
];

#[cfg(test)]
mod auto_continue_tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::num::NonZeroU64;
    use std::time::Duration;
    use std::time::Instant;

    fn make_config() -> AutoContinueConfig {
        AutoContinueConfig {
            enabled: true,
            prompt: "continue".to_string(),
            max_turns: None,
            max_duration: None,
        }
    }

    #[test]
    fn auto_continue_continues_with_next_steps() {
        let mut runtime = AutoContinueRuntimeState::new(make_config());
        let decision = runtime.decide(Some("Next steps:\n- Do a thing"));
        match decision {
            AutoContinueDecision::Continue { prompt } => assert_eq!(prompt, "continue"),
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn auto_continue_stops_on_stop_phrase() {
        let mut runtime = AutoContinueRuntimeState::new(make_config());
        match runtime.decide(Some("No further actions remain.")) {
            AutoContinueDecision::Stop(AutoContinueStopReason::StopPhrase) => {}
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn auto_continue_stops_when_awaiting_user_input() {
        let mut runtime = AutoContinueRuntimeState::new(make_config());
        let msg = "I cannot proceed without the sessions and config path. Please provide them to continue.";
        match runtime.decide(Some(msg)) {
            AutoContinueDecision::Stop(AutoContinueStopReason::AwaitingUserInput) => {}
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn auto_continue_stops_on_repeated_message() {
        let mut runtime = AutoContinueRuntimeState::new(make_config());
        assert!(matches!(
            runtime.decide(Some("Next steps:\n- Do a thing")),
            AutoContinueDecision::Continue { .. }
        ));
        assert!(matches!(
            runtime.decide(Some("Next steps:\n- Do a thing")),
            AutoContinueDecision::Stop(AutoContinueStopReason::RepeatedOutput)
        ));
    }

    #[test]
    fn auto_continue_respects_max_turns() {
        let mut config = make_config();
        config.max_turns = NonZeroU64::new(1);
        let mut runtime = AutoContinueRuntimeState::new(config);
        assert!(matches!(
            runtime.decide(Some("Next steps:\n- Do a thing")),
            AutoContinueDecision::Continue { .. }
        ));
        assert!(matches!(
            runtime.decide(Some("Next steps:\n- Do a thing")),
            AutoContinueDecision::Stop(AutoContinueStopReason::MaxTurns)
        ));
    }

    #[test]
    fn auto_continue_respects_max_duration() {
        let mut config = make_config();
        config.max_duration = Some(Duration::from_secs(1));
        let started_at = Instant::now() - Duration::from_secs(2);
        let mut runtime = AutoContinueRuntimeState::new_with_start(config, started_at);
        match runtime.decide(Some("Next steps:\n- Do a thing")) {
            AutoContinueDecision::Stop(AutoContinueStopReason::MaxDuration) => {}
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn auto_continue_rearms_after_stop_phrase() {
        let mut runtime = AutoContinueRuntimeState::new(make_config());
        assert!(matches!(
            runtime.decide(Some("No further actions remain.")),
            AutoContinueDecision::Stop(AutoContinueStopReason::StopPhrase)
        ));
        assert!(runtime.rearm_if_allowed());
        assert!(runtime.config().enabled);
        assert_eq!(0, runtime.turns_spawned());
    }

    #[test]
    fn auto_continue_does_not_rearm_after_manual_disable() {
        let mut runtime = AutoContinueRuntimeState::new(make_config());
        runtime.disable(AutoContinueStopReason::Disabled);
        assert!(!runtime.rearm_if_allowed());
        assert!(!runtime.config().enabled);
    }

    #[test]
    fn auto_continue_force_enable_overrides_manual_disable() {
        let mut runtime = AutoContinueRuntimeState::new(make_config());
        runtime.disable(AutoContinueStopReason::Disabled);
        runtime.force_enable();
        assert!(runtime.config().enabled);
        assert_eq!(0, runtime.turns_spawned());
    }

    #[test]
    fn auto_continue_rearms_after_interrupt() {
        let mut runtime = AutoContinueRuntimeState::new(make_config());
        runtime.disable(AutoContinueStopReason::Interrupted);
        assert!(runtime.rearm_if_allowed());
        assert!(runtime.config().enabled);
    }

    #[test]
    fn auto_continue_rearms_after_awaiting_user_input() {
        let mut runtime = AutoContinueRuntimeState::new(make_config());
        runtime.disable(AutoContinueStopReason::AwaitingUserInput);
        assert!(runtime.rearm_if_allowed());
        assert!(runtime.config().enabled);
    }
}

// Sometimes new snapshots don't include credits or plan information.
// Preserve those from the previous snapshot when missing. For `limit_id`, treat
// missing values as the default `"codex"` bucket.
fn merge_rate_limit_fields(
    previous: Option<&RateLimitSnapshot>,
    mut snapshot: RateLimitSnapshot,
) -> RateLimitSnapshot {
    if snapshot.limit_id.is_none() {
        snapshot.limit_id = Some("codex".to_string());
    }
    if snapshot.credits.is_none() {
        snapshot.credits = previous.and_then(|prior| prior.credits.clone());
    }
    if snapshot.plan_type.is_none() {
        snapshot.plan_type = previous.and_then(|prior| prior.plan_type);
    }
    snapshot
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::make_session_configuration_for_tests;
    use crate::protocol::RateLimitWindow;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn merge_mcp_tool_selection_deduplicates_and_preserves_order() {
        let session_configuration = make_session_configuration_for_tests().await;
        let mut state = SessionState::new(session_configuration);

        let merged = state.merge_mcp_tool_selection(vec![
            "mcp__rmcp__echo".to_string(),
            "mcp__rmcp__image".to_string(),
            "mcp__rmcp__echo".to_string(),
        ]);
        assert_eq!(
            merged,
            vec![
                "mcp__rmcp__echo".to_string(),
                "mcp__rmcp__image".to_string(),
            ]
        );

        let merged = state.merge_mcp_tool_selection(vec![
            "mcp__rmcp__image".to_string(),
            "mcp__rmcp__search".to_string(),
        ]);
        assert_eq!(
            merged,
            vec![
                "mcp__rmcp__echo".to_string(),
                "mcp__rmcp__image".to_string(),
                "mcp__rmcp__search".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn merge_mcp_tool_selection_empty_input_is_noop() {
        let session_configuration = make_session_configuration_for_tests().await;
        let mut state = SessionState::new(session_configuration);
        state.merge_mcp_tool_selection(vec![
            "mcp__rmcp__echo".to_string(),
            "mcp__rmcp__image".to_string(),
        ]);

        let merged = state.merge_mcp_tool_selection(Vec::new());
        assert_eq!(
            merged,
            vec![
                "mcp__rmcp__echo".to_string(),
                "mcp__rmcp__image".to_string(),
            ]
        );
        assert_eq!(
            state.get_mcp_tool_selection(),
            Some(vec![
                "mcp__rmcp__echo".to_string(),
                "mcp__rmcp__image".to_string(),
            ])
        );
    }

    #[tokio::test]
    async fn clear_mcp_tool_selection_removes_selection() {
        let session_configuration = make_session_configuration_for_tests().await;
        let mut state = SessionState::new(session_configuration);
        state.merge_mcp_tool_selection(vec!["mcp__rmcp__echo".to_string()]);

        state.clear_mcp_tool_selection();

        assert_eq!(state.get_mcp_tool_selection(), None);
    }

    #[tokio::test]
    async fn set_rate_limits_defaults_limit_id_to_codex_when_missing() {
        let session_configuration = make_session_configuration_for_tests().await;
        let mut state = SessionState::new(session_configuration);

        state.set_rate_limits(RateLimitSnapshot {
            limit_id: None,
            limit_name: None,
            primary: Some(RateLimitWindow {
                used_percent: 12.0,
                window_minutes: Some(60),
                resets_at: Some(100),
            }),
            secondary: None,
            credits: None,
            plan_type: None,
        });

        assert_eq!(
            state
                .latest_rate_limits
                .as_ref()
                .and_then(|v| v.limit_id.clone()),
            Some("codex".to_string())
        );
    }

    #[tokio::test]
    async fn set_rate_limits_defaults_to_codex_when_limit_id_missing_after_other_bucket() {
        let session_configuration = make_session_configuration_for_tests().await;
        let mut state = SessionState::new(session_configuration);

        state.set_rate_limits(RateLimitSnapshot {
            limit_id: Some("codex_other".to_string()),
            limit_name: Some("codex_other".to_string()),
            primary: Some(RateLimitWindow {
                used_percent: 20.0,
                window_minutes: Some(60),
                resets_at: Some(200),
            }),
            secondary: None,
            credits: None,
            plan_type: None,
        });
        state.set_rate_limits(RateLimitSnapshot {
            limit_id: None,
            limit_name: None,
            primary: Some(RateLimitWindow {
                used_percent: 30.0,
                window_minutes: Some(60),
                resets_at: Some(300),
            }),
            secondary: None,
            credits: None,
            plan_type: None,
        });

        assert_eq!(
            state
                .latest_rate_limits
                .as_ref()
                .and_then(|v| v.limit_id.clone()),
            Some("codex".to_string())
        );
    }

    #[tokio::test]
    async fn set_rate_limits_carries_credits_and_plan_type_from_codex_to_codex_other() {
        let session_configuration = make_session_configuration_for_tests().await;
        let mut state = SessionState::new(session_configuration);

        state.set_rate_limits(RateLimitSnapshot {
            limit_id: Some("codex".to_string()),
            limit_name: Some("codex".to_string()),
            primary: Some(RateLimitWindow {
                used_percent: 10.0,
                window_minutes: Some(60),
                resets_at: Some(100),
            }),
            secondary: None,
            credits: Some(crate::protocol::CreditsSnapshot {
                has_credits: true,
                unlimited: false,
                balance: Some("50".to_string()),
            }),
            plan_type: Some(codex_protocol::account::PlanType::Plus),
        });

        state.set_rate_limits(RateLimitSnapshot {
            limit_id: Some("codex_other".to_string()),
            limit_name: None,
            primary: Some(RateLimitWindow {
                used_percent: 30.0,
                window_minutes: Some(120),
                resets_at: Some(200),
            }),
            secondary: None,
            credits: None,
            plan_type: None,
        });

        assert_eq!(
            state.latest_rate_limits,
            Some(RateLimitSnapshot {
                limit_id: Some("codex_other".to_string()),
                limit_name: None,
                primary: Some(RateLimitWindow {
                    used_percent: 30.0,
                    window_minutes: Some(120),
                    resets_at: Some(200),
                }),
                secondary: None,
                credits: Some(crate::protocol::CreditsSnapshot {
                    has_credits: true,
                    unlimited: false,
                    balance: Some("50".to_string()),
                }),
                plan_type: Some(codex_protocol::account::PlanType::Plus),
            })
        );
    }
}
