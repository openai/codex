#![allow(dead_code)]
use core::fmt;
use std::{borrow::Cow, sync::Arc};

use opentelemetry::{
    metrics::{
        AsyncInstrumentBuilder, Counter, Gauge, Histogram, HistogramBuilder, InstrumentBuilder,
        InstrumentProvider, ObservableCounter, ObservableGauge, ObservableUpDownCounter,
        UpDownCounter,
    },
    otel_error, InstrumentationScope,
};

use crate::metrics::{
    error::{MetricError, MetricResult},
    instrument::{Instrument, InstrumentKind, Observable, ResolvedMeasures},
    internal::{self, Number},
    pipeline::{Pipelines, Resolver},
};

use super::noop::NoopSyncInstrument;

// maximum length of instrument name
pub(crate) const INSTRUMENT_NAME_MAX_LENGTH: usize = 255;
// maximum length of instrument unit name
pub(crate) const INSTRUMENT_UNIT_NAME_MAX_LENGTH: usize = 63;
// Characters allowed in instrument name
pub(crate) const INSTRUMENT_NAME_ALLOWED_NON_ALPHANUMERIC_CHARS: [char; 4] = ['_', '.', '-', '/'];

// name validation error strings
pub(crate) const INSTRUMENT_NAME_EMPTY: &str = "name must be non-empty";
pub(crate) const INSTRUMENT_NAME_LENGTH: &str = "name must be less than 256 characters";
pub(crate) const INSTRUMENT_NAME_INVALID_CHAR: &str =
    "characters in name must be ASCII and belong to the alphanumeric characters, '_', '.', '-' and '/'";
pub(crate) const INSTRUMENT_NAME_FIRST_ALPHABETIC: &str =
    "name must start with an alphabetic character";

// unit validation error strings
pub(crate) const INSTRUMENT_UNIT_LENGTH: &str = "unit must be less than 64 characters";
pub(crate) const INSTRUMENT_UNIT_INVALID_CHAR: &str = "characters in unit must be ASCII";

/// Handles the creation and coordination of all metric instruments.
///
/// A meter represents a single instrumentation scope; all metric telemetry
/// produced by an instrumentation scope will use metric instruments from a
/// single meter.
///
/// See the [Meter API] docs for usage.
///
/// [Meter API]: opentelemetry::metrics::Meter
pub(crate) struct SdkMeter {
    scope: InstrumentationScope,
    pipes: Arc<Pipelines>,
    u64_resolver: Resolver<u64>,
    i64_resolver: Resolver<i64>,
    f64_resolver: Resolver<f64>,
}

impl SdkMeter {
    pub(crate) fn new(scope: InstrumentationScope, pipes: Arc<Pipelines>) -> Self {
        let view_cache = Default::default();

        SdkMeter {
            scope,
            pipes: Arc::clone(&pipes),
            u64_resolver: Resolver::new(Arc::clone(&pipes), Arc::clone(&view_cache)),
            i64_resolver: Resolver::new(Arc::clone(&pipes), Arc::clone(&view_cache)),
            f64_resolver: Resolver::new(pipes, view_cache),
        }
    }

    fn create_counter<T>(
        &self,
        builder: InstrumentBuilder<'_, Counter<T>>,
        resolver: &InstrumentResolver<'_, T>,
    ) -> Counter<T>
    where
        T: Number,
    {
        let validation_result = validate_instrument_config(builder.name.as_ref(), &builder.unit);
        if let Err(err) = validation_result {
            otel_error!(
                name: "InstrumentCreationFailed",
                meter_name = self.scope.name(),
                instrument_name = builder.name.as_ref(),
                message = "Measurements from this Counter will be ignored.",
                reason = format!("{}", err)
            );
            return Counter::new(Arc::new(NoopSyncInstrument::new()));
        }

        match resolver
            .lookup(
                InstrumentKind::Counter,
                builder.name.clone(),
                builder.description,
                builder.unit,
                None,
            )
            .map(|i| Counter::new(Arc::new(i)))
        {
            Ok(counter) => counter,
            Err(err) => {
                otel_error!(
                    name: "InstrumentCreationFailed",
                    meter_name = self.scope.name(),
                    instrument_name = builder.name.as_ref(),
                    message = "Measurements from this Counter will be ignored.",
                    reason = format!("{}", err)
                );
                Counter::new(Arc::new(NoopSyncInstrument::new()))
            }
        }
    }

    fn create_observable_counter<T>(
        &self,
        builder: AsyncInstrumentBuilder<'_, ObservableCounter<T>, T>,
        resolver: &InstrumentResolver<'_, T>,
    ) -> ObservableCounter<T>
    where
        T: Number,
    {
        let validation_result = validate_instrument_config(builder.name.as_ref(), &builder.unit);
        if let Err(err) = validation_result {
            otel_error!(
                name: "InstrumentCreationFailed", 
                meter_name = self.scope.name(),
                instrument_name = builder.name.as_ref(),
                message = "Callbacks for this ObservableCounter will not be invoked.",
                reason = format!("{}", err));
            return ObservableCounter::new();
        }

        match resolver.measures(
            InstrumentKind::ObservableCounter,
            builder.name.clone(),
            builder.description,
            builder.unit,
            None,
        ) {
            Ok(ms) => {
                if ms.is_empty() {
                    otel_error!(
                        name: "InstrumentCreationFailed",
                        meter_name = self.scope.name(),
                        instrument_name = builder.name.as_ref(),
                        message = "Callbacks for this ObservableCounter will not be invoked. Check View Configuration."
                    );
                    return ObservableCounter::new();
                }

                let observable = Arc::new(Observable::new(ms));

                for callback in builder.callbacks {
                    let cb_inst = Arc::clone(&observable);
                    self.pipes
                        .register_callback(move || callback(cb_inst.as_ref()));
                }

                ObservableCounter::new()
            }
            Err(err) => {
                otel_error!(
                    name: "InstrumentCreationFailed",
                    meter_name = self.scope.name(),
                    instrument_name = builder.name.as_ref(),
                    message = "Callbacks for this ObservableCounter will not be invoked.",
                    reason = format!("{}", err));
                ObservableCounter::new()
            }
        }
    }

    fn create_observable_updown_counter<T>(
        &self,
        builder: AsyncInstrumentBuilder<'_, ObservableUpDownCounter<T>, T>,
        resolver: &InstrumentResolver<'_, T>,
    ) -> ObservableUpDownCounter<T>
    where
        T: Number,
    {
        let validation_result = validate_instrument_config(builder.name.as_ref(), &builder.unit);
        if let Err(err) = validation_result {
            otel_error!(
                name: "InstrumentCreationFailed", 
                meter_name = self.scope.name(),
                instrument_name = builder.name.as_ref(),
                message = "Callbacks for this ObservableUpDownCounter will not be invoked.",
                reason = format!("{}", err));
            return ObservableUpDownCounter::new();
        }

        match resolver.measures(
            InstrumentKind::ObservableUpDownCounter,
            builder.name.clone(),
            builder.description,
            builder.unit,
            None,
        ) {
            Ok(ms) => {
                if ms.is_empty() {
                    otel_error!(
                        name: "InstrumentCreationFailed",
                        meter_name = self.scope.name(),
                        instrument_name = builder.name.as_ref(),
                        message = "Callbacks for this ObservableUpDownCounter will not be invoked. Check View Configuration."
                    );
                    return ObservableUpDownCounter::new();
                }

                let observable = Arc::new(Observable::new(ms));

                for callback in builder.callbacks {
                    let cb_inst = Arc::clone(&observable);
                    self.pipes
                        .register_callback(move || callback(cb_inst.as_ref()));
                }

                ObservableUpDownCounter::new()
            }
            Err(err) => {
                otel_error!(
                    name: "InstrumentCreationFailed",
                    meter_name = self.scope.name(),
                    instrument_name = builder.name.as_ref(),
                    message = "Callbacks for this ObservableUpDownCounter will not be invoked.",
                    reason = format!("{}", err));
                ObservableUpDownCounter::new()
            }
        }
    }

    fn create_observable_gauge<T>(
        &self,
        builder: AsyncInstrumentBuilder<'_, ObservableGauge<T>, T>,
        resolver: &InstrumentResolver<'_, T>,
    ) -> ObservableGauge<T>
    where
        T: Number,
    {
        let validation_result = validate_instrument_config(builder.name.as_ref(), &builder.unit);
        if let Err(err) = validation_result {
            otel_error!(
                name: "InstrumentCreationFailed", 
                meter_name = self.scope.name(),
                instrument_name = builder.name.as_ref(),
                message = "Callbacks for this ObservableGauge will not be invoked.",
                reason = format!("{}", err));
            return ObservableGauge::new();
        }

        match resolver.measures(
            InstrumentKind::ObservableGauge,
            builder.name.clone(),
            builder.description,
            builder.unit,
            None,
        ) {
            Ok(ms) => {
                if ms.is_empty() {
                    otel_error!(
                        name: "InstrumentCreationFailed",
                        meter_name = self.scope.name(),
                        instrument_name = builder.name.as_ref(),
                        message = "Callbacks for this ObservableGauge will not be invoked. Check View Configuration."
                    );
                    return ObservableGauge::new();
                }

                let observable = Arc::new(Observable::new(ms));

                for callback in builder.callbacks {
                    let cb_inst = Arc::clone(&observable);
                    self.pipes
                        .register_callback(move || callback(cb_inst.as_ref()));
                }

                ObservableGauge::new()
            }
            Err(err) => {
                otel_error!(
                    name: "InstrumentCreationFailed",
                    meter_name = self.scope.name(),
                    instrument_name = builder.name.as_ref(),
                    message = "Callbacks for this ObservableGauge will not be invoked.",
                    reason = format!("{}", err));
                ObservableGauge::new()
            }
        }
    }

    fn create_updown_counter<T>(
        &self,
        builder: InstrumentBuilder<'_, UpDownCounter<T>>,
        resolver: &InstrumentResolver<'_, T>,
    ) -> UpDownCounter<T>
    where
        T: Number,
    {
        let validation_result = validate_instrument_config(builder.name.as_ref(), &builder.unit);
        if let Err(err) = validation_result {
            otel_error!(
                name: "InstrumentCreationFailed",
                meter_name = self.scope.name(),
                instrument_name = builder.name.as_ref(),
                message = "Measurements from this UpDownCounter will be ignored.",
                reason = format!("{}", err)
            );
            return UpDownCounter::new(Arc::new(NoopSyncInstrument::new()));
        }

        match resolver
            .lookup(
                InstrumentKind::UpDownCounter,
                builder.name.clone(),
                builder.description,
                builder.unit,
                None,
            )
            .map(|i| UpDownCounter::new(Arc::new(i)))
        {
            Ok(updown_counter) => updown_counter,
            Err(err) => {
                otel_error!(
                    name: "InstrumentCreationFailed",
                    meter_name = self.scope.name(),
                    instrument_name = builder.name.as_ref(),
                    message = "Measurements from this UpDownCounter will be ignored.",
                    reason = format!("{}", err)
                );
                UpDownCounter::new(Arc::new(NoopSyncInstrument::new()))
            }
        }
    }

    fn create_gauge<T>(
        &self,
        builder: InstrumentBuilder<'_, Gauge<T>>,
        resolver: &InstrumentResolver<'_, T>,
    ) -> Gauge<T>
    where
        T: Number,
    {
        let validation_result = validate_instrument_config(builder.name.as_ref(), &builder.unit);
        if let Err(err) = validation_result {
            otel_error!(
                name: "InstrumentCreationFailed",
                meter_name = self.scope.name(),
                instrument_name = builder.name.as_ref(),
                message = "Measurements from this Gauge will be ignored.",
                reason = format!("{}", err)
            );
            return Gauge::new(Arc::new(NoopSyncInstrument::new()));
        }

        match resolver
            .lookup(
                InstrumentKind::Gauge,
                builder.name.clone(),
                builder.description,
                builder.unit,
                None,
            )
            .map(|i| Gauge::new(Arc::new(i)))
        {
            Ok(gauge) => gauge,
            Err(err) => {
                otel_error!(
                    name: "InstrumentCreationFailed",
                    meter_name = self.scope.name(),
                    instrument_name = builder.name.as_ref(),
                    message = "Measurements from this Gauge will be ignored.",
                    reason = format!("{}", err)
                );
                Gauge::new(Arc::new(NoopSyncInstrument::new()))
            }
        }
    }

    fn create_histogram<T>(
        &self,
        builder: HistogramBuilder<'_, Histogram<T>>,
        resolver: &InstrumentResolver<'_, T>,
    ) -> Histogram<T>
    where
        T: Number,
    {
        let validation_result = validate_instrument_config(builder.name.as_ref(), &builder.unit);
        if let Err(err) = validation_result {
            otel_error!(
                name: "InstrumentCreationFailed",
                meter_name = self.scope.name(),
                instrument_name = builder.name.as_ref(),
                message = "Measurements from this Histogram will be ignored.",
                reason = format!("{}", err)
            );
            return Histogram::new(Arc::new(NoopSyncInstrument::new()));
        }

        if let Some(ref boundaries) = builder.boundaries {
            let validation_result = validate_bucket_boundaries(boundaries);
            if let Err(err) = validation_result {
                // TODO: Include the buckets too in the error message.
                // TODO: This validation is not done when Views are used to
                // provide boundaries, and that should be fixed.
                otel_error!(
                    name: "InstrumentCreationFailed",
                    meter_name = self.scope.name(),
                    instrument_name = builder.name.as_ref(),
                    message = "Measurements from this Histogram will be ignored.",
                    reason = format!("{}", err)
                );
                return Histogram::new(Arc::new(NoopSyncInstrument::new()));
            }
        }

        match resolver
            .lookup(
                InstrumentKind::Histogram,
                builder.name.clone(),
                builder.description,
                builder.unit,
                builder.boundaries,
            )
            .map(|i| Histogram::new(Arc::new(i)))
        {
            Ok(histogram) => histogram,
            Err(err) => {
                otel_error!(
                    name: "InstrumentCreationFailed",
                    meter_name = self.scope.name(),
                    instrument_name = builder.name.as_ref(),
                    message = "Measurements from this Histogram will be ignored.",
                    reason = format!("{}", err)
                );
                Histogram::new(Arc::new(NoopSyncInstrument::new()))
            }
        }
    }
}

#[doc(hidden)]
impl InstrumentProvider for SdkMeter {
    fn u64_counter(&self, builder: InstrumentBuilder<'_, Counter<u64>>) -> Counter<u64> {
        let resolver = InstrumentResolver::new(self, &self.u64_resolver);
        self.create_counter(builder, &resolver)
    }

    fn f64_counter(&self, builder: InstrumentBuilder<'_, Counter<f64>>) -> Counter<f64> {
        let resolver = InstrumentResolver::new(self, &self.f64_resolver);
        self.create_counter(builder, &resolver)
    }

    fn u64_observable_counter(
        &self,
        builder: AsyncInstrumentBuilder<'_, ObservableCounter<u64>, u64>,
    ) -> ObservableCounter<u64> {
        let resolver = InstrumentResolver::new(self, &self.u64_resolver);
        self.create_observable_counter(builder, &resolver)
    }

    fn f64_observable_counter(
        &self,
        builder: AsyncInstrumentBuilder<'_, ObservableCounter<f64>, f64>,
    ) -> ObservableCounter<f64> {
        let resolver = InstrumentResolver::new(self, &self.f64_resolver);
        self.create_observable_counter(builder, &resolver)
    }

    fn i64_up_down_counter(
        &self,
        builder: InstrumentBuilder<'_, UpDownCounter<i64>>,
    ) -> UpDownCounter<i64> {
        let resolver = InstrumentResolver::new(self, &self.i64_resolver);
        self.create_updown_counter(builder, &resolver)
    }

    fn f64_up_down_counter(
        &self,
        builder: InstrumentBuilder<'_, UpDownCounter<f64>>,
    ) -> UpDownCounter<f64> {
        let resolver = InstrumentResolver::new(self, &self.f64_resolver);
        self.create_updown_counter(builder, &resolver)
    }

    fn i64_observable_up_down_counter(
        &self,
        builder: AsyncInstrumentBuilder<'_, ObservableUpDownCounter<i64>, i64>,
    ) -> ObservableUpDownCounter<i64> {
        let resolver = InstrumentResolver::new(self, &self.i64_resolver);
        self.create_observable_updown_counter(builder, &resolver)
    }

    fn f64_observable_up_down_counter(
        &self,
        builder: AsyncInstrumentBuilder<'_, ObservableUpDownCounter<f64>, f64>,
    ) -> ObservableUpDownCounter<f64> {
        let resolver = InstrumentResolver::new(self, &self.f64_resolver);
        self.create_observable_updown_counter(builder, &resolver)
    }

    fn u64_gauge(&self, builder: InstrumentBuilder<'_, Gauge<u64>>) -> Gauge<u64> {
        let resolver = InstrumentResolver::new(self, &self.u64_resolver);
        self.create_gauge(builder, &resolver)
    }

    fn f64_gauge(&self, builder: InstrumentBuilder<'_, Gauge<f64>>) -> Gauge<f64> {
        let resolver = InstrumentResolver::new(self, &self.f64_resolver);
        self.create_gauge(builder, &resolver)
    }

    fn i64_gauge(&self, builder: InstrumentBuilder<'_, Gauge<i64>>) -> Gauge<i64> {
        let resolver = InstrumentResolver::new(self, &self.i64_resolver);
        self.create_gauge(builder, &resolver)
    }

    fn u64_observable_gauge(
        &self,
        builder: AsyncInstrumentBuilder<'_, ObservableGauge<u64>, u64>,
    ) -> ObservableGauge<u64> {
        let resolver = InstrumentResolver::new(self, &self.u64_resolver);
        self.create_observable_gauge(builder, &resolver)
    }

    fn i64_observable_gauge(
        &self,
        builder: AsyncInstrumentBuilder<'_, ObservableGauge<i64>, i64>,
    ) -> ObservableGauge<i64> {
        let resolver = InstrumentResolver::new(self, &self.i64_resolver);
        self.create_observable_gauge(builder, &resolver)
    }

    fn f64_observable_gauge(
        &self,
        builder: AsyncInstrumentBuilder<'_, ObservableGauge<f64>, f64>,
    ) -> ObservableGauge<f64> {
        let resolver = InstrumentResolver::new(self, &self.f64_resolver);
        self.create_observable_gauge(builder, &resolver)
    }

    fn f64_histogram(&self, builder: HistogramBuilder<'_, Histogram<f64>>) -> Histogram<f64> {
        let resolver = InstrumentResolver::new(self, &self.f64_resolver);
        self.create_histogram(builder, &resolver)
    }

    fn u64_histogram(&self, builder: HistogramBuilder<'_, Histogram<u64>>) -> Histogram<u64> {
        let resolver = InstrumentResolver::new(self, &self.u64_resolver);
        self.create_histogram(builder, &resolver)
    }
}

fn validate_instrument_config(name: &str, unit: &Option<Cow<'static, str>>) -> MetricResult<()> {
    validate_instrument_name(name).and_then(|_| validate_instrument_unit(unit))
}

fn validate_bucket_boundaries(boundaries: &[f64]) -> MetricResult<()> {
    // Validate boundaries do not contain f64::NAN, f64::INFINITY, or f64::NEG_INFINITY
    for boundary in boundaries {
        if boundary.is_nan() || boundary.is_infinite() {
            return Err(MetricError::InvalidInstrumentConfiguration(
                "Bucket boundaries must not contain NaN, +Inf, or -Inf",
            ));
        }
    }

    // validate that buckets are sorted and non-duplicate
    for i in 1..boundaries.len() {
        if boundaries[i] <= boundaries[i - 1] {
            return Err(MetricError::InvalidInstrumentConfiguration(
                "Bucket boundaries must be sorted and non-duplicate",
            ));
        }
    }

    Ok(())
}

#[cfg(feature = "experimental_metrics_disable_name_validation")]
fn validate_instrument_name(_name: &str) -> MetricResult<()> {
    // No name restrictions when name validation is disabled
    Ok(())
}

#[cfg(not(feature = "experimental_metrics_disable_name_validation"))]
fn validate_instrument_name(name: &str) -> MetricResult<()> {
    if name.is_empty() {
        return Err(MetricError::InvalidInstrumentConfiguration(
            INSTRUMENT_NAME_EMPTY,
        ));
    }
    if name.len() > INSTRUMENT_NAME_MAX_LENGTH {
        return Err(MetricError::InvalidInstrumentConfiguration(
            INSTRUMENT_NAME_LENGTH,
        ));
    }

    if name.starts_with(|c: char| !c.is_ascii_alphabetic()) {
        return Err(MetricError::InvalidInstrumentConfiguration(
            INSTRUMENT_NAME_FIRST_ALPHABETIC,
        ));
    }
    if name.contains(|c: char| {
        !c.is_ascii_alphanumeric() && !INSTRUMENT_NAME_ALLOWED_NON_ALPHANUMERIC_CHARS.contains(&c)
    }) {
        return Err(MetricError::InvalidInstrumentConfiguration(
            INSTRUMENT_NAME_INVALID_CHAR,
        ));
    }
    Ok(())
}

fn validate_instrument_unit(unit: &Option<Cow<'static, str>>) -> MetricResult<()> {
    if let Some(unit) = unit {
        if unit.len() > INSTRUMENT_UNIT_NAME_MAX_LENGTH {
            return Err(MetricError::InvalidInstrumentConfiguration(
                INSTRUMENT_UNIT_LENGTH,
            ));
        }
        if unit.contains(|c: char| !c.is_ascii()) {
            return Err(MetricError::InvalidInstrumentConfiguration(
                INSTRUMENT_UNIT_INVALID_CHAR,
            ));
        }
    }
    Ok(())
}

impl fmt::Debug for SdkMeter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Meter").field("scope", &self.scope).finish()
    }
}

/// Provides all OpenTelemetry instruments.
struct InstrumentResolver<'a, T> {
    meter: &'a SdkMeter,
    resolve: &'a Resolver<T>,
}

impl<'a, T> InstrumentResolver<'a, T>
where
    T: Number,
{
    fn new(meter: &'a SdkMeter, resolve: &'a Resolver<T>) -> Self {
        InstrumentResolver { meter, resolve }
    }

    /// lookup returns the resolved measures.
    fn lookup(
        &self,
        kind: InstrumentKind,
        name: Cow<'static, str>,
        description: Option<Cow<'static, str>>,
        unit: Option<Cow<'static, str>>,
        boundaries: Option<Vec<f64>>,
    ) -> MetricResult<ResolvedMeasures<T>> {
        let aggregators = self.measures(kind, name, description, unit, boundaries)?;
        Ok(ResolvedMeasures {
            measures: aggregators,
        })
    }

    fn measures(
        &self,
        kind: InstrumentKind,
        name: Cow<'static, str>,
        description: Option<Cow<'static, str>>,
        unit: Option<Cow<'static, str>>,
        boundaries: Option<Vec<f64>>,
    ) -> MetricResult<Vec<Arc<dyn internal::Measure<T>>>> {
        let inst = Instrument {
            name,
            description: description.unwrap_or_default(),
            unit: unit.unwrap_or_default(),
            kind,
            scope: self.meter.scope.clone(),
        };

        self.resolve.measures(inst, boundaries)
    }
}

#[allow(unused_imports)]
#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use crate::metrics::error::MetricError;

    use super::{
        validate_instrument_name, validate_instrument_unit, INSTRUMENT_NAME_EMPTY,
        INSTRUMENT_NAME_FIRST_ALPHABETIC, INSTRUMENT_NAME_INVALID_CHAR, INSTRUMENT_NAME_LENGTH,
        INSTRUMENT_UNIT_INVALID_CHAR, INSTRUMENT_UNIT_LENGTH,
    };

    #[test]
    #[cfg(not(feature = "experimental_metrics_disable_name_validation"))]
    fn instrument_name_validation() {
        // (name, expected error)
        let instrument_name_test_cases = vec![
            ("validateName", ""),
            ("_startWithNoneAlphabet", INSTRUMENT_NAME_FIRST_ALPHABETIC),
            ("utf8char锈", INSTRUMENT_NAME_INVALID_CHAR),
            ("a".repeat(255).leak(), ""),
            ("a".repeat(256).leak(), INSTRUMENT_NAME_LENGTH),
            ("invalid name", INSTRUMENT_NAME_INVALID_CHAR),
            ("allow/slash", ""),
            ("allow_under_score", ""),
            ("allow.dots.ok", ""),
            ("", INSTRUMENT_NAME_EMPTY),
            ("\\allow\\slash /sec", INSTRUMENT_NAME_FIRST_ALPHABETIC),
            ("\\allow\\$$slash /sec", INSTRUMENT_NAME_FIRST_ALPHABETIC),
            ("Total $ Count", INSTRUMENT_NAME_INVALID_CHAR),
            (
                "\\test\\UsagePercent(Total) > 80%",
                INSTRUMENT_NAME_FIRST_ALPHABETIC,
            ),
            ("/not / allowed", INSTRUMENT_NAME_FIRST_ALPHABETIC),
        ];
        for (name, expected_error) in instrument_name_test_cases {
            let assert = |result: Result<_, MetricError>| {
                if expected_error.is_empty() {
                    assert!(result.is_ok());
                } else {
                    assert!(matches!(
                        result.unwrap_err(),
                        MetricError::InvalidInstrumentConfiguration(msg) if msg == expected_error
                    ));
                }
            };

            assert(validate_instrument_name(name).map(|_| ()));
        }
    }

    #[test]
    #[cfg(feature = "experimental_metrics_disable_name_validation")]
    fn instrument_name_validation_disabled() {
        // (name, expected error)
        let instrument_name_test_cases = vec![
            ("validateName", ""),
            ("_startWithNoneAlphabet", ""),
            ("utf8char锈", ""),
            ("a".repeat(255).leak(), ""),
            ("a".repeat(256).leak(), ""),
            ("invalid name", ""),
            ("allow/slash", ""),
            ("allow_under_score", ""),
            ("allow.dots.ok", ""),
            ("", ""),
            ("\\allow\\slash /sec", ""),
            ("\\allow\\$$slash /sec", ""),
            ("Total $ Count", ""),
            ("\\test\\UsagePercent(Total) > 80%", ""),
            ("/not / allowed", ""),
        ];
        for (name, expected_error) in instrument_name_test_cases {
            let assert = |result: Result<_, MetricError>| {
                if expected_error.is_empty() {
                    assert!(result.is_ok());
                } else {
                    assert!(matches!(
                        result.unwrap_err(),
                        MetricError::InvalidInstrumentConfiguration(msg) if msg == expected_error
                    ));
                }
            };

            assert(validate_instrument_name(name).map(|_| ()));
        }
    }

    #[test]
    fn instrument_unit_validation() {
        // (unit, expected error)
        let instrument_unit_test_cases = vec![
            (
                "0123456789012345678901234567890123456789012345678901234567890123",
                INSTRUMENT_UNIT_LENGTH,
            ),
            ("utf8char锈", INSTRUMENT_UNIT_INVALID_CHAR),
            ("kb", ""),
            ("Kb/sec", ""),
            ("%", ""),
            ("", ""),
        ];

        for (unit, expected_error) in instrument_unit_test_cases {
            let assert = |result: Result<_, MetricError>| {
                if expected_error.is_empty() {
                    assert!(result.is_ok());
                } else {
                    assert!(matches!(
                        result.unwrap_err(),
                        MetricError::InvalidInstrumentConfiguration(msg) if msg == expected_error
                    ));
                }
            };
            let unit: Option<Cow<'static, str>> = Some(unit.into());

            assert(validate_instrument_unit(&unit).map(|_| ()));
        }
    }
}
