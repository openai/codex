use crate::metrics::MetricEvent;
use crate::metrics::MetricsConfig;
use crate::metrics::config::MetricsExporter;
use crate::metrics::sink::in_memory::InMemoryExporter;
use crate::metrics::sink::statsig::StatsigExporter;
use std::pin::Pin;

pub(crate) mod in_memory;
pub(crate) mod statsig;

pub(crate) trait MetricSink: Send {
    fn export_batch<'a>(
        &'a mut self,
        events: Vec<MetricEvent>,
    ) -> Pin<Box<dyn Future<Output = crate::metrics::Result<()>> + Send + 'a>>;
    fn shutdown<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = crate::metrics::Result<()>> + Send + 'a>>;
}

pub(crate) fn build_metric_sink(
    config: &MetricsConfig,
) -> crate::metrics::Result<Box<dyn MetricSink>> {
    match &config.exporter {
        MetricsExporter::StatsigHttp {
            endpoint,
            api_key_header,
            timeout,
            user_agent,
        } => Ok(Box::new(StatsigExporter::from(
            endpoint,
            api_key_header,
            timeout,
            user_agent,
            &config.api_key,
        )?)),
        MetricsExporter::InMemory(exporter) => {
            Ok(Box::new(InMemoryExporter::from(exporter.clone())))
        }
    }
}
