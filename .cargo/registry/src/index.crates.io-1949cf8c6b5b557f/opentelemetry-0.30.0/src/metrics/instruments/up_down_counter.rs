use crate::KeyValue;
use core::fmt;
use std::sync::Arc;

use super::SyncInstrument;

/// An instrument that records increasing or decreasing values.
///
/// [`UpDownCounter`] can be cloned to create multiple handles to the same instrument. If a [`UpDownCounter`] needs to be shared,
/// users are recommended to clone the [`UpDownCounter`] instead of creating duplicate [`UpDownCounter`]s for the same metric. Creating
/// duplicate [`UpDownCounter`]s for the same metric could lower SDK performance.
#[derive(Clone)]
#[non_exhaustive]
pub struct UpDownCounter<T>(Arc<dyn SyncInstrument<T> + Send + Sync>);

impl<T> fmt::Debug for UpDownCounter<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "UpDownCounter<{}>",
            std::any::type_name::<T>()
        ))
    }
}

impl<T> UpDownCounter<T> {
    /// Create a new up down counter.
    pub fn new(inner: Arc<dyn SyncInstrument<T> + Send + Sync>) -> Self {
        UpDownCounter(inner)
    }

    /// Records an increment or decrement to the counter.
    pub fn add(&self, value: T, attributes: &[KeyValue]) {
        self.0.measure(value, attributes)
    }
}

/// An async instrument that records increasing or decreasing values.
#[derive(Clone)]
#[non_exhaustive]
pub struct ObservableUpDownCounter<T> {
    _marker: std::marker::PhantomData<T>,
}

impl<T> fmt::Debug for ObservableUpDownCounter<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "ObservableUpDownCounter<{}>",
            std::any::type_name::<T>()
        ))
    }
}

impl<T> ObservableUpDownCounter<T> {
    /// Create a new observable up down counter.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        ObservableUpDownCounter {
            _marker: std::marker::PhantomData,
        }
    }
}
