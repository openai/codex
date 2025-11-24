use core::fmt;
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use opentelemetry::{otel_debug, InstrumentationScope, KeyValue};

use crate::{
    error::{OTelSdkError, OTelSdkResult},
    metrics::{
        aggregation,
        data::{Metric, ResourceMetrics, ScopeMetrics},
        error::{MetricError, MetricResult},
        instrument::{Instrument, InstrumentId, InstrumentKind, Stream},
        internal::{self, AggregateBuilder, Number},
        reader::{MetricReader, SdkProducer},
        view::View,
    },
    Resource,
};

use self::internal::AggregateFns;

use super::{aggregation::Aggregation, Temporality};

/// Connects all of the instruments created by a meter provider to a [MetricReader].
///
/// This is the object that will be registered when a meter provider is
/// created.
///
/// As instruments are created the instrument should be checked if it exists in
/// the views of a the reader, and if so each aggregate function should be added
/// to the pipeline.
#[doc(hidden)]
pub struct Pipeline {
    pub(crate) resource: Resource,
    reader: Box<dyn MetricReader>,
    views: Vec<Arc<dyn View>>,
    inner: Mutex<PipelineInner>,
}

impl fmt::Debug for Pipeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Pipeline")
    }
}

/// Single or multi-instrument callbacks
type GenericCallback = Arc<dyn Fn() + Send + Sync>;

const DEFAULT_CARDINALITY_LIMIT: usize = 2000;

#[derive(Default)]
struct PipelineInner {
    aggregations: HashMap<InstrumentationScope, Vec<InstrumentSync>>,
    callbacks: Vec<GenericCallback>,
}

impl fmt::Debug for PipelineInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PipelineInner")
            .field("aggregations", &self.aggregations)
            .field("callbacks", &self.callbacks.len())
            .finish()
    }
}

impl Pipeline {
    /// Adds the [InstrumentSync] to pipeline with scope.
    ///
    /// This method is not idempotent. Duplicate calls will result in duplicate
    /// additions, it is the callers responsibility to ensure this is called with
    /// unique values.
    fn add_sync(&self, scope: InstrumentationScope, i_sync: InstrumentSync) {
        let _ = self.inner.lock().map(|mut inner| {
            inner.aggregations.entry(scope).or_default().push(i_sync);
        });
    }

    /// Registers a single instrument callback to be run when `produce` is called.
    fn add_callback(&self, callback: GenericCallback) {
        let _ = self
            .inner
            .lock()
            .map(|mut inner| inner.callbacks.push(callback));
    }

    /// Send accumulated telemetry
    fn force_flush(&self) -> OTelSdkResult {
        self.reader.force_flush()
    }

    /// Shut down pipeline
    fn shutdown(&self) -> OTelSdkResult {
        self.reader.shutdown()
    }
}

impl SdkProducer for Pipeline {
    /// Returns aggregated metrics from a single collection.
    fn produce(&self, rm: &mut ResourceMetrics) -> OTelSdkResult {
        let inner = self
            .inner
            .lock()
            .map_err(|_| OTelSdkError::InternalFailure("Failed to lock pipeline".into()))?;
        otel_debug!(
            name: "MeterProviderInvokingObservableCallbacks",
            count =  inner.callbacks.len(),
        );
        for cb in &inner.callbacks {
            // TODO consider parallel callbacks.
            cb();
        }

        rm.resource = self.resource.clone();
        if inner.aggregations.len() > rm.scope_metrics.len() {
            rm.scope_metrics
                .reserve(inner.aggregations.len() - rm.scope_metrics.len());
        }

        let mut i = 0;
        for (scope, instruments) in inner.aggregations.iter() {
            let sm = match rm.scope_metrics.get_mut(i) {
                Some(sm) => sm,
                None => {
                    rm.scope_metrics.push(ScopeMetrics::default());
                    rm.scope_metrics.last_mut().unwrap()
                }
            };
            if instruments.len() > sm.metrics.len() {
                sm.metrics.reserve(instruments.len() - sm.metrics.len());
            }

            let mut j = 0;
            for inst in instruments {
                let mut m = sm.metrics.get_mut(j);
                match (inst.comp_agg.call(m.as_mut().map(|m| &mut m.data)), m) {
                    // No metric to re-use, expect agg to create new metric data
                    ((len, Some(initial_agg)), None) if len > 0 => sm.metrics.push(Metric {
                        name: inst.name.clone(),
                        description: inst.description.clone(),
                        unit: inst.unit.clone(),
                        data: initial_agg,
                    }),
                    // Existing metric can be re-used, update its values
                    ((len, data), Some(prev_agg)) if len > 0 => {
                        if let Some(data) = data {
                            // previous aggregation was of a different type
                            prev_agg.data = data;
                        }
                        prev_agg.name.clone_from(&inst.name);
                        prev_agg.description.clone_from(&inst.description);
                        prev_agg.unit.clone_from(&inst.unit);
                    }
                    _ => continue,
                }

                j += 1;
            }

            sm.metrics.truncate(j);
            if !sm.metrics.is_empty() {
                sm.scope = scope.clone();
                i += 1;
            }
        }

        rm.scope_metrics.truncate(i);

        Ok(())
    }
}

/// A synchronization point between a [Pipeline] and an instrument's aggregate function.
struct InstrumentSync {
    name: Cow<'static, str>,
    description: Cow<'static, str>,
    unit: Cow<'static, str>,
    comp_agg: Arc<dyn internal::ComputeAggregation>,
}

impl fmt::Debug for InstrumentSync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InstrumentSync")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("unit", &self.unit)
            .finish()
    }
}

type Cache<T> = Mutex<HashMap<InstrumentId, MetricResult<Option<Arc<dyn internal::Measure<T>>>>>>;

/// Facilitates inserting of new instruments from a single scope into a pipeline.
struct Inserter<T> {
    /// A cache that holds aggregate function inputs whose
    /// outputs have been inserted into the underlying reader pipeline.
    ///
    /// This cache ensures no duplicate aggregate functions are inserted into
    /// the reader pipeline and if a new request during an instrument creation
    /// asks for the same aggregate function input the same instance is
    /// returned.
    aggregators: Cache<T>,

    /// A cache that holds instrument identifiers for all the instruments a [Meter] has
    /// created.
    ///
    /// It is provided from the `Meter` that owns this inserter. This cache ensures
    /// that during the creation of instruments with the same name but different
    /// options (e.g. description, unit) a warning message is logged.
    views: Arc<Mutex<HashMap<Cow<'static, str>, InstrumentId>>>,

    pipeline: Arc<Pipeline>,
}

impl<T> Inserter<T>
where
    T: Number,
{
    fn new(p: Arc<Pipeline>, vc: Arc<Mutex<HashMap<Cow<'static, str>, InstrumentId>>>) -> Self {
        Inserter {
            aggregators: Default::default(),
            views: vc,
            pipeline: Arc::clone(&p),
        }
    }

    /// Inserts the provided instrument into a pipeline.
    ///
    /// All views the pipeline contains are matched against, and any matching view
    /// that creates a unique [Aggregator] will be inserted into the pipeline and
    /// included in the returned list.
    ///
    /// The returned aggregate functions are ensured to be deduplicated and unique.
    /// If another view in another pipeline that is cached by this inserter's cache
    /// has already inserted the same aggregate function for the same instrument,
    /// that function's instance is returned.
    ///
    /// If another instrument has already been inserted by this inserter, or any
    /// other using the same cache, and it conflicts with the instrument being
    /// inserted in this call, an aggregate function matching the arguments will
    /// still be returned but a log message will also be logged to the OTel global
    /// logger.
    ///
    /// If the passed instrument would result in an incompatible aggregate function,
    /// an error is returned and that aggregate function is not inserted or
    /// returned.
    ///
    /// If an instrument is determined to use a [aggregation::Aggregation::Drop],
    /// that instrument is not inserted nor returned.
    fn instrument(
        &self,
        inst: Instrument,
        boundaries: Option<&[f64]>,
    ) -> MetricResult<Vec<Arc<dyn internal::Measure<T>>>> {
        let mut matched = false;
        let mut measures = vec![];
        let mut errs = vec![];
        let kind = inst.kind;

        // The cache will return the same Aggregator instance. Use stream ids to de duplicate.
        let mut seen = HashSet::new();
        for v in &self.pipeline.views {
            let mut stream = match v.match_inst(&inst) {
                Some(stream) => stream,
                None => continue,
            };
            matched = true;

            if stream.name.is_none() {
                stream.name = Some(inst.name.clone());
            }
            if stream.description.is_none() {
                stream.description = Some(inst.description.clone());
            }
            if stream.unit.is_none() {
                stream.unit = Some(inst.unit.clone());
            }

            let id = self.inst_id(kind, &stream);
            if seen.contains(&id) {
                continue; // This aggregator has already been added
            }

            let agg = match self.cached_aggregator(&inst.scope, kind, stream) {
                Ok(Some(agg)) => agg,
                Ok(None) => continue, // Drop aggregator.
                Err(err) => {
                    errs.push(err);
                    continue;
                }
            };
            seen.insert(id);
            measures.push(agg);
        }

        if matched {
            if errs.is_empty() {
                return Ok(measures);
            } else {
                return Err(MetricError::Other(format!("{errs:?}")));
            }
        }

        // Apply implicit default view if no explicit matched.
        let mut stream = Stream {
            name: Some(inst.name),
            description: Some(inst.description),
            unit: Some(inst.unit),
            aggregation: None,
            allowed_attribute_keys: None,
            cardinality_limit: None,
        };

        // Override default histogram boundaries if provided.
        if let Some(boundaries) = boundaries {
            stream.aggregation = Some(Aggregation::ExplicitBucketHistogram {
                boundaries: boundaries.to_vec(),
                record_min_max: true,
            });
        }

        match self.cached_aggregator(&inst.scope, kind, stream) {
            Ok(agg) => {
                if errs.is_empty() {
                    if let Some(agg) = agg {
                        measures.push(agg);
                    }
                    Ok(measures)
                } else {
                    Err(MetricError::Other(format!("{errs:?}")))
                }
            }
            Err(err) => {
                errs.push(err);
                Err(MetricError::Other(format!("{errs:?}")))
            }
        }
    }

    /// Returns the appropriate aggregate functions for an instrument configuration.
    ///
    /// If the exact instrument has been created within the [Scope], that
    /// aggregate function instance will be returned. Otherwise, a new computed
    /// aggregate function will be cached and returned.
    ///
    /// If the instrument configuration conflicts with an instrument that has
    /// already been created (e.g. description, unit, data type) a warning will be
    /// logged with the global OTel logger. A valid new aggregate function for the
    /// instrument configuration will still be returned without an error.
    ///
    /// If the instrument defines an unknown or incompatible aggregation, an error
    /// is returned.
    fn cached_aggregator(
        &self,
        scope: &InstrumentationScope,
        kind: InstrumentKind,
        mut stream: Stream,
    ) -> MetricResult<Option<Arc<dyn internal::Measure<T>>>> {
        // TODO: Create a separate pub (crate) Stream struct for the pipeline,
        // as Stream will not have any optional fields as None at this point and
        // new struct can better reflect this.
        let mut agg = stream
            .aggregation
            .take()
            .unwrap_or_else(|| default_aggregation_selector(kind));

        // Apply default if stream or reader aggregation returns default
        if matches!(agg, aggregation::Aggregation::Default) {
            agg = default_aggregation_selector(kind);
        }

        if let Err(err) = is_aggregator_compatible(&kind, &agg) {
            return Err(MetricError::Other(format!(
                "creating aggregator with instrumentKind: {:?}, aggregation {:?}: {:?}",
                kind, stream.aggregation, err,
            )));
        }

        let mut id = self.inst_id(kind, &stream);
        // If there is a conflict, the specification says the view should
        // still be applied and a warning should be logged.
        self.log_conflict(&id);

        // If there are requests for the same instrument with different name
        // casing, the first-seen needs to be returned. Use a normalize ID for the
        // cache lookup to ensure the correct comparison.
        id.normalize();

        let mut cache = self.aggregators.lock()?;

        let cached = cache.entry(id).or_insert_with(|| {
            let filter = stream
                .allowed_attribute_keys
                .clone()
                .map(|allowed| Arc::new(move |kv: &KeyValue| allowed.contains(&kv.key)) as Arc<_>);

            let cardinality_limit = stream
                .cardinality_limit
                .unwrap_or(DEFAULT_CARDINALITY_LIMIT);
            let b = AggregateBuilder::new(
                self.pipeline.reader.temporality(kind),
                filter,
                cardinality_limit,
            );
            let AggregateFns { measure, collect } = match aggregate_fn(b, &agg, kind) {
                Ok(Some(inst)) => inst,
                other => return other.map(|fs| fs.map(|inst| inst.measure)), // Drop aggregator or error
            };

            otel_debug!(
                name : "Metrics.InstrumentCreated",
                instrument_name = stream.name.clone().unwrap_or_default().as_ref(),
                cardinality_limit = cardinality_limit,
            );

            self.pipeline.add_sync(
                scope.clone(),
                InstrumentSync {
                    name: stream.name.unwrap_or_default(),
                    description: stream.description.unwrap_or_default(),
                    unit: stream.unit.unwrap_or_default(),
                    comp_agg: collect,
                },
            );

            Ok(Some(measure))
        });

        match cached {
            Ok(opt) => Ok(opt.clone()),
            Err(err) => Err(MetricError::Other(err.to_string())),
        }
    }

    /// Validates if an instrument with the same name as id has already been created.
    ///
    /// If that instrument conflicts with id, a warning is logged.
    fn log_conflict(&self, id: &InstrumentId) {
        if let Ok(views) = self.views.lock() {
            if let Some(existing) = views.get(id.name.to_lowercase().as_str()) {
                if existing == id {
                    return;
                }
                // If an existing instrument with the same name but different attributes is found,
                // log a warning with details about the conflicting metric stream definitions.
                otel_debug!(
                    name: "Instrument.DuplicateMetricStreamDefinitions",
                    message = "duplicate metric stream definitions",
                    reason = format!("names: ({} and {}), descriptions: ({} and {}), kinds: ({:?} and {:?}), units: ({:?} and {:?}), and numbers: ({} and {})",
                    existing.name, id.name,
                    existing.description, id.description,
                    existing.kind, id.kind,
                    existing.unit, id.unit,
                    existing.number, id.number,)
                );
            }
        }
    }

    fn inst_id(&self, kind: InstrumentKind, stream: &Stream) -> InstrumentId {
        InstrumentId {
            name: stream.name.clone().unwrap_or_default(),
            description: stream.description.clone().unwrap_or_default(),
            kind,
            unit: stream.unit.clone().unwrap_or_default(),
            number: Cow::Borrowed(std::any::type_name::<T>()),
        }
    }
}

/// The default aggregation and parameters for an instrument of [InstrumentKind].
///
/// This aggregation selector uses the following selection mapping per [the spec]:
///
/// * Counter ⇨ Sum
/// * Observable Counter ⇨ Sum
/// * UpDownCounter ⇨ Sum
/// * Observable UpDownCounter ⇨ Sum
/// * Gauge ⇨ LastValue
/// * Observable Gauge ⇨ LastValue
/// * Histogram ⇨ ExplicitBucketHistogram
///
/// [the spec]: https://github.com/open-telemetry/opentelemetry-specification/blob/v1.19.0/specification/metrics/sdk.md#default-aggregation
fn default_aggregation_selector(kind: InstrumentKind) -> Aggregation {
    match kind {
        InstrumentKind::Counter
        | InstrumentKind::UpDownCounter
        | InstrumentKind::ObservableCounter
        | InstrumentKind::ObservableUpDownCounter => Aggregation::Sum,
        InstrumentKind::Gauge => Aggregation::LastValue,
        InstrumentKind::ObservableGauge => Aggregation::LastValue,
        InstrumentKind::Histogram => Aggregation::ExplicitBucketHistogram {
            boundaries: vec![
                0.0, 5.0, 10.0, 25.0, 50.0, 75.0, 100.0, 250.0, 500.0, 750.0, 1000.0, 2500.0,
                5000.0, 7500.0, 10000.0,
            ],
            record_min_max: true,
        },
    }
}

/// Returns new aggregate functions for the given params.
///
/// If the aggregation is unknown or temporality is invalid, an error is returned.
fn aggregate_fn<T: Number>(
    b: AggregateBuilder<T>,
    agg: &aggregation::Aggregation,
    kind: InstrumentKind,
) -> MetricResult<Option<AggregateFns<T>>> {
    match agg {
        Aggregation::Default => aggregate_fn(b, &default_aggregation_selector(kind), kind),
        Aggregation::Drop => Ok(None),
        Aggregation::LastValue => {
            match kind {
                InstrumentKind::Gauge => Ok(Some(b.last_value(None))),
                // temporality for LastValue only affects how data points are reported, so we can always use
                // delta temporality, because observable instruments should report data points only since previous collection
                InstrumentKind::ObservableGauge => Ok(Some(b.last_value(Some(Temporality::Delta)))),
                _ => Err(MetricError::Other(format!("LastValue aggregation is only available for Gauge or ObservableGauge, but not for {kind:?}")))
            }
        }
        Aggregation::Sum => {
            let fns = match kind {
                // TODO implement: observable instruments should not report data points on every collect
                // from SDK: For asynchronous instruments with Delta or Cumulative aggregation temporality,
                // MetricReader.Collect MUST only receive data points with measurements recorded since the previous collection
                InstrumentKind::ObservableCounter => b.precomputed_sum(true),
                InstrumentKind::ObservableUpDownCounter => b.precomputed_sum(false),
                InstrumentKind::Counter | InstrumentKind::Histogram => b.sum(true),
                _ => b.sum(false),
            };
            Ok(Some(fns))
        }
        Aggregation::ExplicitBucketHistogram {
            boundaries,
            record_min_max,
        } => {
            let record_sum = !matches!(
                kind,
                InstrumentKind::UpDownCounter
                    | InstrumentKind::ObservableUpDownCounter
                    | InstrumentKind::ObservableGauge
            );
            // TODO implement: observable instruments should not report data points on every collect
            // from SDK: For asynchronous instruments with Delta or Cumulative aggregation temporality,
            // MetricReader.Collect MUST only receive data points with measurements recorded since the previous collection
            Ok(Some(b.explicit_bucket_histogram(
                boundaries.to_vec(),
                *record_min_max,
                record_sum,
            )))
        }
        Aggregation::Base2ExponentialHistogram {
            max_size,
            max_scale,
            record_min_max,
        } => {
            let record_sum = !matches!(
                kind,
                InstrumentKind::UpDownCounter
                    | InstrumentKind::ObservableUpDownCounter
                    | InstrumentKind::ObservableGauge
            );
            Ok(Some(b.exponential_bucket_histogram(
                *max_size,
                *max_scale,
                *record_min_max,
                record_sum,
            )))
        }
    }
}

/// Checks if the aggregation can be used by the instrument.
///
/// Current compatibility:
///
/// | Instrument Kind          | Drop | LastValue | Sum | Histogram | Exponential Histogram |
/// |--------------------------|------|-----------|-----|-----------|-----------------------|
/// | Counter                  | ✓    |           | ✓   | ✓         | ✓                     |
/// | UpDownCounter            | ✓    |           | ✓   | ✓         | ✓                     |
/// | Histogram                | ✓    |           | ✓   | ✓         | ✓                     |
/// | Observable Counter       | ✓    |           | ✓   | ✓         | ✓                     |
/// | Observable UpDownCounter | ✓    |           | ✓   | ✓         | ✓                     |
/// | Gauge                    | ✓    | ✓         |     | ✓         | ✓                     |
/// | Observable Gauge         | ✓    | ✓         |     | ✓         | ✓                     |
fn is_aggregator_compatible(
    kind: &InstrumentKind,
    agg: &aggregation::Aggregation,
) -> MetricResult<()> {
    match agg {
        Aggregation::Default => Ok(()),
        Aggregation::ExplicitBucketHistogram { .. }
        | Aggregation::Base2ExponentialHistogram { .. } => {
            if matches!(
                kind,
                InstrumentKind::Counter
                    | InstrumentKind::UpDownCounter
                    | InstrumentKind::Gauge
                    | InstrumentKind::Histogram
                    | InstrumentKind::ObservableCounter
                    | InstrumentKind::ObservableUpDownCounter
                    | InstrumentKind::ObservableGauge
            ) {
                return Ok(());
            }
            Err(MetricError::Other("incompatible aggregation".into()))
        }
        Aggregation::Sum => {
            match kind {
                InstrumentKind::ObservableCounter
                | InstrumentKind::ObservableUpDownCounter
                | InstrumentKind::Counter
                | InstrumentKind::Histogram
                | InstrumentKind::UpDownCounter => Ok(()),
                _ => {
                    // TODO: review need for aggregation check after
                    // https://github.com/open-telemetry/opentelemetry-specification/issues/2710
                    Err(MetricError::Other("incompatible aggregation".into()))
                }
            }
        }
        Aggregation::LastValue => {
            match kind {
                InstrumentKind::Gauge | InstrumentKind::ObservableGauge => Ok(()),
                _ => {
                    // TODO: review need for aggregation check after
                    // https://github.com/open-telemetry/opentelemetry-specification/issues/2710
                    Err(MetricError::Other("incompatible aggregation".into()))
                }
            }
        }
        Aggregation::Drop => Ok(()),
    }
}

/// The group of pipelines connecting Readers with instrument measurement.
#[derive(Clone, Debug)]
pub(crate) struct Pipelines(pub(crate) Vec<Arc<Pipeline>>);

impl Pipelines {
    pub(crate) fn new(
        res: Resource,
        readers: Vec<Box<dyn MetricReader>>,
        views: Vec<Arc<dyn View>>,
    ) -> Self {
        let mut pipes = Vec::with_capacity(readers.len());
        for r in readers {
            let p = Arc::new(Pipeline {
                resource: res.clone(),
                reader: r,
                views: views.clone(),
                inner: Default::default(),
            });
            p.reader.register_pipeline(Arc::downgrade(&p));
            pipes.push(p);
        }

        Pipelines(pipes)
    }

    pub(crate) fn register_callback<F>(&self, callback: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        let cb = Arc::new(callback);
        for pipe in &self.0 {
            pipe.add_callback(cb.clone())
        }
    }

    /// Force flush all pipelines
    pub(crate) fn force_flush(&self) -> OTelSdkResult {
        let mut errs = vec![];
        for pipeline in &self.0 {
            if let Err(err) = pipeline.force_flush() {
                errs.push(err);
            }
        }

        if errs.is_empty() {
            Ok(())
        } else {
            Err(OTelSdkError::InternalFailure(format!("{errs:?}")))
        }
    }

    /// Shut down all pipelines
    pub(crate) fn shutdown(&self) -> OTelSdkResult {
        let mut errs = vec![];
        for pipeline in &self.0 {
            if let Err(err) = pipeline.shutdown() {
                errs.push(err);
            }
        }

        if errs.is_empty() {
            Ok(())
        } else {
            Err(crate::error::OTelSdkError::InternalFailure(format!(
                "{errs:?}"
            )))
        }
    }
}

/// resolver facilitates resolving aggregate functions an instrument calls to
/// aggregate measurements with while updating all pipelines that need to pull from
/// those aggregations.
pub(crate) struct Resolver<T> {
    inserters: Vec<Inserter<T>>,
}

impl<T> Resolver<T>
where
    T: Number,
{
    pub(crate) fn new(
        pipelines: Arc<Pipelines>,
        view_cache: Arc<Mutex<HashMap<Cow<'static, str>, InstrumentId>>>,
    ) -> Self {
        let inserters = pipelines
            .0
            .iter()
            .map(|pipe| Inserter::new(Arc::clone(pipe), Arc::clone(&view_cache)))
            .collect();

        Resolver { inserters }
    }

    /// The measures that must be updated by the instrument defined by key.
    pub(crate) fn measures(
        &self,
        id: Instrument,
        boundaries: Option<Vec<f64>>,
    ) -> MetricResult<Vec<Arc<dyn internal::Measure<T>>>> {
        let (mut measures, mut errs) = (vec![], vec![]);

        for inserter in &self.inserters {
            match inserter.instrument(id.clone(), boundaries.as_deref()) {
                Ok(ms) => measures.extend(ms),
                Err(err) => errs.push(err),
            }
        }

        if errs.is_empty() {
            if measures.is_empty() {
                // TODO: Emit internal log that measurements from the instrument
                // are being dropped due to view configuration
            }
            Ok(measures)
        } else {
            Err(MetricError::Other(format!("{errs:?}")))
        }
    }
}
