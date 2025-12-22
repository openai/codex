use crate::metrics::MetricEvent;
use crate::metrics::sink::MetricSink;
use crate::metrics::util::error_or_panic;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;

const MAX_BATCH_SIZE: usize = 50;
const BATCH_TIMEOUT: Duration = Duration::from_millis(1000);

pub(crate) fn spawn_worker(
    runtime: Runtime,
    exporter: Box<dyn MetricSink>,
    exporter_label: String,
    receiver: mpsc::Receiver<MetricEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let worker = MetricsWorker::new(exporter, exporter_label);
        runtime.block_on(worker.run(receiver));
    })
}

struct MetricsWorker {
    exporter: Box<dyn MetricSink>,
    exporter_label: String,
}

impl MetricsWorker {
    fn new(exporter: Box<dyn MetricSink>, exporter_label: String) -> Self {
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
        if let Err(err) = self.exporter.export_batch(events).await {
            error_or_panic(format!(
                "metrics export failed: {err} (exporter={})",
                self.exporter_label
            ));
        }
    }

    async fn collect_batch(
        first: MetricEvent,
        receiver: &mut mpsc::Receiver<MetricEvent>,
    ) -> Vec<MetricEvent> {
        let mut events = Vec::with_capacity(1);
        events.push(first);

        while events.len() < MAX_BATCH_SIZE {
            match receiver.try_recv() {
                Ok(event) => events.push(event),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return events,
            }
        }

        if events.len() >= MAX_BATCH_SIZE {
            return events;
        }

        let deadline = Instant::now() + BATCH_TIMEOUT;
        while events.len() < MAX_BATCH_SIZE {
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
        if let Err(err) = self.exporter.shutdown().await {
            error_or_panic(format!(
                "metrics shutdown failed: {err} (exporter={})",
                self.exporter_label
            ));
        }
    }
}
