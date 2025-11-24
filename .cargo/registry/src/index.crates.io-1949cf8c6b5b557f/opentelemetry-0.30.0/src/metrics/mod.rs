//! # OpenTelemetry Metrics API

use std::sync::Arc;

mod instruments;
mod meter;
pub(crate) mod noop;
pub use instruments::{
    counter::{Counter, ObservableCounter},
    gauge::{Gauge, ObservableGauge},
    histogram::Histogram,
    up_down_counter::{ObservableUpDownCounter, UpDownCounter},
    AsyncInstrument, AsyncInstrumentBuilder, Callback, HistogramBuilder, InstrumentBuilder,
    SyncInstrument,
};
pub use meter::{Meter, MeterProvider};

/// SDK implemented trait for creating instruments
pub trait InstrumentProvider {
    /// creates an instrument for recording increasing values.
    fn u64_counter(&self, _builder: InstrumentBuilder<'_, Counter<u64>>) -> Counter<u64> {
        Counter::new(Arc::new(noop::NoopSyncInstrument::new()))
    }

    /// creates an instrument for recording increasing values.
    fn f64_counter(&self, _builder: InstrumentBuilder<'_, Counter<f64>>) -> Counter<f64> {
        Counter::new(Arc::new(noop::NoopSyncInstrument::new()))
    }

    /// creates an instrument for recording increasing values via callback.
    fn u64_observable_counter(
        &self,
        _builder: AsyncInstrumentBuilder<'_, ObservableCounter<u64>, u64>,
    ) -> ObservableCounter<u64> {
        ObservableCounter::new()
    }

    /// creates an instrument for recording increasing values via callback.
    fn f64_observable_counter(
        &self,
        _builder: AsyncInstrumentBuilder<'_, ObservableCounter<f64>, f64>,
    ) -> ObservableCounter<f64> {
        ObservableCounter::new()
    }

    /// creates an instrument for recording changes of a value.
    fn i64_up_down_counter(
        &self,
        _builder: InstrumentBuilder<'_, UpDownCounter<i64>>,
    ) -> UpDownCounter<i64> {
        UpDownCounter::new(Arc::new(noop::NoopSyncInstrument::new()))
    }

    /// creates an instrument for recording changes of a value.
    fn f64_up_down_counter(
        &self,
        _builder: InstrumentBuilder<'_, UpDownCounter<f64>>,
    ) -> UpDownCounter<f64> {
        UpDownCounter::new(Arc::new(noop::NoopSyncInstrument::new()))
    }

    /// creates an instrument for recording changes of a value.
    fn i64_observable_up_down_counter(
        &self,
        _builder: AsyncInstrumentBuilder<'_, ObservableUpDownCounter<i64>, i64>,
    ) -> ObservableUpDownCounter<i64> {
        ObservableUpDownCounter::new()
    }

    /// creates an instrument for recording changes of a value via callback.
    fn f64_observable_up_down_counter(
        &self,
        _builder: AsyncInstrumentBuilder<'_, ObservableUpDownCounter<f64>, f64>,
    ) -> ObservableUpDownCounter<f64> {
        ObservableUpDownCounter::new()
    }

    /// creates an instrument for recording independent values.
    fn u64_gauge(&self, _builder: InstrumentBuilder<'_, Gauge<u64>>) -> Gauge<u64> {
        Gauge::new(Arc::new(noop::NoopSyncInstrument::new()))
    }

    /// creates an instrument for recording independent values.
    fn f64_gauge(&self, _builder: InstrumentBuilder<'_, Gauge<f64>>) -> Gauge<f64> {
        Gauge::new(Arc::new(noop::NoopSyncInstrument::new()))
    }

    /// creates an instrument for recording independent values.
    fn i64_gauge(&self, _builder: InstrumentBuilder<'_, Gauge<i64>>) -> Gauge<i64> {
        Gauge::new(Arc::new(noop::NoopSyncInstrument::new()))
    }

    /// creates an instrument for recording the current value via callback.
    fn u64_observable_gauge(
        &self,
        _builder: AsyncInstrumentBuilder<'_, ObservableGauge<u64>, u64>,
    ) -> ObservableGauge<u64> {
        ObservableGauge::new()
    }

    /// creates an instrument for recording the current value via callback.
    fn i64_observable_gauge(
        &self,
        _builder: AsyncInstrumentBuilder<'_, ObservableGauge<i64>, i64>,
    ) -> ObservableGauge<i64> {
        ObservableGauge::new()
    }

    /// creates an instrument for recording the current value via callback.
    fn f64_observable_gauge(
        &self,
        _builder: AsyncInstrumentBuilder<'_, ObservableGauge<f64>, f64>,
    ) -> ObservableGauge<f64> {
        ObservableGauge::new()
    }

    /// creates an instrument for recording a distribution of values.
    fn f64_histogram(&self, _builder: HistogramBuilder<'_, Histogram<f64>>) -> Histogram<f64> {
        Histogram::new(Arc::new(noop::NoopSyncInstrument::new()))
    }

    /// creates an instrument for recording a distribution of values.
    fn u64_histogram(&self, _builder: HistogramBuilder<'_, Histogram<u64>>) -> Histogram<u64> {
        Histogram::new(Arc::new(noop::NoopSyncInstrument::new()))
    }
}
