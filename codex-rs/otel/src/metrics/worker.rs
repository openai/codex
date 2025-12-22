use crate::metrics::exporter::MetricEvent;
use crate::metrics::exporter::WorkerExporter;
use crate::metrics::util::error_or_panic;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;

pub(crate) fn spawn_worker(
    runtime: Runtime,
    exporter: WorkerExporter,
    exporter_label: String,
    receiver: mpsc::Receiver<MetricEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let worker = MetricsWorker::new(exporter, exporter_label);
        runtime.block_on(worker.run(receiver));
    })
}

struct MetricsWorker {
    exporter: WorkerExporter,
    exporter_label: String,
}

impl MetricsWorker {
    fn new(exporter: WorkerExporter, exporter_label: String) -> Self {
        Self {
            exporter,
            exporter_label,
        }
    }

    async fn run(mut self, mut receiver: mpsc::Receiver<MetricEvent>) {
        while let Some(event) = receiver.recv().await {
            let events = Self::collect_batch(event, &mut receiver).await;
            self.export_batch(events).await;
        }
        self.shutdown().await;
    }

    async fn export_batch(&mut self, events: Vec<MetricEvent>) {
        match &mut self.exporter {
            WorkerExporter::Statsig(exporter) => {
                if let Err(err) = exporter.export_events(events).await {
                    error_or_panic(format!(
                        "statsig metrics export failed: {err} (exporter={})",
                        self.exporter_label
                    ));
                }
            }
            WorkerExporter::InMemory(exporter) => {
                exporter.export_events(events, &self.exporter_label).await;
            }
        }
    }

    async fn collect_batch(
        first: MetricEvent,
        receiver: &mut mpsc::Receiver<MetricEvent>,
    ) -> Vec<MetricEvent> {
        let mut events = Vec::with_capacity(1);
        events.push(first);

        // Fast-path: drain anything already enqueued.
        while events.len() < 50 {
            match receiver.try_recv() {
                Ok(event) => events.push(event),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return events,
            }
        }

        if events.len() >= 50 {
            return events;
        }

        // Small coalescing window to catch near-simultaneous metrics without blocking callers.
        let deadline = Instant::now() + Duration::from_millis(1000);
        while events.len() < 50 {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }

            match tokio::time::timeout(remaining, receiver.recv()).await {
                Ok(Some(event)) => events.push(event),
                Ok(None) => break,
                Err(_) => break,
            }
        }

        events
    }

    async fn shutdown(&mut self) {
        if let WorkerExporter::InMemory(exporter) = &mut self.exporter {
            exporter.shutdown(&self.exporter_label).await;
        }
    }
}
