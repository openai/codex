//! Analytics reducer entrypoint for shared observations.
//!
//! This module deliberately reuses the existing analytics reducer while the
//! shared observation stream is being introduced. That gives conformance tests
//! a small, stable bridge: one side feeds legacy analytics facts and the other
//! feeds typed observations, then both paths must produce identical track
//! requests.

use crate::events::TrackEventRequest;
use crate::facts::AnalyticsFact;
use crate::facts::CustomAnalyticsFact;
use crate::observation_projection;
use crate::reducer::AnalyticsReducer;
use codex_observability::events;

/// Analytics reducer entrypoint for typed observations.
#[derive(Default)]
pub(crate) struct AnalyticsObservationReducer {
    legacy: AnalyticsReducer,
}

impl AnalyticsObservationReducer {
    /// Ingests an app.mentioned observation and emits the current analytics event.
    pub(crate) async fn ingest_app_mentioned(
        &mut self,
        observation: events::AppMentioned<'_>,
        out: &mut Vec<TrackEventRequest>,
    ) {
        self.legacy
            .ingest(
                AnalyticsFact::Custom(CustomAnalyticsFact::AppMentioned(
                    observation_projection::app_mentioned_input(observation),
                )),
                out,
            )
            .await;
    }

    /// Ingests an app.used observation and emits the current analytics event.
    pub(crate) async fn ingest_app_used(
        &mut self,
        observation: events::AppUsed<'_>,
        out: &mut Vec<TrackEventRequest>,
    ) {
        self.legacy
            .ingest(
                AnalyticsFact::Custom(CustomAnalyticsFact::AppUsed(
                    observation_projection::app_used_input(observation),
                )),
                out,
            )
            .await;
    }

    /// Ingests a hook.run_completed observation and emits the current analytics event.
    pub(crate) fn ingest_hook_run_completed(
        &mut self,
        observation: events::HookRunCompleted<'_>,
        out: &mut Vec<TrackEventRequest>,
    ) {
        out.push(observation_projection::hook_run_completed_event(
            observation,
        ));
    }

    /// Ingests a plugin.used observation and emits the current analytics event.
    pub(crate) fn ingest_plugin_used(
        &mut self,
        observation: events::PluginUsed<'_>,
        out: &mut Vec<TrackEventRequest>,
    ) {
        out.push(observation_projection::plugin_used_event(observation));
    }

    /// Ingests a plugin.state_changed observation and emits the current analytics event.
    pub(crate) fn ingest_plugin_state_changed(
        &mut self,
        observation: events::PluginStateChanged<'_>,
        out: &mut Vec<TrackEventRequest>,
    ) {
        out.push(observation_projection::plugin_state_changed_event(
            observation,
        ));
    }
}
