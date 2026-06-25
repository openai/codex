use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use anyhow::anyhow;
use chrono::DateTime;
use chrono::Utc;
use codex_features::CurrentTimeSource;
use codex_protocol::ThreadId;

use crate::config::CurrentTimeReminderConfig;

pub type TimeFuture<'a> = Pin<Box<dyn Future<Output = Result<DateTime<Utc>>> + Send + 'a>>;
pub type SleepFuture<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

/// Host integration boundary for reading and waiting on the current time.
pub trait TimeProvider: Send + Sync {
    fn current_time(&self, thread_id: ThreadId) -> TimeFuture<'_>;

    /// Waits for the given duration on this provider's clock.
    ///
    /// Dropping the returned future cancels the wait.
    fn sleep(&self, thread_id: ThreadId, duration: Duration) -> SleepFuture<'_>;
}

pub(crate) struct SystemTimeProvider;

impl TimeProvider for SystemTimeProvider {
    fn current_time(&self, _thread_id: ThreadId) -> TimeFuture<'_> {
        Box::pin(async { Ok(Utc::now()) })
    }

    fn sleep(&self, _thread_id: ThreadId, duration: Duration) -> SleepFuture<'_> {
        Box::pin(async move {
            tokio::time::sleep(duration).await;
            Ok(())
        })
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
