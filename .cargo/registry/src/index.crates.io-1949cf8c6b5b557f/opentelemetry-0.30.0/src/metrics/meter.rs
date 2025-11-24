use core::fmt;
use std::borrow::Cow;
use std::sync::Arc;

use crate::metrics::{
    AsyncInstrumentBuilder, Gauge, InstrumentBuilder, InstrumentProvider, ObservableCounter,
    ObservableGauge, ObservableUpDownCounter, UpDownCounter,
};
use crate::InstrumentationScope;

use super::{Counter, Histogram, HistogramBuilder};

/// Provides access to named [Meter] instances, for instrumenting an application
/// or crate.
pub trait MeterProvider {
    /// Returns a new [Meter] with the provided name and default configuration.
    ///
    /// A [Meter] should be scoped at most to a single application or crate. The
    /// name needs to be unique so it does not collide with other names used by
    /// an application, nor other applications.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::{global, metrics::MeterProvider};
    /// use opentelemetry::KeyValue;
    ///
    /// let provider = global::meter_provider();
    ///
    /// // meter used in applications
    /// let meter = provider.meter("my_app");
    /// ```
    fn meter(&self, name: &'static str) -> Meter {
        let scope = InstrumentationScope::builder(name).build();
        self.meter_with_scope(scope)
    }

    /// Returns a new [Meter] with the given instrumentation scope.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use opentelemetry::InstrumentationScope;
    /// use opentelemetry::metrics::MeterProvider;
    /// use opentelemetry_sdk::metrics::SdkMeterProvider;
    ///
    /// let provider = SdkMeterProvider::default();
    ///
    /// // meter used in applications/binaries
    /// let meter = provider.meter("my_app");
    ///
    /// // meter used in libraries/crates that optionally includes version and schema url
    /// let scope = InstrumentationScope::builder(env!("CARGO_PKG_NAME"))
    ///     .with_version(env!("CARGO_PKG_VERSION"))
    ///     .with_schema_url("https://opentelemetry.io/schema/1.0.0")
    ///     .build();
    ///
    /// let meter = provider.meter_with_scope(scope);
    /// ```
    fn meter_with_scope(&self, scope: InstrumentationScope) -> Meter;
}

/// Provides the ability to create instruments for recording measurements or
/// accepting callbacks to report measurements.
///
/// # Instrument Types
///
/// Instruments are categorized as either synchronous or asynchronous:
///
/// - **Synchronous Instruments** (e.g., Counter): These are used inline with
///   your application's processing logic. For example, you might use a Counter
///   to record the number of HTTP requests received.
///
/// - **Asynchronous Instruments** (e.g., ObservableGauge): These allow you to
///   register a callback function that is invoked during export. For instance,
///   you could use an asynchronous gauge to monitor temperature from a sensor
///   every time metrics are exported.
///
/// # Example Usage
///
/// ```rust
/// use opentelemetry::{global, KeyValue};
///
/// let meter = global::meter("my-meter");
///
/// // Synchronous Instruments
///
/// // u64 Counter
/// let u64_counter = meter.u64_counter("my_u64_counter").build();
/// u64_counter.add(
///     10,
///     &[
///         KeyValue::new("mykey1", "myvalue1"),
///         KeyValue::new("mykey2", "myvalue2"),
///     ],
/// );
///
/// // f64 Counter
/// let f64_counter = meter.f64_counter("my_f64_counter").build();
/// f64_counter.add(
///     3.15,
///     &[
///         KeyValue::new("mykey1", "myvalue1"),
///         KeyValue::new("mykey2", "myvalue2"),
///     ],
/// );
///
///
/// // u64 Observable Counter
/// let _observable_u64_counter = meter
///     .u64_observable_counter("my_observable_u64_counter")
///     .with_description("My observable counter example")
///     .with_unit("myunit")
///     .with_callback(|observer| {
///         observer.observe(
///             100,
///             &[
///                 KeyValue::new("mykey1", "myvalue1"),
///                 KeyValue::new("mykey2", "myvalue2"),
///             ],
///         )
///     })
///     .build();
///
/// // f64 Observable Counter
/// let _observable_f64_counter = meter
///     .f64_observable_counter("my_observable_f64_counter")
///     .with_description("My observable counter example")
///     .with_unit("myunit")
///     .with_callback(|observer| {
///         observer.observe(
///             100.0,
///             &[
///                 KeyValue::new("mykey1", "myvalue1"),
///                 KeyValue::new("mykey2", "myvalue2"),
///             ],
///         )
///     })
///     .build();
///
/// // i64 UpDownCounter
/// let updown_i64_counter = meter.i64_up_down_counter("my_updown_i64_counter").build();
/// updown_i64_counter.add(
///     -10,
///     &[
///         KeyValue::new("mykey1", "myvalue1"),
///         KeyValue::new("mykey2", "myvalue2"),
///     ],
/// );
///
/// // f64 UpDownCounter
/// let updown_f64_counter = meter.f64_up_down_counter("my_updown_f64_counter").build();
/// updown_f64_counter.add(
///     -10.67,
///     &[
///         KeyValue::new("mykey1", "myvalue1"),
///         KeyValue::new("mykey2", "myvalue2"),
///     ],
/// );
///
/// // i64 Observable UpDownCounter
/// let _observable_updown_i64_counter = meter
///     .i64_observable_up_down_counter("my_observable_i64_updown_counter")
///     .with_description("My observable updown counter example")
///     .with_unit("myunit")
///     .with_callback(|observer| {
///         observer.observe(
///             100,
///             &[
///                 KeyValue::new("mykey1", "myvalue1"),
///                 KeyValue::new("mykey2", "myvalue2"),
///             ],
///         )
///     })
///     .build();
///
/// // f64 Observable UpDownCounter
/// let _observable_updown_f64_counter = meter
///     .f64_observable_up_down_counter("my_observable_f64_updown_counter")
///     .with_description("My observable updown counter example")
///     .with_unit("myunit")
///     .with_callback(|observer| {
///         observer.observe(
///             100.0,
///             &[
///                 KeyValue::new("mykey1", "myvalue1"),
///                 KeyValue::new("mykey2", "myvalue2"),
///             ],
///         )
///     })
///     .build();
///
/// // i64 Gauge
/// let gauge = meter.i64_gauge("my_gauge").build();
/// gauge.record(
/// -10,
/// &[
///     KeyValue::new("mykey1", "myvalue1"),
///     KeyValue::new("mykey2", "myvalue2"),
/// ],
/// );
///
/// // u64 Gauge
/// let gauge = meter.u64_gauge("my_gauge").build();
/// gauge.record(
/// 101,
/// &[
///     KeyValue::new("mykey1", "myvalue1"),
///     KeyValue::new("mykey2", "myvalue2"),
/// ],
/// );
///
/// // f64 Gauge
/// let gauge = meter.f64_gauge("my_gauge").build();
/// gauge.record(
/// 12.5,
/// &[
///     KeyValue::new("mykey1", "myvalue1"),
///     KeyValue::new("mykey2", "myvalue2"),
/// ],
/// );
///
/// // u64 Observable Gauge
/// let _observable_u64_gauge = meter
///     .u64_observable_gauge("my_u64_gauge")
///     .with_description("An observable gauge set to 1")
///     .with_unit("myunit")
///     .with_callback(|observer| {
///         observer.observe(
///             1,
///             &[
///                 KeyValue::new("mykey1", "myvalue1"),
///                 KeyValue::new("mykey2", "myvalue2"),
///             ],
///         )
///     })
///     .build();
///
/// // f64 Observable Gauge
/// let _observable_f64_gauge = meter
///     .f64_observable_gauge("my_f64_gauge")
///     .with_description("An observable gauge set to 1.0")
///     .with_unit("myunit")
///     .with_callback(|observer| {
///         observer.observe(
///             1.0,
///             &[
///                 KeyValue::new("mykey1", "myvalue1"),
///                 KeyValue::new("mykey2", "myvalue2"),
///             ],
///         )
///     })
///     .build();
///
/// // i64 Observable Gauge
/// let _observable_i64_gauge = meter
///     .i64_observable_gauge("my_i64_gauge")
///     .with_description("An observable gauge set to 1")
///     .with_unit("myunit")
///     .with_callback(|observer| {
///         observer.observe(
///             1,
///             &[
///                 KeyValue::new("mykey1", "myvalue1"),
///                 KeyValue::new("mykey2", "myvalue2"),
///             ],
///         )
///     })
///     .build();
///
/// // f64 Histogram
/// let f64_histogram = meter.f64_histogram("my_f64_histogram").build();
/// f64_histogram.record(
///     10.5,
///     &[
///         KeyValue::new("mykey1", "myvalue1"),
///         KeyValue::new("mykey2", "myvalue2"),
///     ],
/// );
///
/// // u64 Histogram
/// let u64_histogram = meter.u64_histogram("my_u64_histogram").build();
/// u64_histogram.record(
///     12,
///     &[
///         KeyValue::new("mykey1", "myvalue1"),
///         KeyValue::new("mykey2", "myvalue2"),
///     ],
/// );
/// ```
///
#[derive(Clone)]
#[non_exhaustive]
pub struct Meter {
    pub(crate) instrument_provider: Arc<dyn InstrumentProvider + Send + Sync>,
}

impl Meter {
    /// Create a new named meter from an instrumentation provider
    #[doc(hidden)]
    pub fn new(instrument_provider: Arc<dyn InstrumentProvider + Send + Sync>) -> Self {
        Meter {
            instrument_provider,
        }
    }

    /// creates an instrument builder for recording increasing values.
    ///
    /// [`Counter`] can be cloned to create multiple handles to the same instrument. If a [`Counter`] needs to be shared,
    /// users are recommended to clone the [`Counter`] instead of creating duplicate [`Counter`]s for the same metric. Creating
    /// duplicate [`Counter`]s for the same metric could lower SDK performance.
    pub fn u64_counter(
        &self,
        name: impl Into<Cow<'static, str>>,
    ) -> InstrumentBuilder<'_, Counter<u64>> {
        InstrumentBuilder::new(self, name.into())
    }

    /// creates an instrument builder for recording increasing values.
    ///
    /// [`Counter`] can be cloned to create multiple handles to the same instrument. If a [`Counter`] needs to be shared,
    /// users are recommended to clone the [`Counter`] instead of creating duplicate [`Counter`]s for the same metric. Creating
    /// duplicate [`Counter`]s for the same metric could lower SDK performance.
    pub fn f64_counter(
        &self,
        name: impl Into<Cow<'static, str>>,
    ) -> InstrumentBuilder<'_, Counter<f64>> {
        InstrumentBuilder::new(self, name.into())
    }

    /// creates an instrument builder for recording increasing values via callback.
    pub fn u64_observable_counter(
        &self,
        name: impl Into<Cow<'static, str>>,
    ) -> AsyncInstrumentBuilder<'_, ObservableCounter<u64>, u64> {
        AsyncInstrumentBuilder::new(self, name.into())
    }

    /// creates an instrument builder for recording increasing values via callback.
    pub fn f64_observable_counter(
        &self,
        name: impl Into<Cow<'static, str>>,
    ) -> AsyncInstrumentBuilder<'_, ObservableCounter<f64>, f64> {
        AsyncInstrumentBuilder::new(self, name.into())
    }

    /// creates an instrument builder for recording changes of a value.
    ///
    /// [`UpDownCounter`] can be cloned to create multiple handles to the same instrument. If a [`UpDownCounter`] needs to be shared,
    /// users are recommended to clone the [`UpDownCounter`] instead of creating duplicate [`UpDownCounter`]s for the same metric. Creating
    /// duplicate [`UpDownCounter`]s for the same metric could lower SDK performance.
    pub fn i64_up_down_counter(
        &self,
        name: impl Into<Cow<'static, str>>,
    ) -> InstrumentBuilder<'_, UpDownCounter<i64>> {
        InstrumentBuilder::new(self, name.into())
    }

    /// creates an instrument builder for recording changes of a value.
    ///
    /// [`UpDownCounter`] can be cloned to create multiple handles to the same instrument. If a [`UpDownCounter`] needs to be shared,
    /// users are recommended to clone the [`UpDownCounter`] instead of creating duplicate [`UpDownCounter`]s for the same metric. Creating
    /// duplicate [`UpDownCounter`]s for the same metric could lower SDK performance.
    pub fn f64_up_down_counter(
        &self,
        name: impl Into<Cow<'static, str>>,
    ) -> InstrumentBuilder<'_, UpDownCounter<f64>> {
        InstrumentBuilder::new(self, name.into())
    }

    /// creates an instrument builder for recording changes of a value via callback.
    ///
    /// [`UpDownCounter`] can be cloned to create multiple handles to the same instrument. If a [`UpDownCounter`] needs to be shared,
    /// users are recommended to clone the [`UpDownCounter`] instead of creating duplicate [`UpDownCounter`]s for the same metric. Creating
    /// duplicate [`UpDownCounter`]s for the same metric could lower SDK performance.
    pub fn i64_observable_up_down_counter(
        &self,
        name: impl Into<Cow<'static, str>>,
    ) -> AsyncInstrumentBuilder<'_, ObservableUpDownCounter<i64>, i64> {
        AsyncInstrumentBuilder::new(self, name.into())
    }

    /// creates an instrument builder for recording changes of a value via callback.
    pub fn f64_observable_up_down_counter(
        &self,
        name: impl Into<Cow<'static, str>>,
    ) -> AsyncInstrumentBuilder<'_, ObservableUpDownCounter<f64>, f64> {
        AsyncInstrumentBuilder::new(self, name.into())
    }

    /// creates an instrument builder for recording independent values.
    ///
    /// [`Gauge`] can be cloned to create multiple handles to the same instrument. If a [`Gauge`] needs to be shared,
    /// users are recommended to clone the [`Gauge`] instead of creating duplicate [`Gauge`]s for the same metric. Creating
    /// duplicate [`Gauge`]s for the same metric could lower SDK performance.
    pub fn u64_gauge(
        &self,
        name: impl Into<Cow<'static, str>>,
    ) -> InstrumentBuilder<'_, Gauge<u64>> {
        InstrumentBuilder::new(self, name.into())
    }

    /// creates an instrument builder for recording independent values.
    ///
    /// [`Gauge`] can be cloned to create multiple handles to the same instrument. If a [`Gauge`] needs to be shared,
    /// users are recommended to clone the [`Gauge`] instead of creating duplicate [`Gauge`]s for the same metric. Creating
    /// duplicate [`Gauge`]s for the same metric could lower SDK performance.
    pub fn f64_gauge(
        &self,
        name: impl Into<Cow<'static, str>>,
    ) -> InstrumentBuilder<'_, Gauge<f64>> {
        InstrumentBuilder::new(self, name.into())
    }

    /// creates an instrument builder for recording independent values.
    /// [`Gauge`] can be cloned to create multiple handles to the same instrument. If a [`Gauge`] needs to be shared,
    /// users are recommended to clone the [`Gauge`] instead of creating duplicate [`Gauge`]s for the same metric. Creating
    /// duplicate [`Gauge`]s for the same metric could lower SDK performance.
    pub fn i64_gauge(
        &self,
        name: impl Into<Cow<'static, str>>,
    ) -> InstrumentBuilder<'_, Gauge<i64>> {
        InstrumentBuilder::new(self, name.into())
    }

    /// creates an instrument builder for recording the current value via callback.
    pub fn u64_observable_gauge(
        &self,
        name: impl Into<Cow<'static, str>>,
    ) -> AsyncInstrumentBuilder<'_, ObservableGauge<u64>, u64> {
        AsyncInstrumentBuilder::new(self, name.into())
    }

    /// creates an instrument builder for recording the current value via callback.
    pub fn i64_observable_gauge(
        &self,
        name: impl Into<Cow<'static, str>>,
    ) -> AsyncInstrumentBuilder<'_, ObservableGauge<i64>, i64> {
        AsyncInstrumentBuilder::new(self, name.into())
    }

    /// creates an instrument builder for recording the current value via callback.
    pub fn f64_observable_gauge(
        &self,
        name: impl Into<Cow<'static, str>>,
    ) -> AsyncInstrumentBuilder<'_, ObservableGauge<f64>, f64> {
        AsyncInstrumentBuilder::new(self, name.into())
    }

    /// creates an instrument builder for recording a distribution of values.
    ///
    /// [`Histogram`] can be cloned to create multiple handles to the same instrument. If a [`Histogram`] needs to be shared,
    /// users are recommended to clone the [`Histogram`] instead of creating duplicate [`Histogram`]s for the same metric. Creating
    /// duplicate [`Histogram`]s for the same metric could lower SDK performance.
    pub fn f64_histogram(
        &self,
        name: impl Into<Cow<'static, str>>,
    ) -> HistogramBuilder<'_, Histogram<f64>> {
        HistogramBuilder::new(self, name.into())
    }

    /// creates an instrument builder for recording a distribution of values.
    ///
    /// [`Histogram`] can be cloned to create multiple handles to the same instrument. If a [`Histogram`] needs to be shared,
    /// users are recommended to clone the [`Histogram`] instead of creating duplicate [`Histogram`]s for the same metric. Creating
    /// duplicate [`Histogram`]s for the same metric could lower SDK performance.
    pub fn u64_histogram(
        &self,
        name: impl Into<Cow<'static, str>>,
    ) -> HistogramBuilder<'_, Histogram<u64>> {
        HistogramBuilder::new(self, name.into())
    }
}

impl fmt::Debug for Meter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Meter")
    }
}
