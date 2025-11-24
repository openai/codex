use super::IdGenerator;
use crate::error::{OTelSdkError, OTelSdkResult};
/// # Trace Provider SDK
///
/// The `TracerProvider` handles the creation and management of [`Tracer`] instances and coordinates
/// span processing. It serves as the central configuration point for tracing, ensuring consistency
/// across all [`Tracer`] instances it creates.
///
/// ## Tracer Creation
///
/// New [`Tracer`] instances are always created through a `TracerProvider`. These `Tracer`s share
/// a common configuration, which includes the [`Resource`], span processors, sampling strategies,
/// and span limits. This avoids the need for each `Tracer` to maintain its own version of these
/// configurations, ensuring uniform behavior across all instances.
///
/// ## Cloning and Shutdown
///
/// The `TracerProvider` is designed to be clonable. Cloning a `TracerProvider`  creates a
/// new reference to the same provider, not a new instance. Dropping the last reference
/// to the `TracerProvider` will automatically trigger its shutdown. During shutdown, the provider
/// will flush all remaining spans, ensuring they are passed to the configured processors.
/// Users can also manually trigger shutdown using the [`shutdown`](TracerProvider::shutdown)
/// method, which will ensure the same behavior.
///
/// Once shut down, the `TracerProvider` transitions into a disabled state. In this state, further
/// operations on its associated `Tracer` instances will result in no-ops, ensuring that no spans
/// are processed or exported after shutdown.
///
/// ## Span Processing and Force Flush
///
/// The `TracerProvider` manages the lifecycle of span processors, which are responsible for
/// collecting, processing, and exporting spans. The [`force_flush`](TracerProvider::force_flush) method
/// invoked at any time will trigger an immediate flush of all pending spans (if any) to the exporters.
/// This will block the user thread till all the spans are passed to exporters.
///
/// # Examples
///
/// ```
/// use opentelemetry::global;
/// use opentelemetry_sdk::trace::SdkTracerProvider;
/// use opentelemetry::trace::Tracer;
///
/// fn init_tracing() -> SdkTracerProvider {
///     let provider = SdkTracerProvider::default();
///
///     // Set the provider to be used globally
///     let _ = global::set_tracer_provider(provider.clone());
///
///     provider
/// }
///
/// fn main() {
///     let provider = init_tracing();
///
///     // create tracer..
///     let tracer = global::tracer("example/client");
///
///     // create span...
///     let span = tracer
///         .span_builder("test_span")
///         .start(&tracer);
///
///     // Explicitly shut down the provider
///     provider.shutdown();
/// }
/// ```
use crate::trace::{
    BatchSpanProcessor, Config, RandomIdGenerator, Sampler, SdkTracer, SimpleSpanProcessor,
    SpanLimits,
};
use crate::Resource;
use crate::{trace::SpanExporter, trace::SpanProcessor};
use opentelemetry::otel_debug;
use opentelemetry::{otel_info, InstrumentationScope};
use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

static PROVIDER_RESOURCE: OnceLock<Resource> = OnceLock::new();

// a no nop tracer provider used as placeholder when the provider is shutdown
// TODO Replace with LazyLock once it is stable
static NOOP_TRACER_PROVIDER: OnceLock<SdkTracerProvider> = OnceLock::new();
#[inline]
fn noop_tracer_provider() -> &'static SdkTracerProvider {
    NOOP_TRACER_PROVIDER.get_or_init(|| {
        SdkTracerProvider {
            inner: Arc::new(TracerProviderInner {
                processors: Vec::new(),
                config: Config {
                    // cannot use default here as the default resource is not empty
                    sampler: Box::new(Sampler::ParentBased(Box::new(Sampler::AlwaysOn))),
                    id_generator: Box::<RandomIdGenerator>::default(),
                    span_limits: SpanLimits::default(),
                    resource: Cow::Owned(Resource::empty()),
                },
                is_shutdown: AtomicBool::new(true),
            }),
        }
    })
}

/// TracerProvider inner type
#[derive(Debug)]
pub(crate) struct TracerProviderInner {
    processors: Vec<Box<dyn SpanProcessor>>,
    config: crate::trace::Config,
    is_shutdown: AtomicBool,
}

impl TracerProviderInner {
    /// Crate-private shutdown method to be called both from explicit shutdown
    /// and from Drop when the last reference is released.
    pub(crate) fn shutdown_with_timeout(&self, timeout: Duration) -> Vec<OTelSdkResult> {
        let mut results = vec![];
        for processor in &self.processors {
            let result = processor.shutdown_with_timeout(timeout);
            if let Err(err) = &result {
                // Log at debug level because:
                //  - The error is also returned to the user for handling (if applicable)
                //  - Or the error occurs during `TracerProviderInner::Drop` as part of telemetry shutdown,
                //    which is non-actionable by the user
                otel_debug!(name: "TracerProvider.Drop.ShutdownError",
                        error = format!("{err}"));
            }
            results.push(result);
        }
        results
    }
    /// shutdown with default timeout
    pub(crate) fn shutdown(&self) -> Vec<OTelSdkResult> {
        self.shutdown_with_timeout(Duration::from_secs(5))
    }
}

impl Drop for TracerProviderInner {
    fn drop(&mut self) {
        if !self.is_shutdown.load(Ordering::Relaxed) {
            let _ = self.shutdown(); // errors are handled within shutdown
        } else {
            otel_debug!(
                name: "TracerProvider.Drop.AlreadyShutdown",
                message = "TracerProvider was already shut down; drop will not attempt shutdown again."
            );
        }
    }
}

/// Creator and registry of named [`SdkTracer`] instances.
///
/// `TracerProvider` is a container holding pointers to `SpanProcessor` and other components.
/// Cloning a `TracerProvider` instance and dropping it will not stop span processing. To stop span processing, users
/// must either call the `shutdown` method explicitly or allow the last reference to the `TracerProvider`
/// to be dropped. When the last reference is dropped, the shutdown process will be automatically triggered
/// to ensure proper cleanup.
#[derive(Clone, Debug)]
pub struct SdkTracerProvider {
    inner: Arc<TracerProviderInner>,
}

impl Default for SdkTracerProvider {
    fn default() -> Self {
        SdkTracerProvider::builder().build()
    }
}

impl SdkTracerProvider {
    /// Build a new tracer provider
    pub(crate) fn new(inner: TracerProviderInner) -> Self {
        SdkTracerProvider {
            inner: Arc::new(inner),
        }
    }

    /// Create a new [`SdkTracerProvider`] builder.
    pub fn builder() -> TracerProviderBuilder {
        TracerProviderBuilder::default()
    }

    /// Span processors associated with this provider
    pub(crate) fn span_processors(&self) -> &[Box<dyn SpanProcessor>] {
        &self.inner.processors
    }

    /// Config associated with this tracer
    pub(crate) fn config(&self) -> &crate::trace::Config {
        &self.inner.config
    }

    /// true if the provider has been shutdown
    /// Don't start span or export spans when provider is shutdown
    pub(crate) fn is_shutdown(&self) -> bool {
        self.inner.is_shutdown.load(Ordering::Relaxed)
    }

    /// Force flush all remaining spans in span processors and return results.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::global;
    /// use opentelemetry_sdk::trace::SdkTracerProvider;
    ///
    /// fn init_tracing() -> SdkTracerProvider {
    ///     let provider = SdkTracerProvider::default();
    ///
    ///     // Set provider to be used as global tracer provider
    ///     let _ = global::set_tracer_provider(provider.clone());
    ///
    ///     provider
    /// }
    ///
    /// fn main() {
    ///     let provider = init_tracing();
    ///
    ///     // create spans..
    ///
    ///     // force all spans to flush
    ///     if let Err(err) = provider.force_flush() {
    ///         // .. handle flush error
    ///     }
    ///
    ///     // create more spans..
    ///
    ///     // dropping provider ensures all remaining spans are exported
    ///     drop(provider);
    /// }
    /// ```
    pub fn force_flush(&self) -> OTelSdkResult {
        let result: Vec<_> = self
            .span_processors()
            .iter()
            .map(|processor| processor.force_flush())
            .collect();
        if result.iter().all(|r| r.is_ok()) {
            Ok(())
        } else {
            Err(OTelSdkError::InternalFailure(format!("errs: {:?}", result)))
        }
    }

    /// Shuts down the current `TracerProvider`.
    ///
    /// Note that shut down doesn't means the TracerProvider has dropped
    pub fn shutdown_with_timeout(&self, timeout: Duration) -> OTelSdkResult {
        if self
            .inner
            .is_shutdown
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            // propagate the shutdown signal to processors
            let results = self.inner.shutdown_with_timeout(timeout);

            if results.iter().all(|res| res.is_ok()) {
                Ok(())
            } else {
                Err(OTelSdkError::InternalFailure(format!(
                    "Shutdown errors: {:?}",
                    results
                        .into_iter()
                        .filter_map(Result::err)
                        .collect::<Vec<_>>() // Collect only the errors
                )))
            }
        } else {
            Err(OTelSdkError::AlreadyShutdown)
        }
    }

    /// shutdown with default timeout
    pub fn shutdown(&self) -> OTelSdkResult {
        self.shutdown_with_timeout(Duration::from_secs(5))
    }
}

impl opentelemetry::trace::TracerProvider for SdkTracerProvider {
    /// This implementation of `TracerProvider` produces `Tracer` instances.
    type Tracer = SdkTracer;

    fn tracer(&self, name: impl Into<Cow<'static, str>>) -> Self::Tracer {
        let scope = InstrumentationScope::builder(name).build();
        self.tracer_with_scope(scope)
    }

    fn tracer_with_scope(&self, scope: InstrumentationScope) -> Self::Tracer {
        if self.inner.is_shutdown.load(Ordering::Relaxed) {
            return SdkTracer::new(scope, noop_tracer_provider().clone());
        }
        if scope.name().is_empty() {
            otel_info!(name: "TracerNameEmpty",  message = "Tracer name is empty; consider providing a meaningful name. Tracer will function normally and the provided name will be used as-is.");
        };
        SdkTracer::new(scope, self.clone())
    }
}

/// Builder for provider attributes.
#[derive(Debug, Default)]
pub struct TracerProviderBuilder {
    processors: Vec<Box<dyn SpanProcessor>>,
    config: crate::trace::Config,
    resource: Option<Resource>,
}

impl TracerProviderBuilder {
    /// Adds a [SimpleSpanProcessor] with the configured exporter to the pipeline.
    ///
    /// # Arguments
    ///
    /// * `exporter` - The exporter to be used by the SimpleSpanProcessor.
    ///
    /// # Returns
    ///
    /// A new `Builder` instance with the SimpleSpanProcessor added to the pipeline.
    ///
    /// Processors are invoked in the order they are added.
    pub fn with_simple_exporter<T: SpanExporter + 'static>(self, exporter: T) -> Self {
        let simple = SimpleSpanProcessor::new(exporter);
        self.with_span_processor(simple)
    }

    /// Adds a [BatchSpanProcessor] with the configured exporter to the pipeline.
    ///
    /// # Arguments
    ///
    /// * `exporter` - The exporter to be used by the BatchSpanProcessor.
    ///
    /// # Returns
    ///
    /// A new `Builder` instance with the BatchSpanProcessor added to the pipeline.
    ///
    /// Processors are invoked in the order they are added.
    pub fn with_batch_exporter<T: SpanExporter + 'static>(self, exporter: T) -> Self {
        let batch = BatchSpanProcessor::builder(exporter).build();
        self.with_span_processor(batch)
    }

    /// Adds a custom [SpanProcessor] to the pipeline.
    ///
    /// # Arguments
    ///
    /// * `processor` - The `SpanProcessor` to be added.
    ///
    /// # Returns
    ///
    /// A new `Builder` instance with the custom `SpanProcessor` added to the pipeline.
    ///
    /// Processors are invoked in the order they are added.
    pub fn with_span_processor<T: SpanProcessor + 'static>(self, processor: T) -> Self {
        let mut processors = self.processors;
        processors.push(Box::new(processor));

        TracerProviderBuilder { processors, ..self }
    }

    /// Specify the sampler to be used.
    pub fn with_sampler<T: crate::trace::ShouldSample + 'static>(mut self, sampler: T) -> Self {
        self.config.sampler = Box::new(sampler);
        self
    }

    /// Specify the id generator to be used.
    pub fn with_id_generator<T: IdGenerator + 'static>(mut self, id_generator: T) -> Self {
        self.config.id_generator = Box::new(id_generator);
        self
    }

    /// Specify the number of events to be recorded per span.
    pub fn with_max_events_per_span(mut self, max_events: u32) -> Self {
        self.config.span_limits.max_events_per_span = max_events;
        self
    }

    /// Specify the number of attributes to be recorded per span.
    pub fn with_max_attributes_per_span(mut self, max_attributes: u32) -> Self {
        self.config.span_limits.max_attributes_per_span = max_attributes;
        self
    }

    /// Specify the number of events to be recorded per span.
    pub fn with_max_links_per_span(mut self, max_links: u32) -> Self {
        self.config.span_limits.max_links_per_span = max_links;
        self
    }

    /// Specify the number of attributes one event can have.
    pub fn with_max_attributes_per_event(mut self, max_attributes: u32) -> Self {
        self.config.span_limits.max_attributes_per_event = max_attributes;
        self
    }

    /// Specify the number of attributes one link can have.
    pub fn with_max_attributes_per_link(mut self, max_attributes: u32) -> Self {
        self.config.span_limits.max_attributes_per_link = max_attributes;
        self
    }

    /// Specify all limit via the span_limits
    pub fn with_span_limits(mut self, span_limits: SpanLimits) -> Self {
        self.config.span_limits = span_limits;
        self
    }

    /// Associates a [Resource] with a [SdkTracerProvider].
    ///
    /// This [Resource] represents the entity producing telemetry and is associated
    /// with all [Tracer]s the [SdkTracerProvider] will create.
    ///
    /// By default, if this option is not used, the default [Resource] will be used.
    ///
    /// *Note*: Calls to this method are additive, each call merges the provided
    /// resource with the previous one.
    ///
    /// [Tracer]: opentelemetry::trace::Tracer
    pub fn with_resource(self, resource: Resource) -> Self {
        let resource = match self.resource {
            Some(existing) => Some(existing.merge(&resource)),
            None => Some(resource),
        };

        TracerProviderBuilder { resource, ..self }
    }

    /// Create a new provider from this configuration.
    pub fn build(self) -> SdkTracerProvider {
        let mut config = self.config;

        // Now, we can update the config with the resource.
        if let Some(resource) = self.resource {
            config.resource = Cow::Owned(resource);
        };

        // Standard config will contain an owned [`Resource`] (either sdk default or use supplied)
        // we can optimize the common case with a static ref to avoid cloning the underlying
        // resource data for each span.
        //
        // For the uncommon case where there are multiple tracer providers with different resource
        // configurations, users can optionally provide their own borrowed static resource.
        if matches!(config.resource, Cow::Owned(_)) {
            config.resource =
                match PROVIDER_RESOURCE.get_or_init(|| config.resource.clone().into_owned()) {
                    static_resource if *static_resource == *config.resource.as_ref() => {
                        Cow::Borrowed(static_resource)
                    }
                    _ => config.resource, // Use the new resource if different
                };
        }

        // Create a new vector to hold the modified processors
        let mut processors = self.processors;

        // Set the resource for each processor
        for p in &mut processors {
            p.set_resource(config.resource.as_ref());
        }

        let is_shutdown = AtomicBool::new(false);
        SdkTracerProvider::new(TracerProviderInner {
            processors,
            config,
            is_shutdown,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::error::{OTelSdkError, OTelSdkResult};
    use crate::resource::{
        SERVICE_NAME, TELEMETRY_SDK_LANGUAGE, TELEMETRY_SDK_NAME, TELEMETRY_SDK_VERSION,
    };
    use crate::trace::provider::TracerProviderInner;
    use crate::trace::{Config, Span, SpanProcessor};
    use crate::trace::{SdkTracerProvider, SpanData};
    use crate::Resource;
    use opentelemetry::trace::{Tracer, TracerProvider};
    use opentelemetry::{Context, Key, KeyValue, Value};

    use std::env;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    // fields below is wrapped with Arc so we can assert it
    #[derive(Default, Debug)]
    struct AssertInfo {
        started_span: AtomicU32,
        is_shutdown: AtomicBool,
    }

    #[derive(Default, Debug, Clone)]
    struct SharedAssertInfo(Arc<AssertInfo>);

    impl SharedAssertInfo {
        fn started_span_count(&self, count: u32) -> bool {
            self.0.started_span.load(Ordering::SeqCst) == count
        }
    }

    #[derive(Debug)]
    struct TestSpanProcessor {
        success: bool,
        assert_info: SharedAssertInfo,
    }

    impl TestSpanProcessor {
        fn new(success: bool) -> TestSpanProcessor {
            TestSpanProcessor {
                success,
                assert_info: SharedAssertInfo::default(),
            }
        }

        // get handle to assert info
        fn assert_info(&self) -> SharedAssertInfo {
            self.assert_info.clone()
        }
    }

    impl SpanProcessor for TestSpanProcessor {
        fn on_start(&self, _span: &mut Span, _cx: &Context) {
            self.assert_info
                .0
                .started_span
                .fetch_add(1, Ordering::SeqCst);
        }

        fn on_end(&self, _span: SpanData) {
            // ignore
        }

        fn force_flush(&self) -> OTelSdkResult {
            if self.success {
                Ok(())
            } else {
                Err(OTelSdkError::InternalFailure("cannot export".into()))
            }
        }

        fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
            if self.assert_info.0.is_shutdown.load(Ordering::SeqCst) {
                Ok(())
            } else {
                let _ = self.assert_info.0.is_shutdown.compare_exchange(
                    false,
                    true,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                );
                self.force_flush()
            }
        }
    }

    #[test]
    fn test_force_flush() {
        let tracer_provider = super::SdkTracerProvider::new(TracerProviderInner {
            processors: vec![
                Box::from(TestSpanProcessor::new(true)),
                Box::from(TestSpanProcessor::new(false)),
            ],
            config: Default::default(),
            is_shutdown: AtomicBool::new(false),
        });

        let results = tracer_provider.force_flush();
        assert!(results.is_err());
    }

    #[test]
    fn test_tracer_provider_default_resource() {
        let assert_resource = |provider: &super::SdkTracerProvider,
                               resource_key: &'static str,
                               expect: Option<&'static str>| {
            assert_eq!(
                provider
                    .config()
                    .resource
                    .get(&Key::from_static_str(resource_key))
                    .map(|v| v.to_string()),
                expect.map(|s| s.to_string())
            );
        };
        let assert_telemetry_resource = |provider: &super::SdkTracerProvider| {
            assert_eq!(
                provider
                    .config()
                    .resource
                    .get(&TELEMETRY_SDK_LANGUAGE.into()),
                Some(Value::from("rust"))
            );
            assert_eq!(
                provider.config().resource.get(&TELEMETRY_SDK_NAME.into()),
                Some(Value::from("opentelemetry"))
            );
            assert_eq!(
                provider
                    .config()
                    .resource
                    .get(&TELEMETRY_SDK_VERSION.into()),
                Some(Value::from(env!("CARGO_PKG_VERSION")))
            );
        };

        // If users didn't provide a resource and there isn't a env var set. Use default one.
        temp_env::with_var_unset("OTEL_RESOURCE_ATTRIBUTES", || {
            let default_config_provider = super::SdkTracerProvider::builder().build();
            assert_resource(
                &default_config_provider,
                SERVICE_NAME,
                Some("unknown_service"),
            );
            assert_telemetry_resource(&default_config_provider);
        });

        // If user provided config, use that.
        let custom_config_provider = super::SdkTracerProvider::builder()
            .with_resource(
                Resource::builder_empty()
                    .with_service_name("test_service")
                    .build(),
            )
            .build();
        assert_resource(&custom_config_provider, SERVICE_NAME, Some("test_service"));
        assert_eq!(custom_config_provider.config().resource.len(), 1);

        // If `OTEL_RESOURCE_ATTRIBUTES` is set, read them automatically
        temp_env::with_var(
            "OTEL_RESOURCE_ATTRIBUTES",
            Some("key1=value1, k2, k3=value2"),
            || {
                let env_resource_provider = super::SdkTracerProvider::builder().build();
                assert_resource(
                    &env_resource_provider,
                    SERVICE_NAME,
                    Some("unknown_service"),
                );
                assert_resource(&env_resource_provider, "key1", Some("value1"));
                assert_resource(&env_resource_provider, "k3", Some("value2"));
                assert_telemetry_resource(&env_resource_provider);
                assert_eq!(env_resource_provider.config().resource.len(), 6);
            },
        );

        // When `OTEL_RESOURCE_ATTRIBUTES` is set and also user provided config
        temp_env::with_var(
            "OTEL_RESOURCE_ATTRIBUTES",
            Some("my-custom-key=env-val,k2=value2"),
            || {
                let user_provided_resource_config_provider = super::SdkTracerProvider::builder()
                    .with_resource(
                        Resource::builder()
                            .with_attributes([
                                KeyValue::new("my-custom-key", "my-custom-value"),
                                KeyValue::new("my-custom-key2", "my-custom-value2"),
                            ])
                            .build(),
                    )
                    .build();
                assert_resource(
                    &user_provided_resource_config_provider,
                    SERVICE_NAME,
                    Some("unknown_service"),
                );
                assert_resource(
                    &user_provided_resource_config_provider,
                    "my-custom-key",
                    Some("my-custom-value"),
                );
                assert_resource(
                    &user_provided_resource_config_provider,
                    "my-custom-key2",
                    Some("my-custom-value2"),
                );
                assert_resource(
                    &user_provided_resource_config_provider,
                    "k2",
                    Some("value2"),
                );
                assert_telemetry_resource(&user_provided_resource_config_provider);
                assert_eq!(
                    user_provided_resource_config_provider
                        .config()
                        .resource
                        .len(),
                    7
                );
            },
        );

        // If user provided a resource, it takes priority during collision.
        let no_service_name = super::SdkTracerProvider::builder()
            .with_resource(Resource::empty())
            .build();

        assert_eq!(no_service_name.config().resource.len(), 0)
    }

    #[test]
    fn test_shutdown_noops() {
        let processor = TestSpanProcessor::new(false);
        let assert_handle = processor.assert_info();
        let tracer_provider = super::SdkTracerProvider::new(TracerProviderInner {
            processors: vec![Box::from(processor)],
            config: Default::default(),
            is_shutdown: AtomicBool::new(false),
        });

        let test_tracer_1 = tracer_provider.tracer("test1");
        let _ = test_tracer_1.start("test");

        assert!(assert_handle.started_span_count(1));

        let _ = test_tracer_1.start("test");

        assert!(assert_handle.started_span_count(2));

        let shutdown = |tracer_provider: super::SdkTracerProvider| {
            let _ = tracer_provider.shutdown(); // shutdown once
        };

        // assert tracer provider can be shutdown using on a cloned version
        shutdown(tracer_provider.clone());

        // after shutdown we should get noop tracer
        let noop_tracer = tracer_provider.tracer("noop");

        // noop tracer cannot start anything
        let _ = noop_tracer.start("test");
        assert!(assert_handle.started_span_count(2));
        // noop tracer's tracer provider should be shutdown
        assert!(noop_tracer.provider().is_shutdown());

        // existing tracer becomes noop after shutdown
        let _ = test_tracer_1.start("test");
        assert!(assert_handle.started_span_count(2));

        // also existing tracer's tracer provider are in shutdown state
        assert!(test_tracer_1.provider().is_shutdown());
    }

    #[test]
    fn with_resource_multiple_calls_ensure_additive() {
        let resource = SdkTracerProvider::builder()
            .with_resource(Resource::new(vec![KeyValue::new("key1", "value1")]))
            .with_resource(Resource::new(vec![KeyValue::new("key2", "value2")]))
            .with_resource(
                Resource::builder_empty()
                    .with_schema_url(vec![], "http://example.com")
                    .build(),
            )
            .with_resource(Resource::new(vec![KeyValue::new("key3", "value3")]))
            .build()
            .inner
            .config
            .resource
            .clone()
            .into_owned();

        assert_eq!(
            resource.get(&Key::from_static_str("key1")),
            Some(Value::from("value1"))
        );
        assert_eq!(
            resource.get(&Key::from_static_str("key2")),
            Some(Value::from("value2"))
        );
        assert_eq!(
            resource.get(&Key::from_static_str("key3")),
            Some(Value::from("value3"))
        );
        assert_eq!(resource.schema_url(), Some("http://example.com"));
    }

    #[derive(Debug)]
    struct CountingShutdownProcessor {
        shutdown_count: Arc<AtomicU32>,
    }

    impl CountingShutdownProcessor {
        fn new(shutdown_count: Arc<AtomicU32>) -> Self {
            CountingShutdownProcessor { shutdown_count }
        }
    }

    impl SpanProcessor for CountingShutdownProcessor {
        fn on_start(&self, _span: &mut Span, _cx: &Context) {
            // No operation needed for this processor
        }

        fn on_end(&self, _span: SpanData) {
            // No operation needed for this processor
        }

        fn force_flush(&self) -> OTelSdkResult {
            Ok(())
        }

        fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
            self.shutdown_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn drop_test_with_multiple_providers() {
        let shutdown_count = Arc::new(AtomicU32::new(0));

        {
            // Create a shared TracerProviderInner and use it across multiple providers
            let shared_inner = Arc::new(TracerProviderInner {
                processors: vec![Box::new(CountingShutdownProcessor::new(
                    shutdown_count.clone(),
                ))],
                config: Config::default(),
                is_shutdown: AtomicBool::new(false),
            });

            {
                let tracer_provider1 = super::SdkTracerProvider {
                    inner: shared_inner.clone(),
                };
                let tracer_provider2 = super::SdkTracerProvider {
                    inner: shared_inner.clone(),
                };

                let tracer1 = tracer_provider1.tracer("test-tracer1");
                let tracer2 = tracer_provider2.tracer("test-tracer2");

                let _span1 = tracer1.start("span1");
                let _span2 = tracer2.start("span2");

                // TracerProviderInner should not be dropped yet, since both providers and `shared_inner`
                // are still holding a reference.
            }
            // At this point, both `tracer_provider1` and `tracer_provider2` are dropped,
            // but `shared_inner` still holds a reference, so `TracerProviderInner` is NOT dropped yet.
            assert_eq!(shutdown_count.load(Ordering::SeqCst), 0);
        }
        // Verify shutdown was called during the drop of the shared TracerProviderInner
        assert_eq!(shutdown_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn drop_after_shutdown_test_with_multiple_providers() {
        let shutdown_count = Arc::new(AtomicU32::new(0));

        // Create a shared TracerProviderInner and use it across multiple providers
        let shared_inner = Arc::new(TracerProviderInner {
            processors: vec![Box::new(CountingShutdownProcessor::new(
                shutdown_count.clone(),
            ))],
            config: Config::default(),
            is_shutdown: AtomicBool::new(false),
        });

        // Create a scope to test behavior when providers are dropped
        {
            let tracer_provider1 = super::SdkTracerProvider {
                inner: shared_inner.clone(),
            };
            let tracer_provider2 = super::SdkTracerProvider {
                inner: shared_inner.clone(),
            };

            // Explicitly shut down the tracer provider
            let shutdown_result = tracer_provider1.shutdown();
            assert!(shutdown_result.is_ok());

            // Verify that shutdown was called exactly once
            assert_eq!(shutdown_count.load(Ordering::SeqCst), 1);

            // TracerProvider2 should observe the shutdown state but not trigger another shutdown
            let shutdown_result2 = tracer_provider2.shutdown();
            assert!(shutdown_result2.is_err());
            assert_eq!(shutdown_count.load(Ordering::SeqCst), 1);

            // Both tracer providers will be dropped at the end of this scope
        }

        // Verify that shutdown was only called once, even after drop
        assert_eq!(shutdown_count.load(Ordering::SeqCst), 1);
    }
}
