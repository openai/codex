use crate::events::TrackEventRequest;
use crate::events::TrackEventsRequest;
use crate::local_sink::SharedLocalAnalyticsSink;
use crate::local_sink::append_codex_analytics_event_best_effort;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::default_client::create_client;
use std::sync::Arc;
use std::time::Duration;

const ANALYTICS_EVENTS_TIMEOUT: Duration = Duration::from_secs(10);
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
                send_track_events(auth_manager, base_url, &events).await;
            }
            Self::Local(sink) => {
                for event in writable_events(events, self.kind()) {
                    match event {
                        AnalyticsEvent::CodexAnalytics(event) => {
                            append_codex_analytics_event_best_effort(sink, event);
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

async fn send_track_events(
    auth_manager: &AuthManager,
    base_url: &str,
    events: &[&TrackEventRequest],
) {
    if events.is_empty() {
        return;
    }

    let Some(auth) = auth_manager.auth().await else {
        return;
    };
    if !auth.uses_codex_backend() {
        return;
    }

    let base_url = base_url.trim_end_matches('/');
    let url = format!("{base_url}/codex/analytics-events/events");
    for events in track_event_request_batches(events) {
        send_track_events_request(&auth, &url, events).await;
    }
}

fn track_event_request_batches<'a>(
    events: &'a [&'a TrackEventRequest],
) -> Vec<&'a [&'a TrackEventRequest]> {
    let mut batches = Vec::new();
    let mut current_batch_start = 0;

    for (index, event) in events.iter().enumerate() {
        if event.should_send_in_isolated_request() {
            if current_batch_start < index {
                batches.push(&events[current_batch_start..index]);
            }
            batches.push(&events[index..=index]);
            current_batch_start = index + 1;
        }
    }

    if current_batch_start < events.len() {
        batches.push(&events[current_batch_start..]);
    }

    batches
}

async fn send_track_events_request(auth: &CodexAuth, url: &str, events: &[&TrackEventRequest]) {
    if events.is_empty() {
        return;
    }

    let payload = TrackEventsRequest { events };

    let response = create_client()
        .post(url)
        .timeout(ANALYTICS_EVENTS_TIMEOUT)
        .headers(codex_model_provider::auth_provider_from_auth(auth).to_auth_headers())
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await;

    match response {
        Ok(response) if response.status().is_success() => {}
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::warn!("events failed with status {status}: {body}");
        }
        Err(err) => {
            tracing::warn!("failed to send events request: {err}");
        }
    }
}

#[cfg(test)]
#[path = "event_sink_tests.rs"]
mod tests;
