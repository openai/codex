//! Telemetry event collector
//!
//! Collects and buffers telemetry events before persistence.

use super::events::TelemetryEvent;
use super::storage::TelemetryStorage;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tracing::debug;
use tracing::error;

/// Telemetry collector configuration
#[derive(Debug, Clone)]
pub struct CollectorConfig {
    /// Buffer size for events
    pub buffer_size: usize,

    /// Flush interval in seconds
    pub flush_interval_secs: u64,

    /// Enable telemetry collection
    pub enabled: bool,
}

impl Default for CollectorConfig {
    fn default() -> Self {
        Self {
            buffer_size: 100,
            flush_interval_secs: 60,
            enabled: true,
        }
    }
}

/// Telemetry collector
pub struct TelemetryCollector {
    config: CollectorConfig,
    storage: Arc<TelemetryStorage>,
    event_tx: mpsc::Sender<TelemetryEvent>,
    event_rx: Arc<RwLock<mpsc::Receiver<TelemetryEvent>>>,
}

impl TelemetryCollector {
    /// Create a new telemetry collector
    pub fn new(config: CollectorConfig, storage: Arc<TelemetryStorage>) -> Self {
        let (event_tx, event_rx) = mpsc::channel(config.buffer_size);

        Self {
            config,
            storage,
            event_tx,
            event_rx: Arc::new(RwLock::new(event_rx)),
        }
    }

    /// Record an event
    pub async fn record(&self, event: TelemetryEvent) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        debug!("Recording telemetry event: {:?}", event.event_type);

        if let Err(e) = self.event_tx.send(event).await {
            error!("Failed to send telemetry event: {}", e);
        }

        Ok(())
    }

    /// Start the collector background task
    pub async fn start(self: Arc<Self>) {
        let collector = Arc::clone(&self);

        tokio::spawn(async move {
            collector.run().await;
        });
    }

    /// Run the collector loop
    async fn run(&self) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(
            self.config.flush_interval_secs,
        ));

        let mut buffer = Vec::new();

        loop {
            tokio::select! {
                // Check for new events
                event = async {
                    let mut rx = self.event_rx.write().await;
                    rx.recv().await
                } => {
                    if let Some(event) = event {
                        buffer.push(event);

                        // Flush if buffer is full
                        if buffer.len() >= self.config.buffer_size {
                            self.flush_buffer(&mut buffer).await;
                        }
                    }
                }

                // Periodic flush
                _ = interval.tick() => {
                    if !buffer.is_empty() {
                        self.flush_buffer(&mut buffer).await;
                    }
                }
            }
        }
    }

    /// Flush buffer to storage
    async fn flush_buffer(&self, buffer: &mut Vec<TelemetryEvent>) {
        if buffer.is_empty() {
            return;
        }

        debug!("Flushing {} telemetry events", buffer.len());

        for event in buffer.drain(..) {
            if let Err(e) = self.storage.store(&event).await {
                error!("Failed to store telemetry event: {}", e);
            }
        }
    }

    /// Shutdown the collector (flush remaining events)
    pub async fn shutdown(&self) {
        let mut rx = self.event_rx.write().await;
        let mut buffer = Vec::new();

        // Drain remaining events
        while let Ok(event) = rx.try_recv() {
            buffer.push(event);
        }

        // Flush
        drop(rx);
        if !buffer.is_empty() {
            for event in buffer {
                if let Err(e) = self.storage.store(&event).await {
                    error!("Failed to store telemetry event during shutdown: {}", e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::events::EventType;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_collector_creation() {
        let config = CollectorConfig::default();
        let storage = Arc::new(TelemetryStorage::new(PathBuf::from("/tmp/telemetry")).unwrap());
        let collector = TelemetryCollector::new(config, storage);

        assert!(collector.config.enabled);
        assert_eq!(collector.config.buffer_size, 100);
    }

    #[tokio::test]
    async fn test_record_event() {
        let config = CollectorConfig::default();
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = Arc::new(TelemetryStorage::new(temp_dir.path().to_path_buf()).unwrap());
        let collector = Arc::new(TelemetryCollector::new(config, storage));

        let event = TelemetryEvent::new(EventType::BlueprintStart).with_session_id("test-session");

        collector.record(event).await.unwrap();
    }

    #[tokio::test]
    async fn test_disabled_collector() {
        let config = CollectorConfig {
            enabled: false,
            ..Default::default()
        };
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = Arc::new(TelemetryStorage::new(temp_dir.path().to_path_buf()).unwrap());
        let collector = Arc::new(TelemetryCollector::new(config, storage));

        let event = TelemetryEvent::new(EventType::BlueprintStart);

        // Should succeed but not record
        collector.record(event).await.unwrap();
    }
}
