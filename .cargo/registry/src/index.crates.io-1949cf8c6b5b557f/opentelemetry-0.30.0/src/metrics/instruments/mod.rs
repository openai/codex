use gauge::{Gauge, ObservableGauge};

use crate::metrics::Meter;
use crate::KeyValue;
use core::fmt;
use std::borrow::Cow;
use std::marker;

use super::{
    Counter, Histogram, InstrumentProvider, ObservableCounter, ObservableUpDownCounter,
    UpDownCounter,
};

pub(super) mod counter;
pub(super) mod gauge;
pub(super) mod histogram;
pub(super) mod up_down_counter;

/// An SDK implemented instrument that records measurements via callback.
pub trait AsyncInstrument<T>: Send + Sync {
    /// Observes the state of the instrument.
    ///
    /// It is only valid to call this within a callback.
    fn observe(&self, measurement: T, attributes: &[KeyValue]);
}

/// An SDK implemented instrument that records measurements synchronously.
pub trait SyncInstrument<T>: Send + Sync {
    /// Records a measurement synchronously.
    fn measure(&self, measurement: T, attributes: &[KeyValue]);
}

/// Configuration for building a Histogram.
#[non_exhaustive] // We expect to add more configuration fields in the future
pub struct HistogramBuilder<'a, T> {
    /// Instrument provider is used to create the instrument.
    pub instrument_provider: &'a dyn InstrumentProvider,

    /// Name of the Histogram.
    pub name: Cow<'static, str>,

    /// Description of the Histogram.
    pub description: Option<Cow<'static, str>>,

    /// Unit of the Histogram.
    pub unit: Option<Cow<'static, str>>,

    /// Bucket boundaries for the histogram.
    pub boundaries: Option<Vec<f64>>,

    // boundaries: Vec<T>,
    _marker: marker::PhantomData<T>,
}

impl<'a, T> HistogramBuilder<'a, T> {
    /// Create a new instrument builder
    pub(crate) fn new(meter: &'a Meter, name: Cow<'static, str>) -> Self {
        HistogramBuilder {
            instrument_provider: meter.instrument_provider.as_ref(),
            name,
            description: None,
            unit: None,
            boundaries: None,
            _marker: marker::PhantomData,
        }
    }

    /// Set the description for this instrument
    pub fn with_description<S: Into<Cow<'static, str>>>(mut self, description: S) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the unit for this instrument.
    ///
    /// Unit is case sensitive(`kb` is not the same as `kB`).
    ///
    /// Unit must be:
    /// - ASCII string
    /// - No longer than 63 characters
    pub fn with_unit<S: Into<Cow<'static, str>>>(mut self, unit: S) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// Set the boundaries for this histogram.
    ///
    /// Setting boundaries is optional. By default, the boundaries are set to:
    ///
    /// `[0.0, 5.0, 10.0, 25.0, 50.0, 75.0, 100.0, 250.0, 500.0, 750.0, 1000.0,
    /// 2500.0, 5000.0, 7500.0, 10000.0]`
    ///
    /// # Notes
    /// - Boundaries must not contain `f64::NAN`, `f64::INFINITY` or
    ///   `f64::NEG_INFINITY`
    /// - Values must be in strictly increasing order (e.g., each value must be
    ///   greater than the previous).
    /// - Boundaries must not contain duplicate values.
    ///
    /// If invalid boundaries are provided, the instrument will not report
    /// measurements.
    /// Providing an empty `vec![]` means no bucket information will be
    /// calculated.
    ///
    /// # Warning
    /// Using more buckets can improve the accuracy of percentile calculations in backends.
    /// However, this comes at a cost, including increased memory, CPU, and network usage.
    /// Choose the number of buckets carefully, considering your application's performance
    /// and resource requirements.
    pub fn with_boundaries(mut self, boundaries: Vec<f64>) -> Self {
        self.boundaries = Some(boundaries);
        self
    }
}

impl HistogramBuilder<'_, Histogram<f64>> {
    /// Creates a new instrument.
    ///
    /// Validates the instrument configuration and creates a new instrument. In
    /// case of invalid configuration, a no-op instrument is returned
    /// and an error is logged using internal logging.
    pub fn build(self) -> Histogram<f64> {
        self.instrument_provider.f64_histogram(self)
    }
}

impl HistogramBuilder<'_, Histogram<u64>> {
    /// Creates a new instrument.
    ///
    /// Validates the instrument configuration and creates a new instrument. In
    /// case of invalid configuration, a no-op instrument is returned
    /// and an error is logged using internal logging.
    pub fn build(self) -> Histogram<u64> {
        self.instrument_provider.u64_histogram(self)
    }
}

/// Configuration for building a sync instrument.
#[non_exhaustive] // We expect to add more configuration fields in the future
pub struct InstrumentBuilder<'a, T> {
    /// Instrument provider is used to create the instrument.
    pub instrument_provider: &'a dyn InstrumentProvider,

    /// Name of the instrument.
    pub name: Cow<'static, str>,

    /// Description of the instrument.
    pub description: Option<Cow<'static, str>>,

    /// Unit of the instrument.
    pub unit: Option<Cow<'static, str>>,

    _marker: marker::PhantomData<T>,
}

impl<'a, T> InstrumentBuilder<'a, T> {
    /// Create a new instrument builder
    pub(crate) fn new(meter: &'a Meter, name: Cow<'static, str>) -> Self {
        InstrumentBuilder {
            instrument_provider: meter.instrument_provider.as_ref(),
            name,
            description: None,
            unit: None,
            _marker: marker::PhantomData,
        }
    }

    /// Set the description for this instrument
    pub fn with_description<S: Into<Cow<'static, str>>>(mut self, description: S) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the unit for this instrument.
    ///
    /// Unit is case sensitive(`kb` is not the same as `kB`).
    ///
    /// Unit must be:
    /// - ASCII string
    /// - No longer than 63 characters
    pub fn with_unit<S: Into<Cow<'static, str>>>(mut self, unit: S) -> Self {
        self.unit = Some(unit.into());
        self
    }
}

macro_rules! build_instrument {
    ($name:ident, $inst:ty) => {
        impl<'a> InstrumentBuilder<'a, $inst> {
            #[doc = concat!("Validates the instrument configuration and creates a new `",  stringify!($inst), "`.")]
            /// In case of invalid configuration, a no-op instrument is returned
            /// and an error is logged using internal logging.
            pub fn build(self) -> $inst {
                self.instrument_provider.$name(self)
            }
        }
    };
}

build_instrument!(u64_counter, Counter<u64>);
build_instrument!(f64_counter, Counter<f64>);
build_instrument!(u64_gauge, Gauge<u64>);
build_instrument!(f64_gauge, Gauge<f64>);
build_instrument!(i64_gauge, Gauge<i64>);
build_instrument!(i64_up_down_counter, UpDownCounter<i64>);
build_instrument!(f64_up_down_counter, UpDownCounter<f64>);

impl<T> fmt::Debug for InstrumentBuilder<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InstrumentBuilder")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("unit", &self.unit)
            .field("kind", &std::any::type_name::<T>())
            .finish()
    }
}

impl<T> fmt::Debug for HistogramBuilder<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HistogramBuilder")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("unit", &self.unit)
            .field("boundaries", &self.boundaries)
            .field(
                "kind",
                &format!("Histogram<{}>", &std::any::type_name::<T>()),
            )
            .finish()
    }
}

/// A function registered with a [Meter] that makes observations for the
/// instruments it is registered with.
///
/// The async instrument parameter is used to record measurement observations
/// for these instruments.
///
/// The function needs to complete in a finite amount of time.
pub type Callback<T> = Box<dyn Fn(&dyn AsyncInstrument<T>) + Send + Sync>;

/// Configuration for building an async instrument.
#[non_exhaustive] // We expect to add more configuration fields in the future
pub struct AsyncInstrumentBuilder<'a, I, M> {
    /// Instrument provider is used to create the instrument.
    pub instrument_provider: &'a dyn InstrumentProvider,

    /// Name of the instrument.
    pub name: Cow<'static, str>,

    /// Description of the instrument.
    pub description: Option<Cow<'static, str>>,

    /// Unit of the instrument.
    pub unit: Option<Cow<'static, str>>,

    /// Callbacks to be called for this instrument.
    pub callbacks: Vec<Callback<M>>,

    _inst: marker::PhantomData<I>,
}

impl<'a, I, M> AsyncInstrumentBuilder<'a, I, M> {
    /// Create a new instrument builder
    pub(crate) fn new(meter: &'a Meter, name: Cow<'static, str>) -> Self {
        AsyncInstrumentBuilder {
            instrument_provider: meter.instrument_provider.as_ref(),
            name,
            description: None,
            unit: None,
            _inst: marker::PhantomData,
            callbacks: Vec::new(),
        }
    }

    /// Set the description for this instrument
    pub fn with_description<S: Into<Cow<'static, str>>>(mut self, description: S) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the unit for this instrument.
    ///
    /// Unit is case sensitive(`kb` is not the same as `kB`).
    ///
    /// Unit must be:
    /// - ASCII string
    /// - No longer than 63 characters
    pub fn with_unit<S: Into<Cow<'static, str>>>(mut self, unit: S) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// Set the callback to be called for this instrument.
    pub fn with_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&dyn AsyncInstrument<M>) + Send + Sync + 'static,
    {
        self.callbacks.push(Box::new(callback));
        self
    }
}

macro_rules! build_async_instrument {
    ($name:ident, $inst:ty, $measurement:ty) => {
        impl<'a> AsyncInstrumentBuilder<'a, $inst, $measurement> {
            #[doc = concat!("Validates the instrument configuration and creates a new `",  stringify!($inst), "`.")]
            /// In case of invalid configuration, a no-op instrument is returned
            /// and an error is logged using internal logging.
            pub fn build(self) -> $inst {
                self.instrument_provider.$name(self)
            }
        }
    };
}

build_async_instrument!(u64_observable_counter, ObservableCounter<u64>, u64);
build_async_instrument!(f64_observable_counter, ObservableCounter<f64>, f64);
build_async_instrument!(u64_observable_gauge, ObservableGauge<u64>, u64);
build_async_instrument!(f64_observable_gauge, ObservableGauge<f64>, f64);
build_async_instrument!(i64_observable_gauge, ObservableGauge<i64>, i64);
build_async_instrument!(
    i64_observable_up_down_counter,
    ObservableUpDownCounter<i64>,
    i64
);
build_async_instrument!(
    f64_observable_up_down_counter,
    ObservableUpDownCounter<f64>,
    f64
);

impl<I, M> fmt::Debug for AsyncInstrumentBuilder<'_, I, M>
where
    I: AsyncInstrument<M>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InstrumentBuilder")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("unit", &self.unit)
            .field("kind", &std::any::type_name::<I>())
            .field("callbacks_len", &self.callbacks.len())
            .finish()
    }
}
