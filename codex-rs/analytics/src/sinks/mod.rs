mod codex_backend;
mod local;

use crate::events::TrackEventRequest;
use codex_login::AuthManager;
use std::sync::Arc;

pub use local::LOCAL_ANALYTICS_SCHEMA_VERSION;
pub use local::LocalAnalyticsRecord;
pub use local::LocalAnalyticsRecordType;
pub(crate) use local::SharedLocalAnalyticsSink;
#[cfg(test)]
pub(crate) use local::local_analytics_sink_for_path;
pub(crate) use local::local_analytics_sink_from_env;

const CODEX_ANALYTICS_EVENT_SINKS: AnalyticsEventSinkSet =
    AnalyticsEventSinkSet::CODEX_BACKEND.union(AnalyticsEventSinkSet::LOCAL);

pub(crate) enum AnalyticsEvent {
    CodexAnalytics(TrackEventRequest),
}

impl AnalyticsEvent {
    fn writable_sinks(&self) -> AnalyticsEventSinkSet {
        match self {
            Self::CodexAnalytics(_) => CODEX_ANALYTICS_EVENT_SINKS,
        }
    }
}

impl From<TrackEventRequest> for AnalyticsEvent {
    fn from(event: TrackEventRequest) -> Self {
        Self::CodexAnalytics(event)
    }
}

pub(crate) enum AnalyticsEventSink {
    CodexBackend {
        auth_manager: Arc<AuthManager>,
        base_url: String,
    },
    Local(SharedLocalAnalyticsSink),
}

impl AnalyticsEventSink {
    pub(crate) async fn write(&self, events: &[AnalyticsEvent]) {
        match self {
            Self::CodexBackend {
                auth_manager,
                base_url,
            } => {
                let events = writable_events(events, self.kind())
                    .map(|event| match event {
                        AnalyticsEvent::CodexAnalytics(event) => event,
                    })
                    .collect::<Vec<_>>();
                codex_backend::write(auth_manager, base_url, &events).await;
            }
            Self::Local(sink) => {
                for event in writable_events(events, self.kind()) {
                    match event {
                        AnalyticsEvent::CodexAnalytics(event) => {
                            local::append_codex_analytics_event_best_effort(sink, event);
                        }
                    }
                }
            }
        }
    }

    fn kind(&self) -> AnalyticsEventSinkKind {
        match self {
            Self::CodexBackend { .. } => AnalyticsEventSinkKind::CodexBackend,
            Self::Local(_) => AnalyticsEventSinkKind::Local,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum AnalyticsEventSinkKind {
    CodexBackend,
    Local,
}

impl AnalyticsEventSinkKind {
    const fn flag(self) -> u8 {
        match self {
            Self::CodexBackend => 1 << 0,
            Self::Local => 1 << 1,
        }
    }
}

#[derive(Clone, Copy)]
struct AnalyticsEventSinkSet(u8);

impl AnalyticsEventSinkSet {
    const CODEX_BACKEND: Self = Self(AnalyticsEventSinkKind::CodexBackend.flag());
    const LOCAL: Self = Self(AnalyticsEventSinkKind::Local.flag());

    const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    fn contains(self, sink: AnalyticsEventSinkKind) -> bool {
        self.0 & sink.flag() != 0
    }
}

fn writable_events(
    events: &[AnalyticsEvent],
    sink: AnalyticsEventSinkKind,
) -> impl Iterator<Item = &AnalyticsEvent> {
    events
        .iter()
        .filter(move |event| event.writable_sinks().contains(sink))
}
