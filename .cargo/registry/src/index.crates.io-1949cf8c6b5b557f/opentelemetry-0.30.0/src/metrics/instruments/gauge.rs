use crate::KeyValue;
use core::fmt;
use std::sync::Arc;

use super::SyncInstrument;

/// An instrument that records independent values
///
/// [`Gauge`] can be cloned to create multiple handles to the same instrument. If a [`Gauge`] needs to be shared,
/// users are recommended to clone the [`Gauge`] instead of creating duplicate [`Gauge`]s for the same metric. Creating
/// duplicate [`Gauge`]s for the same metric could lower SDK performance.
#[derive(Clone)]
#[non_exhaustive]
pub struct Gauge<T>(Arc<dyn SyncInstrument<T> + Send + Sync>);

impl<T> fmt::Debug for Gauge<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("Gauge<{}>", std::any::type_name::<T>()))
    }
}

impl<T> Gauge<T> {
    /// Create a new gauge.
    pub fn new(inner: Arc<dyn SyncInstrument<T> + Send + Sync>) -> Self {
        Gauge(inner)
    }

    /// Records an independent value.
    pub fn record(&self, value: T, attributes: &[KeyValue]) {
        self.0.measure(value, attributes)
    }
}

/// An async instrument that records independent readings.
#[derive(Clone)]
#[non_exhaustive]
pub struct ObservableGauge<T> {
    _marker: std::marker::PhantomData<T>,
}

impl<T> fmt::Debug for ObservableGauge<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "ObservableGauge<{}>",
            std::any::type_name::<T>()
        ))
    }
}

impl<T> ObservableGauge<T> {
    /// Create a new gauge
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        ObservableGauge {
            _marker: std::marker::PhantomData,
        }
    }
}
