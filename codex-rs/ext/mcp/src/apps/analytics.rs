use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::PoisonError;

use codex_analytics::AppInvocation;
use codex_analytics::InvocationType;
use codex_analytics::TrackEventsContext;
use codex_analytics::build_track_events_context;
use codex_apps::CodexAppsSnapshot;
use codex_core_skills::injection::ToolMentionKind;
use codex_core_skills::injection::app_id_from_path;
use codex_core_skills::injection::extract_tool_mentions;
use codex_core_skills::injection::tool_kind_for_path;
use codex_extension_api::ContextualUserFragment;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ToolFinishInput;
use codex_extension_api::ToolLifecycleContributor;
use codex_extension_api::ToolLifecycleFuture;
use codex_extension_api::TurnInputContext;
use codex_extension_api::TurnInputContributor;
use codex_protocol::user_input::UserInput;

use super::CodexAppsMcpExtension;
use super::presentation::AppsThreadState;

struct AppsTurnAnalyticsState {
    tracking: TrackEventsContext,
    explicit_app_ids: Mutex<HashSet<String>>,
}

#[derive(Clone)]
pub(super) struct AppsToolUsage {
    connector_id: String,
    connector_name: String,
}

#[derive(Default)]
struct AppsToolUsageState {
    by_call: Mutex<HashMap<String, AppsToolUsage>>,
}

impl TurnInputContributor for CodexAppsMcpExtension {
    fn contribute<'a>(
        &'a self,
        input: TurnInputContext,
        _session_store: &'a ExtensionData,
        thread_store: &'a ExtensionData,
        turn_store: &'a ExtensionData,
    ) -> ExtensionFuture<'a, Vec<Box<dyn ContextualUserFragment + Send>>> {
        Box::pin(async move {
            let explicit_app_ids = collect_explicit_app_ids(&input.user_input);
            let snapshot = thread_store
                .get::<AppsThreadState>()
                .and_then(|state| state.snapshot());
            let mentions = mentioned_app_invocations(&explicit_app_ids, snapshot.as_ref());
            let turn_state = turn_store.get_or_init(|| AppsTurnAnalyticsState {
                tracking: build_track_events_context(
                    input.model_slug,
                    thread_store.level_id().to_string(),
                    input.turn_id,
                    input.product_client_id,
                ),
                explicit_app_ids: Mutex::new(HashSet::new()),
            });
            turn_state
                .explicit_app_ids
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .extend(explicit_app_ids);
            self.analytics_events_client
                .track_app_mentioned(turn_state.tracking.clone(), mentions);
            Vec::new()
        })
    }
}

impl ToolLifecycleContributor for CodexAppsMcpExtension {
    fn on_tool_finish<'a>(&'a self, input: ToolFinishInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(async move {
            let Some((tracking, invocation)) =
                app_invocation_for_finished_call(input.turn_store, input.call_id)
            else {
                return;
            };
            self.analytics_events_client
                .track_app_used(tracking, invocation);
        })
    }
}

pub(super) fn remember_app_tool_usage(
    turn_store: &ExtensionData,
    call_id: &str,
    connector_id: &str,
    connector_name: &str,
) {
    turn_store
        .get_or_init(AppsToolUsageState::default)
        .by_call
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(
            call_id.to_string(),
            AppsToolUsage {
                connector_id: connector_id.to_string(),
                connector_name: connector_name.to_string(),
            },
        );
}

fn collect_explicit_app_ids(inputs: &[UserInput]) -> HashSet<String> {
    let mut app_ids = HashSet::new();
    for input in inputs {
        match input {
            UserInput::Text { text, .. } => {
                for path in extract_tool_mentions(text).paths() {
                    insert_app_id(path, &mut app_ids);
                }
            }
            UserInput::Mention { path, .. } => insert_app_id(path, &mut app_ids),
            _ => {}
        }
    }
    app_ids
}

fn insert_app_id(path: &str, app_ids: &mut HashSet<String>) {
    if tool_kind_for_path(path) == ToolMentionKind::App
        && let Some(app_id) = app_id_from_path(path)
    {
        app_ids.insert(app_id.to_string());
    }
}

fn mentioned_app_invocations(
    explicit_app_ids: &HashSet<String>,
    snapshot: Option<&CodexAppsSnapshot>,
) -> Vec<AppInvocation> {
    let mut app_ids = explicit_app_ids.iter().collect::<Vec<_>>();
    app_ids.sort_unstable();
    app_ids
        .into_iter()
        .map(|app_id| AppInvocation {
            connector_id: Some(app_id.clone()),
            app_name: snapshot.and_then(|snapshot| {
                snapshot
                    .all_connectors()
                    .iter()
                    .find(|app| app.id() == app_id.as_str())
                    .map(|app| app.name().to_string())
            }),
            invocation_type: Some(InvocationType::Explicit),
        })
        .collect()
}

fn app_invocation_for_finished_call(
    turn_store: &ExtensionData,
    call_id: &str,
) -> Option<(TrackEventsContext, AppInvocation)> {
    let usage = turn_store
        .get::<AppsToolUsageState>()?
        .by_call
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(call_id)?;
    let turn_state = turn_store.get::<AppsTurnAnalyticsState>()?;
    let explicit = turn_state
        .explicit_app_ids
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .contains(&usage.connector_id);
    let invocation_type = if explicit {
        InvocationType::Explicit
    } else {
        InvocationType::Implicit
    };
    Some((
        turn_state.tracking.clone(),
        AppInvocation {
            connector_id: Some(usage.connector_id),
            app_name: Some(usage.connector_name),
            invocation_type: Some(invocation_type),
        },
    ))
}

#[cfg(test)]
#[path = "analytics_tests.rs"]
mod tests;
