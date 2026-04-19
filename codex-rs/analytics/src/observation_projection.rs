//! Projection from shared observations into the current analytics schema.
//!
//! The observation taxonomy is intended to describe what Codex did, not the
//! shape of any particular telemetry backend. This private module is the
//! adapter boundary where typed observations are translated into the legacy
//! analytics facts and track-event payloads that already exist today.

use crate::events::CodexHookRunEventRequest;
use crate::events::CodexHookRunMetadata;
use crate::events::CodexPluginEventRequest;
use crate::events::CodexPluginMetadata;
use crate::events::CodexPluginUsedEventRequest;
use crate::events::CodexPluginUsedMetadata;
use crate::events::TrackEventRequest;
use crate::facts;
use crate::facts::AppInvocation;
use crate::facts::AppMentionedInput;
use crate::facts::AppUsedInput;
use crate::facts::TrackEventsContext;
use crate::facts::TurnTokenUsageFact;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartedNotification;
use codex_app_server_protocol::TurnStatus as AppServerTurnStatus;
use codex_login::default_client::originator;
use codex_observability::events;
use codex_protocol::protocol::HookRunStatus as ProtocolHookRunStatus;
use codex_protocol::protocol::TokenUsage;

pub(crate) fn app_mentioned_input(observation: events::AppMentioned<'_>) -> AppMentionedInput {
    AppMentionedInput {
        tracking: tracking_from_fields(
            observation.model_slug,
            observation.thread_id,
            observation.turn_id,
        ),
        mentions: vec![app_invocation_from_fields(
            observation.connector_id,
            observation.app_name,
            observation.invocation_type,
        )],
    }
}

pub(crate) fn app_used_input(observation: events::AppUsed<'_>) -> AppUsedInput {
    AppUsedInput {
        tracking: tracking_from_fields(
            observation.model_slug,
            observation.thread_id,
            observation.turn_id,
        ),
        app: app_invocation_from_fields(
            observation.connector_id,
            observation.app_name,
            observation.invocation_type,
        ),
    }
}

pub(crate) fn hook_run_completed_event(
    observation: events::HookRunCompleted<'_>,
) -> TrackEventRequest {
    TrackEventRequest::HookRun(CodexHookRunEventRequest {
        event_type: "codex_hook_run",
        event_params: CodexHookRunMetadata {
            thread_id: Some(observation.thread_id.to_string()),
            turn_id: Some(observation.turn_id.to_string()),
            model_slug: Some(observation.model_slug.to_string()),
            hook_name: Some(observation.hook_name.to_string()),
            hook_source: Some(observation.hook_source),
            status: Some(match observation.status {
                events::HookRunStatus::Completed => ProtocolHookRunStatus::Completed,
                events::HookRunStatus::Failed => ProtocolHookRunStatus::Failed,
                events::HookRunStatus::Blocked => ProtocolHookRunStatus::Blocked,
                events::HookRunStatus::Stopped => ProtocolHookRunStatus::Stopped,
            }),
        },
    })
}

pub(crate) fn plugin_used_event(observation: events::PluginUsed<'_>) -> TrackEventRequest {
    TrackEventRequest::PluginUsed(CodexPluginUsedEventRequest {
        event_type: "codex_plugin_used",
        event_params: CodexPluginUsedMetadata {
            plugin: plugin_metadata_from_fields(
                observation.plugin_id,
                observation.plugin_name,
                observation.marketplace_name,
                observation.has_skills,
                observation.mcp_server_count,
                observation.connector_ids,
            ),
            thread_id: Some(observation.thread_id.to_string()),
            turn_id: Some(observation.turn_id.to_string()),
            model_slug: Some(observation.model_slug.to_string()),
        },
    })
}

pub(crate) fn plugin_state_changed_event(
    observation: events::PluginStateChanged<'_>,
) -> TrackEventRequest {
    let event = CodexPluginEventRequest {
        event_type: match observation.state {
            events::PluginState::Installed => "codex_plugin_installed",
            events::PluginState::Uninstalled => "codex_plugin_uninstalled",
            events::PluginState::Enabled => "codex_plugin_enabled",
            events::PluginState::Disabled => "codex_plugin_disabled",
        },
        event_params: plugin_metadata_from_fields(
            observation.plugin_id,
            observation.plugin_name,
            observation.marketplace_name,
            observation.has_skills,
            observation.mcp_server_count,
            observation.connector_ids,
        ),
    };

    match observation.state {
        events::PluginState::Installed => TrackEventRequest::PluginInstalled(event),
        events::PluginState::Uninstalled => TrackEventRequest::PluginUninstalled(event),
        events::PluginState::Enabled => TrackEventRequest::PluginEnabled(event),
        events::PluginState::Disabled => TrackEventRequest::PluginDisabled(event),
    }
}

pub(crate) fn turn_started_notification(
    observation: events::TurnStarted<'_>,
) -> ServerNotification {
    ServerNotification::TurnStarted(TurnStartedNotification {
        thread_id: observation.thread_id.to_string(),
        turn: Turn {
            id: observation.turn_id.to_string(),
            items: vec![],
            status: AppServerTurnStatus::InProgress,
            error: None,
            started_at: Some(observation.started_at),
            completed_at: None,
            duration_ms: None,
        },
    })
}

pub(crate) fn turn_token_usage_fact(
    observation: &events::TurnEnded<'_>,
) -> Option<TurnTokenUsageFact> {
    let token_usage = observation.token_usage?;
    Some(TurnTokenUsageFact {
        turn_id: observation.turn_id.to_string(),
        thread_id: observation.thread_id.to_string(),
        token_usage: TokenUsage {
            input_tokens: token_usage.input_tokens,
            cached_input_tokens: token_usage.cached_input_tokens,
            output_tokens: token_usage.output_tokens,
            reasoning_output_tokens: token_usage.reasoning_output_tokens,
            total_tokens: token_usage.total_tokens,
        },
    })
}

pub(crate) fn turn_ended_notification(observation: events::TurnEnded<'_>) -> ServerNotification {
    ServerNotification::TurnCompleted(TurnCompletedNotification {
        thread_id: observation.thread_id.to_string(),
        turn: Turn {
            id: observation.turn_id.to_string(),
            items: vec![],
            status: match observation.status {
                events::TurnStatus::Completed => AppServerTurnStatus::Completed,
                events::TurnStatus::Failed => AppServerTurnStatus::Failed,
                events::TurnStatus::Interrupted => AppServerTurnStatus::Interrupted,
            },
            // Error taxonomy needs a separate design pass. Keeping it out of
            // the first terminal-turn observation avoids baking app-server
            // transport categories into the shared event model.
            error: None,
            started_at: None,
            completed_at: Some(observation.ended_at),
            duration_ms: Some(observation.duration_ms),
        },
    })
}

fn tracking_from_fields(model_slug: &str, thread_id: &str, turn_id: &str) -> TrackEventsContext {
    TrackEventsContext {
        model_slug: model_slug.to_string(),
        thread_id: thread_id.to_string(),
        turn_id: turn_id.to_string(),
    }
}

fn app_invocation_from_fields(
    connector_id: Option<&str>,
    app_name: Option<&str>,
    invocation_type: Option<events::InvocationType>,
) -> AppInvocation {
    AppInvocation {
        connector_id: connector_id.map(str::to_string),
        app_name: app_name.map(str::to_string),
        invocation_type: invocation_type.map(map_invocation_type),
    }
}

fn plugin_metadata_from_fields(
    plugin_id: &str,
    plugin_name: &str,
    marketplace_name: &str,
    has_skills: Option<bool>,
    mcp_server_count: Option<usize>,
    connector_ids: Option<&[String]>,
) -> CodexPluginMetadata {
    CodexPluginMetadata {
        plugin_id: Some(plugin_id.to_string()),
        plugin_name: Some(plugin_name.to_string()),
        marketplace_name: Some(marketplace_name.to_string()),
        has_skills,
        mcp_server_count,
        connector_ids: connector_ids.map(<[String]>::to_vec),
        product_client_id: Some(originator().value),
    }
}

fn map_invocation_type(invocation_type: events::InvocationType) -> facts::InvocationType {
    match invocation_type {
        events::InvocationType::Explicit => facts::InvocationType::Explicit,
        events::InvocationType::Implicit => facts::InvocationType::Implicit,
    }
}
