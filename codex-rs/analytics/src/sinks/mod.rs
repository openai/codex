mod codex_backend;
mod local;

use crate::events::AnalyticsEvent;
use crate::events::AnalyticsEventSinkKind;
use codex_login::AuthManager;
use std::sync::Arc;

pub use local::LOCAL_ANALYTICS_SCHEMA_VERSION;
pub use local::LocalAnalyticsRecord;
pub use local::LocalAnalyticsRecordType;
pub(crate) use local::SharedLocalAnalyticsSink;
#[cfg(test)]
pub(crate) use local::local_analytics_sink_for_path;
pub(crate) use local::local_analytics_sink_from_env;

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
                let sink = self.kind();
                let events = events
                    .iter()
                    .filter(|event| event.is_writable_to(sink))
                    .map(|event| match event {
                        AnalyticsEvent::CodexAnalytics(event) => event,
                    })
                    .collect::<Vec<_>>();
                codex_backend::write(auth_manager, base_url, &events).await;
            }
            Self::Local(sink) => {
                let sink_kind = self.kind();
                for event in events {
                    if event.is_writable_to(sink_kind) {
                        local::write(sink, event);
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
