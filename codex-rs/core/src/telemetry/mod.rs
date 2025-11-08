//! Telemetry module for Blueprint Mode
//!
//! Privacy-respecting telemetry collection and storage.

pub mod collector;
pub mod events;
pub mod storage;

pub use collector::CollectorConfig;
pub use collector::TelemetryCollector;
pub use events::EventType;
pub use events::TelemetryEvent;
pub use events::hash_id;
pub use events::sanitize_url;
pub use storage::TelemetryStorage;

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;

/// Global telemetry instance (lazy-initialized)
static TELEMETRY: once_cell::sync::OnceCell<Arc<TelemetryCollector>> =
    once_cell::sync::OnceCell::new();

/// Initialize telemetry with default configuration
pub fn init() -> Result<()> {
    let config = CollectorConfig::default();
    let storage_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".codex")
        .join("telemetry");

    init_with_config(config, storage_dir)
}

/// Initialize telemetry with custom configuration
pub fn init_with_config(config: CollectorConfig, storage_dir: PathBuf) -> Result<()> {
    let storage = Arc::new(TelemetryStorage::new(storage_dir)?);
    let collector = Arc::new(TelemetryCollector::new(config, storage));

    // Start background task
    let collector_clone = Arc::clone(&collector);
    tokio::spawn(async move {
        collector_clone.start().await;
    });

    TELEMETRY
        .set(collector)
        .map_err(|_| anyhow::anyhow!("Telemetry already initialized"))?;

    Ok(())
}

/// Get the global telemetry instance
pub fn instance() -> Option<Arc<TelemetryCollector>> {
    TELEMETRY.get().cloned()
}

/// Record a telemetry event (convenience function)
pub async fn record(event: TelemetryEvent) -> Result<()> {
    if let Some(collector) = instance() {
        collector.record(event).await?;
    }
    Ok(())
}

/// Shutdown telemetry (flush remaining events)
pub async fn shutdown() {
    if let Some(collector) = instance() {
        collector.shutdown().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = CollectorConfig::default();

        init_with_config(config, temp_dir.path().to_path_buf()).unwrap();

        assert!(instance().is_some());
    }
}
