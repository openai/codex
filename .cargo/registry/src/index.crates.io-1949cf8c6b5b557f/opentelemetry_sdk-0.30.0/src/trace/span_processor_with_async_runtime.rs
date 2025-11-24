use crate::error::{OTelSdkError, OTelSdkResult};
use crate::resource::Resource;
use crate::runtime::{to_interval_stream, RuntimeChannel, TrySend};
use crate::trace::BatchConfig;
use crate::trace::Span;
use crate::trace::SpanProcessor;
use crate::trace::{SpanData, SpanExporter};
use futures_channel::oneshot;
use futures_util::pin_mut;
use futures_util::{
    future::{self, BoxFuture, Either},
    select,
    stream::{self, FusedStream, FuturesUnordered},
    StreamExt as _,
};
use opentelemetry::Context;
use opentelemetry::{otel_debug, otel_error, otel_warn};
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// A [`SpanProcessor`] that asynchronously buffers finished spans and reports
/// them at a preconfigured interval.
///
/// Batch span processors need to run a background task to collect and send
/// spans. Different runtimes need different ways to handle the background task.
///
/// Note: Configuring an opentelemetry `Runtime` that's not compatible with the
/// underlying runtime can cause deadlocks (see tokio section).
///
/// ### Use with Tokio
///
/// Tokio currently offers two different schedulers. One is
/// `current_thread_scheduler`, the other is `multiple_thread_scheduler`. Both
/// of them default to use batch span processors to install span exporters.
///
/// Tokio's `current_thread_scheduler` can cause the program to hang forever if
/// blocking work is scheduled with other tasks in the same runtime. To avoid
/// this, be sure to enable the `rt-tokio-current-thread` feature in this crate
/// if you are using that runtime (e.g. users of actix-web), and blocking tasks
/// will then be scheduled on a different thread.
///
/// # Examples
///
/// This processor can be configured with an [`executor`] of your choice to
/// batch and upload spans asynchronously when they end. If you have added a
/// library like [`tokio`], you can pass in their respective
/// `spawn` and `interval` functions to have batching performed in those
/// contexts.
///
/// ```
/// # #[cfg(feature="tokio")]
/// # {
/// use opentelemetry::global;
/// use opentelemetry_sdk::{runtime, testing::trace::NoopSpanExporter, trace};
/// use opentelemetry_sdk::trace::BatchConfigBuilder;
/// use std::time::Duration;
/// use opentelemetry_sdk::trace::span_processor_with_async_runtime::BatchSpanProcessor;
///
/// #[tokio::main]
/// async fn main() {
///     // Configure your preferred exporter
///     let exporter = NoopSpanExporter::new();
///
///     // Create a batch span processor using an exporter and a runtime
///     let batch = BatchSpanProcessor::builder(exporter, runtime::Tokio)
///         .with_batch_config(BatchConfigBuilder::default().with_max_queue_size(4096).build())
///         .build();
///
///     // Then use the `with_batch_exporter` method to have the provider export spans in batches.
///     let provider = trace::SdkTracerProvider::builder()
///         .with_span_processor(batch)
///         .build();
///
///     let _ = global::set_tracer_provider(provider);
/// }
/// # }
/// ```
///
/// [`executor`]: https://docs.rs/futures/0.3/futures/executor/index.html
/// [`tokio`]: https://tokio.rs
pub struct BatchSpanProcessor<R: RuntimeChannel> {
    message_sender: R::Sender<BatchMessage>,

    // Track dropped spans
    dropped_spans_count: AtomicUsize,

    // Track the maximum queue size that was configured for this processor
    max_queue_size: usize,
}

impl<R: RuntimeChannel> fmt::Debug for BatchSpanProcessor<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BatchSpanProcessor")
            .field("message_sender", &self.message_sender)
            .finish()
    }
}

impl<R: RuntimeChannel> SpanProcessor for BatchSpanProcessor<R> {
    fn on_start(&self, _span: &mut Span, _cx: &Context) {
        // Ignored
    }

    fn on_end(&self, span: SpanData) {
        if !span.span_context.is_sampled() {
            return;
        }

        let result = self.message_sender.try_send(BatchMessage::ExportSpan(span));

        // If the queue is full, and we can't buffer a span
        if result.is_err() {
            // Increment the number of dropped spans. If this is the first time we've had to drop,
            // emit a warning.
            if self.dropped_spans_count.fetch_add(1, Ordering::Relaxed) == 0 {
                otel_warn!(name: "BatchSpanProcessor.SpanDroppingStarted",
                    message = "Beginning to drop span messages due to full/internal errors. No further log will be emitted for further drops until Shutdown. During Shutdown time, a log will be emitted with exact count of total spans dropped.");
            }
        }
    }

    fn force_flush(&self) -> OTelSdkResult {
        let (res_sender, res_receiver) = oneshot::channel();
        self.message_sender
            .try_send(BatchMessage::Flush(Some(res_sender)))
            .map_err(|err| {
                OTelSdkError::InternalFailure(format!("Failed to send flush message: {}", err))
            })?;

        futures_executor::block_on(res_receiver).map_err(|err| {
            OTelSdkError::InternalFailure(format!("Flush response channel error: {}", err))
        })?
    }

    fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
        let dropped_spans = self.dropped_spans_count.load(Ordering::Relaxed);
        let max_queue_size = self.max_queue_size;
        if dropped_spans > 0 {
            otel_warn!(
                name: "BatchSpanProcessor.Shutdown",
                dropped_spans = dropped_spans,
                max_queue_size = max_queue_size,
                message = "Spans were dropped due to a full or closed queue. The count represents the total count of span records dropped in the lifetime of the BatchSpanProcessor. Consider increasing the queue size and/or decrease delay between intervals."
            );
        }

        let (res_sender, res_receiver) = oneshot::channel();
        self.message_sender
            .try_send(BatchMessage::Shutdown(res_sender))
            .map_err(|err| {
                OTelSdkError::InternalFailure(format!("Failed to send shutdown message: {}", err))
            })?;

        futures_executor::block_on(res_receiver).map_err(|err| {
            OTelSdkError::InternalFailure(format!("Shutdown response channel error: {}", err))
        })?
    }

    fn set_resource(&mut self, resource: &Resource) {
        let resource = Arc::new(resource.clone());
        let _ = self
            .message_sender
            .try_send(BatchMessage::SetResource(resource));
    }
}

/// Messages sent between application thread and batch span processor's work thread.
// In this enum the size difference is not a concern because:
// 1. If we wrap SpanData into a pointer, it will add overhead when processing.
// 2. Most of the messages will be ExportSpan.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
enum BatchMessage {
    /// Export spans, usually called when span ends
    ExportSpan(SpanData),
    /// Flush the current buffer to the backend, it can be triggered by
    /// pre configured interval or a call to `force_push` function.
    Flush(Option<oneshot::Sender<OTelSdkResult>>),
    /// Shut down the worker thread, push all spans in buffer to the backend.
    Shutdown(oneshot::Sender<OTelSdkResult>),
    /// Set the resource for the exporter.
    SetResource(Arc<Resource>),
}

struct BatchSpanProcessorInternal<E, R> {
    spans: Vec<SpanData>,
    export_tasks: FuturesUnordered<BoxFuture<'static, OTelSdkResult>>,
    runtime: R,
    exporter: E,
    config: BatchConfig,
}

impl<E: SpanExporter, R: RuntimeChannel> BatchSpanProcessorInternal<E, R> {
    async fn flush(&mut self, res_channel: Option<oneshot::Sender<OTelSdkResult>>) {
        let export_result = self.export().await;
        let task = Box::pin(async move {
            if let Some(channel) = res_channel {
                // If a response channel is provided, attempt to send the export result through it.
                if let Err(result) = channel.send(export_result) {
                    otel_debug!(
                        name: "BatchSpanProcessor.Flush.SendResultError",
                        reason = format!("{:?}", result)
                    );
                }
            } else if let Err(err) = export_result {
                // If no channel is provided and the export operation encountered an error,
                // log the error directly here.
                // TODO: Consider returning the status instead of logging it.
                otel_error!(
                    name: "BatchSpanProcessor.Flush.ExportError",
                    reason = format!("{:?}", err),
                    message = "Failed during the export process"
                );
            }

            Ok(())
        });

        if self.config.max_concurrent_exports == 1 {
            let _ = task.await;
        } else {
            self.export_tasks.push(task);
            while self.export_tasks.next().await.is_some() {}
        }
    }

    /// Process a single message
    ///
    /// A return value of false indicates shutdown
    async fn process_message(&mut self, message: BatchMessage) -> bool {
        match message {
            // Span has finished, add to buffer of pending spans.
            BatchMessage::ExportSpan(span) => {
                self.spans.push(span);

                if self.spans.len() == self.config.max_export_batch_size {
                    // If concurrent exports are saturated, wait for one to complete.
                    if !self.export_tasks.is_empty()
                        && self.export_tasks.len() == self.config.max_concurrent_exports
                    {
                        self.export_tasks.next().await;
                    }

                    let export_result = self.export().await;
                    let task = async move {
                        if let Err(err) = export_result {
                            otel_error!(
                                name: "BatchSpanProcessor.Export.Error",
                                reason = format!("{}", err)
                            );
                        }

                        Ok(())
                    };
                    // Special case when not using concurrent exports
                    if self.config.max_concurrent_exports == 1 {
                        let _ = task.await;
                    } else {
                        self.export_tasks.push(Box::pin(task));
                    }
                }
            }
            // Span batch interval time reached or a force flush has been invoked, export
            // current spans.
            //
            // This is a hint to ensure that any tasks associated with Spans for which the
            // SpanProcessor had already received events prior to the call to ForceFlush
            // SHOULD be completed as soon as possible, preferably before returning from
            // this method.
            //
            // In particular, if any SpanProcessor has any associated exporter, it SHOULD
            // try to call the exporter's Export with all spans for which this was not
            // already done and then invoke ForceFlush on it. The built-in SpanProcessors
            // MUST do so. If a timeout is specified (see below), the SpanProcessor MUST
            // prioritize honoring the timeout over finishing all calls. It MAY skip or
            // abort some or all Export or ForceFlush calls it has made to achieve this
            // goal.
            //
            // NB: `force_flush` is not currently implemented on exporters; the equivalent
            // would be waiting for exporter tasks to complete. In the case of
            // channel-coupled exporters, they will need a `force_flush` implementation to
            // properly block.
            BatchMessage::Flush(res_channel) => {
                self.flush(res_channel).await;
            }
            // Stream has terminated or processor is shutdown, return to finish execution.
            BatchMessage::Shutdown(ch) => {
                self.flush(Some(ch)).await;
                let _ = self.exporter.shutdown();
                return false;
            }
            // propagate the resource
            BatchMessage::SetResource(resource) => {
                self.exporter.set_resource(&resource);
            }
        }
        true
    }

    async fn export(&mut self) -> OTelSdkResult {
        // Batch size check for flush / shutdown. Those methods may be called
        // when there's no work to do.
        if self.spans.is_empty() {
            return Ok(());
        }

        let export = self.exporter.export(self.spans.split_off(0));
        let timeout = self.runtime.delay(self.config.max_export_timeout);
        let time_out = self.config.max_export_timeout;

        pin_mut!(export);
        pin_mut!(timeout);

        match future::select(export, timeout).await {
            Either::Left((export_res, _)) => export_res,
            Either::Right((_, _)) => Err(OTelSdkError::Timeout(time_out)),
        }
    }

    async fn run(mut self, mut messages: impl FusedStream<Item = BatchMessage> + Unpin) {
        loop {
            select! {
                // FuturesUnordered implements Fuse intelligently such that it
                // will become eligible again once new tasks are added to it.
                _ = self.export_tasks.next() => {
                    // An export task completed; do we need to do anything with it?
                },
                message = messages.next() => {
                    match message {
                        Some(message) => {
                            if !self.process_message(message).await {
                                break;
                            }
                        },
                        None => break,
                    }
                },
            }
        }
    }
}

impl<R: RuntimeChannel> BatchSpanProcessor<R> {
    pub(crate) fn new<E>(exporter: E, config: BatchConfig, runtime: R) -> Self
    where
        E: SpanExporter + Send + Sync + 'static,
    {
        let (message_sender, message_receiver) =
            runtime.batch_message_channel(config.max_queue_size);

        let max_queue_size = config.max_queue_size;

        let inner_runtime = runtime.clone();
        // Spawn worker process via user-defined spawn function.
        runtime.spawn(async move {
            // Timer will take a reference to the current runtime, so its important we do this within the
            // runtime.spawn()
            let ticker = to_interval_stream(inner_runtime.clone(), config.scheduled_delay)
                .skip(1) // The ticker is fired immediately, so we should skip the first one to align with the interval.
                .map(|_| BatchMessage::Flush(None));
            let timeout_runtime = inner_runtime.clone();

            let messages = Box::pin(stream::select(message_receiver, ticker));
            let processor = BatchSpanProcessorInternal {
                spans: Vec::new(),
                export_tasks: FuturesUnordered::new(),
                runtime: timeout_runtime,
                config,
                exporter,
            };

            processor.run(messages).await
        });

        // Return batch processor with link to worker
        BatchSpanProcessor {
            message_sender,
            dropped_spans_count: AtomicUsize::new(0),
            max_queue_size,
        }
    }

    /// Create a new batch processor builder
    pub fn builder<E>(exporter: E, runtime: R) -> BatchSpanProcessorBuilder<E, R>
    where
        E: SpanExporter,
    {
        BatchSpanProcessorBuilder {
            exporter,
            config: Default::default(),
            runtime,
        }
    }
}

/// A builder for creating [`BatchSpanProcessor`] instances.
///
#[derive(Debug)]
pub struct BatchSpanProcessorBuilder<E, R> {
    exporter: E,
    config: BatchConfig,
    runtime: R,
}

impl<E, R> BatchSpanProcessorBuilder<E, R>
where
    E: SpanExporter + 'static,
    R: RuntimeChannel,
{
    /// Set the BatchConfig for [BatchSpanProcessorBuilder]
    pub fn with_batch_config(self, config: BatchConfig) -> Self {
        BatchSpanProcessorBuilder { config, ..self }
    }

    /// Build a batch processor
    pub fn build(self) -> BatchSpanProcessor<R> {
        BatchSpanProcessor::new(self.exporter, self.config, self.runtime)
    }
}

#[cfg(all(test, feature = "testing", feature = "trace"))]
mod tests {
    // cargo test trace::span_processor::tests:: --features=testing
    use super::{BatchSpanProcessor, SpanProcessor};
    use crate::error::OTelSdkResult;
    use crate::runtime;
    use crate::testing::trace::{new_test_export_span_data, new_tokio_test_exporter};
    use crate::trace::span_processor::{
        OTEL_BSP_EXPORT_TIMEOUT, OTEL_BSP_MAX_EXPORT_BATCH_SIZE, OTEL_BSP_MAX_QUEUE_SIZE,
        OTEL_BSP_MAX_QUEUE_SIZE_DEFAULT, OTEL_BSP_SCHEDULE_DELAY, OTEL_BSP_SCHEDULE_DELAY_DEFAULT,
    };
    use crate::trace::{BatchConfig, BatchConfigBuilder, InMemorySpanExporterBuilder};
    use crate::trace::{SpanData, SpanExporter};
    use futures_util::Future;
    use std::fmt::Debug;
    use std::time::Duration;

    struct BlockingExporter<D> {
        delay_for: Duration,
        delay_fn: D,
    }

    impl<D, DS> Debug for BlockingExporter<D>
    where
        D: Fn(Duration) -> DS + 'static + Send + Sync,
        DS: Future<Output = ()> + Send + Sync + 'static,
    {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("blocking exporter for testing")
        }
    }

    impl<D, DS> SpanExporter for BlockingExporter<D>
    where
        D: Fn(Duration) -> DS + 'static + Send + Sync,
        DS: Future<Output = ()> + Send + Sync + 'static,
    {
        async fn export(&self, _batch: Vec<SpanData>) -> OTelSdkResult {
            (self.delay_fn)(self.delay_for).await;
            Ok(())
        }
    }

    #[test]
    fn test_build_batch_span_processor_builder() {
        let mut env_vars = vec![
            (OTEL_BSP_MAX_EXPORT_BATCH_SIZE, Some("500")),
            (OTEL_BSP_SCHEDULE_DELAY, Some("I am not number")),
            (OTEL_BSP_EXPORT_TIMEOUT, Some("2046")),
        ];
        temp_env::with_vars(env_vars.clone(), || {
            let builder = BatchSpanProcessor::builder(
                InMemorySpanExporterBuilder::new().build(),
                runtime::Tokio,
            );
            // export batch size cannot exceed max queue size
            assert_eq!(builder.config.max_export_batch_size, 500);
            assert_eq!(
                builder.config.scheduled_delay,
                OTEL_BSP_SCHEDULE_DELAY_DEFAULT
            );
            assert_eq!(
                builder.config.max_queue_size,
                OTEL_BSP_MAX_QUEUE_SIZE_DEFAULT
            );
            assert_eq!(
                builder.config.max_export_timeout,
                Duration::from_millis(2046)
            );
        });

        env_vars.push((OTEL_BSP_MAX_QUEUE_SIZE, Some("120")));

        temp_env::with_vars(env_vars, || {
            let builder = BatchSpanProcessor::builder(
                InMemorySpanExporterBuilder::new().build(),
                runtime::Tokio,
            );
            assert_eq!(builder.config.max_export_batch_size, 120);
            assert_eq!(builder.config.max_queue_size, 120);
        });
    }

    #[tokio::test]
    async fn test_batch_span_processor() {
        let (exporter, mut export_receiver, _shutdown_receiver) = new_tokio_test_exporter();
        let config = BatchConfigBuilder::default()
            .with_scheduled_delay(Duration::from_secs(60 * 60 * 24)) // set the tick to 24 hours so we know the span must be exported via force_flush
            .build();
        let processor = BatchSpanProcessor::new(exporter, config, runtime::TokioCurrentThread);
        let handle = tokio::spawn(async move {
            loop {
                if let Some(span) = export_receiver.recv().await {
                    assert_eq!(span.span_context, new_test_export_span_data().span_context);
                    break;
                }
            }
        });
        tokio::time::sleep(Duration::from_secs(1)).await; // skip the first
        processor.on_end(new_test_export_span_data());
        let flush_res = processor.force_flush();
        assert!(flush_res.is_ok());
        let _shutdown_result = processor.shutdown();

        assert!(
            tokio::time::timeout(Duration::from_secs(5), handle)
                .await
                .is_ok(),
            "timed out in 5 seconds. force_flush may not export any data when called"
        );
    }

    // If the time_out is true, then the result suppose to ended with timeout.
    // otherwise the exporter should be able to export within time out duration.
    async fn timeout_test_tokio(time_out: bool) {
        let config = BatchConfig {
            max_export_timeout: Duration::from_millis(if time_out { 5 } else { 60 }),
            scheduled_delay: Duration::from_secs(60 * 60 * 24), // set the tick to 24 hours so we know the span must be exported via force_flush,
            ..Default::default()
        };
        let exporter = BlockingExporter {
            delay_for: Duration::from_millis(if !time_out { 5 } else { 60 }),
            delay_fn: tokio::time::sleep,
        };
        let processor = BatchSpanProcessor::new(exporter, config, runtime::TokioCurrentThread);
        tokio::time::sleep(Duration::from_secs(1)).await; // skip the first
        processor.on_end(new_test_export_span_data());
        let flush_res = processor.force_flush();
        if time_out {
            assert!(flush_res.is_err());
        } else {
            assert!(flush_res.is_ok());
        }
        let shutdown_res = processor.shutdown();
        assert!(shutdown_res.is_ok());
    }

    #[test]
    fn test_timeout_tokio_timeout() {
        // If time_out is true, then we ask exporter to block for 60s and set timeout to 5s.
        // If time_out is false, then we ask the exporter to block for 5s and set timeout to 60s.
        // Either way, the test should be finished within 5s.
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(timeout_test_tokio(true));
    }

    #[test]
    fn test_timeout_tokio_not_timeout() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(timeout_test_tokio(false));
    }
}
