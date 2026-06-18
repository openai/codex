use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use anyhow::anyhow;
use chrono::DateTime;
use chrono::Utc;
use codex_features::CurrentTimeSource;
use codex_protocol::ThreadId;

use crate::config::CurrentTimeReminderConfig;

pub type TimeFuture<'a> = Pin<Box<dyn Future<Output = Result<DateTime<Utc>>> + Send + 'a>>;

/// Host integration boundary for obtaining the current time.
pub trait TimeProvider: Send + Sync {
    fn current_time(&self, thread_id: ThreadId) -> TimeFuture<'_>;
}

pub(crate) struct SystemTimeProvider;

impl TimeProvider for SystemTimeProvider {
    fn current_time(&self, _thread_id: ThreadId) -> TimeFuture<'_> {
        Box::pin(async { Ok(Utc::now()) })
    }
}

pub(crate) fn resolve_time_provider(
    config: Option<&CurrentTimeReminderConfig>,
    external_provider: Option<Arc<dyn TimeProvider>>,
) -> Result<Arc<dyn TimeProvider>> {
    match config.map(|config| config.clock_source).unwrap_or_default() {
        CurrentTimeSource::System => Ok(Arc::new(SystemTimeProvider)),
        CurrentTimeSource::External => external_provider.ok_or_else(|| {
            anyhow!(
                "features.current_time_reminder.clock_source is external, but no external current-time provider is available"
            )
        }),
    }
}
