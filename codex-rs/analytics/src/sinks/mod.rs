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

pub(crate) enum AnalyticsEventSink {
    CodexBackend {
        auth_manager: Arc<AuthManager>,
        base_url: String,
    },
    Local(SharedLocalAnalyticsSink),
}

impl AnalyticsEventSink {
    pub(crate) async fn write(&self, events: &[TrackEventRequest]) {
        match self {
            Self::CodexBackend {
                auth_manager,
                base_url,
            } => codex_backend::write(auth_manager, base_url, events).await,
            Self::Local(sink) => local::write(sink, events),
        }
    }
}
