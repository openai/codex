use crate::error::{OTelSdkError, OTelSdkResult};
use crate::metrics::Temporality;
use crate::metrics::{
    data::ResourceMetrics, instrument::InstrumentKind, pipeline::Pipeline, reader::MetricReader,
};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct TestMetricReader {
    is_shutdown: Arc<Mutex<bool>>,
}

impl TestMetricReader {
    // Constructor to initialize the TestMetricReader
    pub fn new() -> Self {
        TestMetricReader {
            is_shutdown: Arc::new(Mutex::new(false)),
        }
    }

    // Method to check if the reader is shutdown
    pub fn is_shutdown(&self) -> bool {
        *self.is_shutdown.lock().unwrap()
    }
}

impl Default for TestMetricReader {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricReader for TestMetricReader {
    fn register_pipeline(&self, _pipeline: Weak<Pipeline>) {}

    fn collect(&self, _rm: &mut ResourceMetrics) -> OTelSdkResult {
        Ok(())
    }

    fn force_flush(&self) -> OTelSdkResult {
        Ok(())
    }

    fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
        let result = self.force_flush();
        {
            let mut is_shutdown = self.is_shutdown.lock().unwrap();
            *is_shutdown = true;
        }
        result.map_err(|e| OTelSdkError::InternalFailure(e.to_string()))
    }

    fn temporality(&self, _kind: InstrumentKind) -> Temporality {
        Temporality::default()
    }
}
