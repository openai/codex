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
    /// Feeds an existing analytics fact into the wrapped reducer for conformance tests.
    ///
    /// The observation stream is being introduced incrementally, so tests need
    /// to hold the not-yet-migrated lifecycle context constant while swapping a
    /// specific source from legacy facts to observations.
    #[cfg(test)]
    pub(crate) async fn ingest_existing_fact_for_test(
        &mut self,
        fact: AnalyticsFact,
        out: &mut Vec<TrackEventRequest>,
    ) {
        self.legacy.ingest(fact, out).await;
    }

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

    /// Ingests a thread.started observation into the current analytics reducer state.
    pub(crate) fn ingest_thread_started(
        &mut self,
        connection_id: u64,
        observation: events::ThreadStarted<'_>,
        out: &mut Vec<TrackEventRequest>,
    ) {
        self.legacy
            .ingest_observed_thread_started(connection_id, observation, out);
    }

    /// Ingests a turn.started observation into the current turn-event reducer state.
    pub(crate) async fn ingest_turn_started(
        &mut self,
        observation: events::TurnStarted<'_>,
        out: &mut Vec<TrackEventRequest>,
    ) {
        self.legacy
            .ingest(
                AnalyticsFact::Custom(CustomAnalyticsFact::TurnResolvedConfig(Box::new(
                    observation_projection::turn_resolved_config_fact(&observation),
                ))),
                out,
            )
            .await;

        self.legacy
            .ingest(
                AnalyticsFact::Notification(Box::new(
                    observation_projection::turn_started_notification(observation),
                )),
                out,
            )
            .await;
    }

    /// Ingests a turn.ended observation into the current turn-event reducer state.
    pub(crate) async fn ingest_turn_ended(
        &mut self,
        observation: events::TurnEnded<'_>,
        out: &mut Vec<TrackEventRequest>,
    ) {
        if let Some(token_usage) = observation_projection::turn_token_usage_fact(&observation) {
            self.legacy
                .ingest(
                    AnalyticsFact::Custom(CustomAnalyticsFact::TurnTokenUsage(Box::new(
                        token_usage,
                    ))),
                    out,
                )
                .await;
        }

        self.legacy
            .ingest(
                AnalyticsFact::Notification(Box::new(
                    observation_projection::turn_ended_notification(observation),
                )),
                out,
            )
            .await;
    }

    /// Ingests a turn.steer observation and emits the current analytics event.
    pub(crate) fn ingest_turn_steer(
        &mut self,
        connection_id: u64,
        observation: events::TurnSteer<'_>,
        out: &mut Vec<TrackEventRequest>,
    ) {
        self.legacy
            .ingest_observed_turn_steer(connection_id, observation, out);
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
