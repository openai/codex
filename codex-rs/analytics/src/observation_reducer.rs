//! Projection from shared observations into analytics track events.
//!
//! This module deliberately reuses the existing analytics reducer while the
//! shared observation stream is being introduced. That gives conformance tests
//! a small, stable bridge: one side feeds legacy analytics facts and the other
//! feeds typed observations, then both paths must produce identical track
//! requests.

use crate::events::TrackEventRequest;
use crate::facts;
use crate::facts::AnalyticsFact;
use crate::facts::AppInvocation;
use crate::facts::AppUsedInput;
use crate::facts::CustomAnalyticsFact;
use crate::facts::TrackEventsContext;
use crate::reducer::AnalyticsReducer;
use codex_observability::events;

/// Analytics reducer entrypoint for typed observations.
#[derive(Default)]
pub(crate) struct AnalyticsObservationReducer {
    legacy: AnalyticsReducer,
}

impl AnalyticsObservationReducer {
    /// Ingests an app.used observation and emits the current analytics event.
    pub(crate) async fn ingest_app_used(
        &mut self,
        observation: events::AppUsed<'_>,
        out: &mut Vec<TrackEventRequest>,
    ) {
        let tracking = TrackEventsContext {
            model_slug: observation.model_slug.to_string(),
            thread_id: observation.thread_id.to_string(),
            turn_id: observation.turn_id.to_string(),
        };
        let app = AppInvocation {
            connector_id: observation.connector_id.map(str::to_string),
            app_name: observation.app_name.map(str::to_string),
            invocation_type: observation.invocation_type.map(map_invocation_type),
        };

        self.legacy
            .ingest(
                AnalyticsFact::Custom(CustomAnalyticsFact::AppUsed(AppUsedInput { tracking, app })),
                out,
            )
            .await;
    }
}

fn map_invocation_type(invocation_type: events::InvocationType) -> facts::InvocationType {
    match invocation_type {
        events::InvocationType::Explicit => facts::InvocationType::Explicit,
        events::InvocationType::Implicit => facts::InvocationType::Implicit,
    }
}
