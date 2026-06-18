use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use anyhow::anyhow;
use chrono::DateTime;
use chrono::Utc;
use codex_features::VarlatencyClockSource;
use codex_protocol::ThreadId;

use crate::config::VarlatencyConfig;

pub type CurrentTimeFuture<'a> = Pin<Box<dyn Future<Output = Result<DateTime<Utc>>> + Send + 'a>>;

/// Host integration boundary for obtaining the current time.
pub trait CurrentTimeProvider: Send + Sync {
    fn current_time(&self, thread_id: ThreadId) -> CurrentTimeFuture<'_>;
}

struct SystemCurrentTimeProvider;

impl CurrentTimeProvider for SystemCurrentTimeProvider {
    fn current_time(&self, _thread_id: ThreadId) -> CurrentTimeFuture<'_> {
        Box::pin(async { Ok(Utc::now()) })
    }
}

pub(crate) fn resolve_current_time_provider(
    config: Option<&VarlatencyConfig>,
    external_provider: Option<Arc<dyn CurrentTimeProvider>>,
) -> Result<Option<Arc<dyn CurrentTimeProvider>>> {
    let Some(config) = config else {
        return Ok(None);
    };

    match config.clock_source {
        VarlatencyClockSource::System => Ok(Some(Arc::new(SystemCurrentTimeProvider))),
        VarlatencyClockSource::AppServerClient => external_provider.map(Some).ok_or_else(|| {
            anyhow!(
                "features.varlatency.clock_source is app_server_client, but no external current-time provider is available"
            )
        }),
    }
}
