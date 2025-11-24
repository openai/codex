//! Utilities to track metrics in Sentry.
//!
//! Metrics are numerical values that can track anything about your environment over time, from
//! latency to error rates to user signups.
//!
//! Metrics at Sentry come in different flavors, in order to help you track your data in the most
//! efficient and cost-effective way. The types of metrics we currently support are:
//!
//!  - **Counters** track a value that can only be incremented.
//!  - **Distributions** track a list of values over time in on which you can perform aggregations
//!    like max, min, avg.
//!  - **Gauges** track a value that can go up and down.
//!  - **Sets** track a set of values on which you can perform aggregations such as count_unique.
//!
//! For more information on metrics in Sentry, see [our docs].
//!
//! # Usage
//!
//! To collect a metric, use the [`Metric`] struct to capture all relevant properties of your
//! metric. Then, use [`send`](Metric::send) to send the metric to Sentry:
//!
//! ```
//! use std::time::Duration;
//! use sentry::metrics::Metric;
//!
//! Metric::count("requests")
//!     .with_tag("method", "GET")
//!     .send();
//!
//! Metric::timing("request.duration", Duration::from_millis(17))
//!     .with_tag("status_code", "200")
//!     // unit is added automatically by timing
//!     .send();
//!
//! Metric::set("site.visitors", "user1")
//!     .with_unit("user")
//!     .send();
//! ```
//!
//! # Usage with Cadence
//!
//! [`cadence`] is a popular Statsd client for Rust and can be used to send metrics to Sentry. To
//! use Sentry directly with `cadence`, see the [`sentry-cadence`](crate::cadence) documentation.
//!
//! [our docs]: https://develop.sentry.dev/delightful-developer-metrics/

mod normalization;

use std::borrow::Cow;
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::{self, Display};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sentry_types::protocol::latest::{Envelope, EnvelopeItem};

use crate::client::TransportArc;
use crate::{ClientOptions, Hub};

pub use crate::units::*;

const BUCKET_INTERVAL: Duration = Duration::from_secs(10);
const FLUSH_INTERVAL: Duration = Duration::from_secs(5);
const MAX_WEIGHT: usize = 100_000;

/// Type alias for strings used in [`Metric`] for names and tags.
pub type MetricStr = Cow<'static, str>;

/// Type used for [`MetricValue::Counter`].
pub type CounterValue = f64;

/// Type used for [`MetricValue::Distribution`].
pub type DistributionValue = f64;

/// Type used for [`MetricValue::Set`].
pub type SetValue = u32;

/// Type used for [`MetricValue::Gauge`].
pub type GaugeValue = f64;

/// The value of a [`Metric`], indicating its type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MetricValue {
    /// Counts instances of an event.
    ///
    /// Counters can be incremented and decremented. The default operation is to increment a counter
    /// by `1`, although increments by larger values and even floating point values are possible.
    ///
    /// # Example
    ///
    /// ```
    /// use sentry::metrics::{Metric, MetricValue};
    ///
    /// Metric::build("my.counter", MetricValue::Counter(1.0)).send();
    /// ```
    Counter(CounterValue),

    /// Builds a statistical distribution over values reported.
    ///
    /// Based on individual reported values, distributions allow to query the maximum, minimum, or
    /// average of the reported values, as well as statistical quantiles. With an increasing number
    /// of values in the distribution, its accuracy becomes approximate.
    ///
    /// # Example
    ///
    /// ```
    /// use sentry::metrics::{Metric, MetricValue};
    ///
    /// Metric::build("my.distribution", MetricValue::Distribution(42.0)).send();
    /// ```
    Distribution(DistributionValue),

    /// Counts the number of unique reported values.
    ///
    /// Sets allow sending arbitrary discrete values, including strings, and store the deduplicated
    /// count. With an increasing number of unique values in the set, its accuracy becomes
    /// approximate. It is not possible to query individual values from a set.
    ///
    /// # Example
    ///
    /// To create a set value, use [`MetricValue::set_from_str`] or
    /// [`MetricValue::set_from_display`]. These functions convert the provided argument into a
    /// unique hash value, which is then used as the set value.
    ///
    /// ```
    /// use sentry::metrics::{Metric, MetricValue};
    ///
    /// Metric::build("my.set", MetricValue::set_from_str("foo")).send();
    /// ```
    Set(SetValue),

    /// Stores absolute snapshots of values.
    ///
    /// In addition to plain [counters](Self::Counter), gauges store a snapshot of the maximum,
    /// minimum and sum of all values, as well as the last reported value. Note that the "last"
    /// component of this aggregation is not commutative. Which value is preserved as last value is
    /// implementation-defined.
    ///
    /// # Example
    ///
    /// ```
    /// use sentry::metrics::{Metric, MetricValue};
    ///
    /// Metric::build("my.gauge", MetricValue::Gauge(42.0)).send();
    /// ```
    Gauge(GaugeValue),
}

impl MetricValue {
    /// Returns a set value representing the given string.
    pub fn set_from_str(string: &str) -> Self {
        Self::Set(hash_set_value(string))
    }

    /// Returns a set value representing the given argument.
    pub fn set_from_display(display: impl fmt::Display) -> Self {
        Self::Set(hash_set_value(&display.to_string()))
    }

    /// Returns the type of the metric value.
    fn ty(&self) -> MetricType {
        match self {
            Self::Counter(_) => MetricType::Counter,
            Self::Distribution(_) => MetricType::Distribution,
            Self::Gauge(_) => MetricType::Gauge,
            Self::Set(_) => MetricType::Set,
        }
    }
}

impl Display for MetricValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Counter(v) => write!(f, "{}", v),
            Self::Distribution(v) => write!(f, "{}", v),
            Self::Gauge(v) => write!(f, "{}", v),
            Self::Set(v) => write!(f, "{}", v),
        }
    }
}

/// Hashes the given set value.
///
/// Sets only guarantee 32-bit accuracy, but arbitrary strings are allowed on the protocol. Upon
/// parsing, they are hashed and only used as hashes subsequently.
fn hash_set_value(string: &str) -> u32 {
    crc32fast::hash(string.as_bytes())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum MetricType {
    Counter,
    Distribution,
    Set,
    Gauge,
}

impl MetricType {
    /// Return the shortcode for this metric type.
    pub fn as_str(&self) -> &'static str {
        match self {
            MetricType::Counter => "c",
            MetricType::Distribution => "d",
            MetricType::Set => "s",
            MetricType::Gauge => "g",
        }
    }
}

impl fmt::Display for MetricType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for MetricType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "c" | "m" => Self::Counter,
            "h" | "d" | "ms" => Self::Distribution,
            "s" => Self::Set,
            "g" => Self::Gauge,
            _ => return Err(()),
        })
    }
}

/// A snapshot of values.
#[derive(Clone, Copy, Debug, PartialEq)]
struct GaugeSummary {
    /// The last value reported in the bucket.
    ///
    /// This aggregation is not commutative.
    pub last: GaugeValue,
    /// The minimum value reported in the bucket.
    pub min: GaugeValue,
    /// The maximum value reported in the bucket.
    pub max: GaugeValue,
    /// The sum of all values reported in the bucket.
    pub sum: GaugeValue,
    /// The number of times this bucket was updated with a new value.
    pub count: u64,
}

impl GaugeSummary {
    /// Creates a gauge snapshot from a single value.
    pub fn single(value: GaugeValue) -> Self {
        Self {
            last: value,
            min: value,
            max: value,
            sum: value,
            count: 1,
        }
    }

    /// Inserts a new value into the gauge.
    pub fn insert(&mut self, value: GaugeValue) {
        self.last = value;
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        self.sum += value;
        self.count += 1;
    }
}

/// The aggregated value of a [`Metric`] bucket.
#[derive(Debug)]
enum BucketValue {
    Counter(CounterValue),
    Distribution(Vec<DistributionValue>),
    Set(BTreeSet<SetValue>),
    Gauge(GaugeSummary),
}

impl BucketValue {
    /// Inserts a new value into the bucket and returns the added weight.
    pub fn insert(&mut self, value: MetricValue) -> usize {
        match (self, value) {
            (Self::Counter(c1), MetricValue::Counter(c2)) => {
                *c1 += c2;
                0
            }
            (Self::Distribution(d1), MetricValue::Distribution(d2)) => {
                d1.push(d2);
                1
            }
            (Self::Set(s1), MetricValue::Set(s2)) => {
                if s1.insert(s2) {
                    1
                } else {
                    0
                }
            }
            (Self::Gauge(g1), MetricValue::Gauge(g2)) => {
                g1.insert(g2);
                0
            }
            _ => panic!("invalid metric type"),
        }
    }

    /// Returns the number of values stored in this bucket.
    pub fn weight(&self) -> usize {
        match self {
            BucketValue::Counter(_) => 1,
            BucketValue::Distribution(v) => v.len(),
            BucketValue::Set(v) => v.len(),
            BucketValue::Gauge(_) => 5,
        }
    }
}

impl From<MetricValue> for BucketValue {
    fn from(value: MetricValue) -> Self {
        match value {
            MetricValue::Counter(v) => Self::Counter(v),
            MetricValue::Distribution(v) => Self::Distribution(vec![v]),
            MetricValue::Gauge(v) => Self::Gauge(GaugeSummary::single(v)),
            MetricValue::Set(v) => Self::Set(BTreeSet::from([v])),
        }
    }
}

/// A metric value that contains a numeric value and metadata to be sent to Sentry.
///
/// # Units
///
/// To make the most out of metrics in Sentry, consider assigning a unit during construction. This
/// can be achieved using the [`with_unit`](MetricBuilder::with_unit) builder method. See the
/// documentation for more examples on units.
///
/// ```
/// use sentry::metrics::{Metric, InformationUnit};
///
/// Metric::distribution("request.size", 47.2)
///     .with_unit(InformationUnit::Byte)
///     .send();
/// ```
///
/// # Sending Metrics
///
/// Metrics can be sent to Sentry directly using the [`send`](MetricBuilder::send) method on the
/// constructor. This will send the metric to the [`Client`](crate::Client) on the current [`Hub`].
/// If there is no client on the current hub, the metric is dropped.
///
/// ```
/// use sentry::metrics::Metric;
///
/// Metric::count("requests")
///     .with_tag("method", "GET")
///     .send();
/// ```
///
/// # Sending to a Custom Client
///
/// Metrics can also be sent to a custom client. This is useful if you want to send metrics to a
/// different Sentry project or with different configuration. To do so, finish building the metric
/// and then call [`add_metric`](crate::Client::add_metric) to the client:
///
/// ```
/// use sentry::Hub;
/// use sentry::metrics::Metric;
///
/// let metric = Metric::count("requests")
///    .with_tag("method", "GET")
///    .finish();
///
/// // Obtain a client from somewhere
/// if let Some(client) = Hub::current().client() {
///     client.add_metric(metric);
/// }
/// ```
#[derive(Debug)]
pub struct Metric {
    /// The name of the metric, identifying it in Sentry.
    ///
    /// The name should consist of
    name: MetricStr,
    unit: MetricUnit,
    value: MetricValue,
    tags: BTreeMap<MetricStr, MetricStr>,
    time: Option<SystemTime>,
}

impl Metric {
    /// Creates a new metric with the stated name and value.
    ///
    /// The provided name identifies the metric in Sentry. It should consist of alphanumeric
    /// characters and `_`, `-`, and `.`. While a single forward slash (`/`) is also allowed in
    /// metric names, it has a special meaning and should not be used in regular metric names. All
    /// characters that do not match this criteria are sanitized.
    ///
    /// The value of the metric determines its type. See the [struct-level](self) docs and
    /// constructor methods for examples on how to build metrics.
    pub fn build(name: impl Into<MetricStr>, value: MetricValue) -> MetricBuilder {
        let metric = Metric {
            name: name.into(),
            unit: MetricUnit::None,
            value,
            tags: BTreeMap::new(),
            time: None,
        };

        MetricBuilder { metric }
    }

    /// Parses a metric from a StatsD string.
    ///
    /// This supports regular StatsD payloads with an extension for tags. In the below example, tags
    /// are optional:
    ///
    /// ```plain
    /// <metricname>:<value>|<type>|#<tag1>:<value1>,<tag2>:<value2>
    /// ```
    ///
    /// Units are encoded into the metric name, separated by an `@`:
    ///
    /// ```plain
    /// <metricname>@<unit>:<value>|<type>|#<tag1>:<value1>,<tag2>:<value2>
    /// ```
    pub fn parse_statsd(string: &str) -> Result<Self, ParseMetricError> {
        parse_metric_opt(string).ok_or(ParseMetricError(()))
    }

    /// Builds a metric that increments a [counter](MetricValue::Counter) by the given value.
    ///
    /// # Example
    ///
    /// ```
    /// use sentry::metrics::{Metric};
    ///
    /// Metric::incr("operation.total_values", 7.0).send();
    /// ```
    pub fn incr(name: impl Into<MetricStr>, value: f64) -> MetricBuilder {
        Self::build(name, MetricValue::Counter(value))
    }

    /// Builds a metric that [counts](MetricValue::Counter) the single occurrence of an event.
    ///
    /// # Example
    ///
    /// ```
    /// use sentry::metrics::{Metric};
    ///
    /// Metric::count("requests").send();
    /// ```
    pub fn count(name: impl Into<MetricStr>) -> MetricBuilder {
        Self::build(name, MetricValue::Counter(1.0))
    }

    /// Builds a metric that tracks the duration of an operation.
    ///
    /// This is a [distribution](MetricValue::Distribution) metric that is tracked in seconds.
    ///
    /// # Example
    ///
    /// ```
    /// use std::time::Duration;
    /// use sentry::metrics::{Metric};
    ///
    /// Metric::timing("operation", Duration::from_secs(1)).send();
    /// ```
    pub fn timing(name: impl Into<MetricStr>, timing: Duration) -> MetricBuilder {
        Self::build(name, MetricValue::Distribution(timing.as_secs_f64()))
            .with_unit(DurationUnit::Second)
    }

    /// Builds a metric that tracks the [distribution](MetricValue::Distribution) of values.
    ///
    /// # Example
    ///
    /// ```
    /// use sentry::metrics::{Metric};
    ///
    /// Metric::distribution("operation.batch_size", 42.0).send();
    /// ```
    pub fn distribution(name: impl Into<MetricStr>, value: f64) -> MetricBuilder {
        Self::build(name, MetricValue::Distribution(value))
    }

    /// Builds a metric that tracks the [unique number](MetricValue::Set) of values provided.
    ///
    /// See [`MetricValue`] for more ways to construct sets.
    ///
    /// # Example
    ///
    /// ```
    /// use sentry::metrics::{Metric};
    ///
    /// Metric::set("users", "user1").send();
    /// ```
    pub fn set(name: impl Into<MetricStr>, string: &str) -> MetricBuilder {
        Self::build(name, MetricValue::set_from_str(string))
    }

    /// Builds a metric that tracks the [snapshot](MetricValue::Gauge) of provided values.
    ///
    /// # Example
    ///
    /// ```
    /// use sentry::metrics::{Metric};
    ///
    /// Metric::gauge("cache.size", 42.0).send();
    /// ```
    pub fn gauge(name: impl Into<MetricStr>, value: f64) -> MetricBuilder {
        Self::build(name, MetricValue::Gauge(value))
    }

    /// Sends the metric to the current client.
    ///
    /// When building a metric, you can use [`MetricBuilder::send`] to send the metric directly. If
    /// there is no client on the current [`Hub`], the metric is dropped.
    pub fn send(self) {
        if let Some(client) = Hub::current().client() {
            client.add_metric(self);
        }
    }

    /// Convert the metric into an [`Envelope`] containing a single [`EnvelopeItem::Statsd`].
    pub fn to_envelope(self) -> Envelope {
        let timestamp = self
            .time
            .unwrap_or(SystemTime::now())
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let data = format!(
            "{}@{}:{}|{}|#{}|T{}",
            normalization::normalize_name(self.name.as_ref()),
            normalization::normalize_unit(self.unit.to_string().as_ref()),
            self.value,
            self.value.ty(),
            normalization::normalize_tags(&self.tags),
            timestamp
        );
        EnvelopeItem::Statsd(data.into_bytes()).into()
    }
}

/// A builder for metrics.
///
/// Use one of the [`Metric`] constructors to create a new builder. See the struct-level docs for
/// examples of how to build metrics.
#[must_use]
#[derive(Debug)]
pub struct MetricBuilder {
    metric: Metric,
}

impl MetricBuilder {
    /// Sets the unit for the metric.
    ///
    /// The unit augments the metric value by giving it a magnitude and semantics. Some units have
    /// special support when rendering metrics or their values in Sentry, such as for timings. See
    /// [`MetricUnit`] for more information on the supported units. The unit can be set to
    /// [`MetricUnit::None`] to indicate that the metric has no unit, or to [`MetricUnit::Custom`]
    /// to indicate a user-defined unit.
    ///
    /// By default, the unit is set to [`MetricUnit::None`].
    pub fn with_unit(mut self, unit: impl Into<MetricUnit>) -> Self {
        self.metric.unit = unit.into();
        self
    }

    /// Adds a tag to the metric.
    ///
    /// Tags allow you to add dimensions to metrics. They are key-value pairs that can be filtered
    /// or grouped by in Sentry.
    ///
    /// When sent to Sentry via [`MetricBuilder::send`] or when added to a
    /// [`Client`](crate::Client), the client may add default tags to the metrics, such as the
    /// `release` or the `environment` from the Scope.
    pub fn with_tag(mut self, name: impl Into<MetricStr>, value: impl Into<MetricStr>) -> Self {
        self.metric.tags.insert(name.into(), value.into());
        self
    }

    /// Adds multiple tags to the metric.
    ///
    /// Tags allow you to add dimensions to metrics. They are key-value pairs that can be filtered
    /// or grouped by in Sentry.
    ///
    /// When sent to Sentry via [`MetricBuilder::send`] or when added to a
    /// [`Client`](crate::Client), the client may add default tags to the metrics, such as the
    /// `release` or the `environment` from the Scope.
    pub fn with_tags<T, K, V>(mut self, tags: T) -> Self
    where
        T: IntoIterator<Item = (K, V)>,
        K: Into<MetricStr>,
        V: Into<MetricStr>,
    {
        for (k, v) in tags {
            self.metric.tags.insert(k.into(), v.into());
        }
        self
    }

    /// Sets the timestamp for the metric.
    ///
    /// By default, the timestamp is set to the current time when the metric is built or sent.
    pub fn with_time(mut self, time: SystemTime) -> Self {
        self.metric.time = Some(time);
        self
    }

    /// Builds the metric.
    pub fn finish(self) -> Metric {
        self.metric
    }

    /// Sends the metric to the current client.
    ///
    /// This is a shorthand for `.finish().send()`. If there is no client on the current [`Hub`],
    /// the metric is dropped.
    pub fn send(self) {
        self.finish().send()
    }
}

/// Error emitted from [`Metric::parse_statsd`] for invalid metric strings.
#[derive(Debug)]
pub struct ParseMetricError(());

impl std::error::Error for ParseMetricError {}

impl fmt::Display for ParseMetricError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid metric string")
    }
}

fn parse_metric_opt(string: &str) -> Option<Metric> {
    let mut components = string.split('|');

    let (mri_str, value_str) = components.next()?.split_once(':')?;
    let (name, unit) = match mri_str.split_once('@') {
        Some((name, unit_str)) => (name, unit_str.parse().ok()?),
        None => (mri_str, MetricUnit::None),
    };

    let ty = components.next().and_then(|s| s.parse().ok())?;
    let value = match ty {
        MetricType::Counter => MetricValue::Counter(value_str.parse().ok()?),
        MetricType::Distribution => MetricValue::Distribution(value_str.parse().ok()?),
        MetricType::Set => MetricValue::Set(value_str.parse().ok()?),
        MetricType::Gauge => {
            // Gauge values are serialized as `last:min:max:sum:count`. We want to be able
            // to parse those strings back, so we just take the first colon-separated segment.
            let value_str = value_str.split(':').next().unwrap();
            MetricValue::Gauge(value_str.parse().ok()?)
        }
    };

    let mut builder = Metric::build(name.to_owned(), value).with_unit(unit);

    for component in components {
        if let Some('#') = component.chars().next() {
            for pair in component.get(1..)?.split(',') {
                let mut key_value = pair.splitn(2, ':');

                let key = key_value.next()?.to_owned();
                let value = key_value.next().unwrap_or_default().to_owned();

                builder = builder.with_tag(key, value);
            }
        }
    }

    Some(builder.finish())
}

/// Composite bucket key for [`BucketMap`].
#[derive(Debug, PartialEq, Eq, Hash)]
struct BucketKey {
    ty: MetricType,
    name: MetricStr,
    unit: MetricUnit,
    tags: BTreeMap<MetricStr, MetricStr>,
}

/// UNIX timestamp used for buckets.
type Timestamp = u64;

/// A nested map storing metric buckets.
///
/// This map consists of two levels:
///  1. The rounded UNIX timestamp of buckets.
///  2. The metric buckets themselves with a corresponding timestamp.
///
/// This structure allows for efficient dequeueing of buckets that are older than a certain
/// threshold. The buckets are dequeued in order of their timestamp, so the oldest buckets are
/// dequeued first.
type BucketMap = BTreeMap<Timestamp, HashMap<BucketKey, BucketValue>>;

#[derive(Debug)]
struct SharedAggregatorState {
    buckets: BucketMap,
    weight: usize,
    running: bool,
    force_flush: bool,
}

impl SharedAggregatorState {
    pub fn new() -> Self {
        Self {
            buckets: BTreeMap::new(),
            weight: 0,
            running: true,
            force_flush: false,
        }
    }

    /// Adds a new bucket to the aggregator.
    ///
    /// The bucket timestamp is rounded to the nearest bucket interval. Note that this does NOT
    /// automatically flush the aggregator if the weight exceeds the weight threshold.
    pub fn add(&mut self, mut timestamp: Timestamp, key: BucketKey, value: MetricValue) {
        // Floor timestamp to bucket interval
        timestamp /= BUCKET_INTERVAL.as_secs();
        timestamp *= BUCKET_INTERVAL.as_secs();

        match self.buckets.entry(timestamp).or_default().entry(key) {
            Entry::Occupied(mut e) => self.weight += e.get_mut().insert(value),
            Entry::Vacant(e) => self.weight += e.insert(value.into()).weight(),
        }
    }

    /// Removes and returns all buckets that are ready to flush.
    ///
    /// Buckets are ready to flush as soon as their time window has closed. For example, a bucket
    /// from timestamps `[4600, 4610)` is ready to flush immediately at `4610`.
    pub fn take_buckets(&mut self) -> BucketMap {
        if self.force_flush || !self.running {
            self.weight = 0;
            self.force_flush = false;
            std::mem::take(&mut self.buckets)
        } else {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .saturating_sub(BUCKET_INTERVAL)
                .as_secs();

            // Split all buckets after the cutoff time. `split` contains newer buckets, which should
            // remain, so swap them. After the swap, `split` contains all older buckets.
            let mut split = self.buckets.split_off(&timestamp);
            std::mem::swap(&mut split, &mut self.buckets);

            self.weight -= split
                .values()
                .flat_map(|map| map.values())
                .map(|bucket| bucket.weight())
                .sum::<usize>();

            split
        }
    }

    pub fn weight(&self) -> usize {
        self.weight
    }
}

type TagMap = BTreeMap<MetricStr, MetricStr>;

fn get_default_tags(options: &ClientOptions) -> TagMap {
    let mut tags = TagMap::new();
    if let Some(ref release) = options.release {
        tags.insert("release".into(), release.clone());
    }
    tags.insert(
        "environment".into(),
        options
            .environment
            .clone()
            .filter(|e| !e.is_empty())
            .unwrap_or(Cow::Borrowed("production")),
    );
    tags
}

#[derive(Clone)]
struct Worker {
    shared: Arc<Mutex<SharedAggregatorState>>,
    default_tags: TagMap,
    transport: TransportArc,
}

impl Worker {
    pub fn run(self) {
        loop {
            // Park instead of sleep so we can wake the thread up. Do not account for delays during
            // flushing, since we benefit from some drift to spread out metric submissions.
            thread::park_timeout(FLUSH_INTERVAL);

            let buckets = {
                let mut guard = self.shared.lock().unwrap();
                if !guard.running {
                    break;
                }
                guard.take_buckets()
            };

            self.flush_buckets(buckets);
        }
    }

    pub fn flush_buckets(&self, buckets: BucketMap) {
        if buckets.is_empty() {
            return;
        }

        // The transport is usually available when flush is called. Prefer a short lock and worst
        // case throw away the result rather than blocking the transport for too long.
        if let Ok(output) = self.format_payload(buckets) {
            let mut envelope = Envelope::new();
            envelope.add_item(EnvelopeItem::Statsd(output));

            if let Some(ref transport) = *self.transport.read().unwrap() {
                transport.send_envelope(envelope);
            }
        }
    }

    fn format_payload(&self, buckets: BucketMap) -> std::io::Result<Vec<u8>> {
        use std::io::Write;
        let mut out = vec![];

        for (timestamp, buckets) in buckets {
            for (key, value) in buckets {
                write!(
                    &mut out,
                    "{}",
                    normalization::normalize_name(key.name.as_ref())
                )?;
                match key.unit {
                    MetricUnit::Custom(u) => {
                        write!(&mut out, "@{}", normalization::normalize_unit(u.as_ref()))?
                    }
                    _ => write!(&mut out, "@{}", key.unit)?,
                }
                match value {
                    BucketValue::Counter(c) => {
                        write!(&mut out, ":{}", c)?;
                    }
                    BucketValue::Distribution(d) => {
                        for v in d {
                            write!(&mut out, ":{}", v)?;
                        }
                    }
                    BucketValue::Set(s) => {
                        for v in s {
                            write!(&mut out, ":{}", v)?;
                        }
                    }
                    BucketValue::Gauge(g) => {
                        write!(
                            &mut out,
                            ":{}:{}:{}:{}:{}",
                            g.last, g.min, g.max, g.sum, g.count
                        )?;
                    }
                }

                write!(&mut out, "|{}", key.ty.as_str())?;
                let normalized_tags =
                    normalization::normalize_tags(&key.tags).with_default_tags(&self.default_tags);
                write!(&mut out, "|#{}", normalized_tags)?;
                writeln!(&mut out, "|T{}", timestamp)?;
            }
        }

        Ok(out)
    }
}

impl fmt::Debug for Worker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Worker")
            .field("transport", &format_args!("ArcTransport"))
            .field("default_tags", &self.default_tags)
            .finish()
    }
}

#[derive(Debug)]
pub(crate) struct MetricAggregator {
    local_worker: Worker,
    handle: Option<JoinHandle<()>>,
}

impl MetricAggregator {
    pub fn new(transport: TransportArc, options: &ClientOptions) -> Self {
        let worker = Worker {
            shared: Arc::new(Mutex::new(SharedAggregatorState::new())),
            default_tags: get_default_tags(options),
            transport,
        };

        let local_worker = worker.clone();

        let handle = thread::Builder::new()
            .name("sentry-metrics".into())
            .spawn(move || worker.run())
            .expect("failed to spawn thread");

        Self {
            local_worker,
            handle: Some(handle),
        }
    }

    pub fn add(&self, metric: Metric) {
        let Metric {
            name,
            unit,
            value,
            tags,
            time,
        } = metric;

        let timestamp = time
            .unwrap_or_else(SystemTime::now)
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let key = BucketKey {
            ty: value.ty(),
            name,
            unit,
            tags,
        };

        let mut guard = self.local_worker.shared.lock().unwrap();
        guard.add(timestamp, key, value);

        if guard.weight() > MAX_WEIGHT {
            if let Some(ref handle) = self.handle {
                guard.force_flush = true;
                handle.thread().unpark();
            }
        }
    }

    pub fn flush(&self) {
        let buckets = {
            let mut guard = self.local_worker.shared.lock().unwrap();
            guard.force_flush = true;
            guard.take_buckets()
        };

        self.local_worker.flush_buckets(buckets);
    }
}

impl Drop for MetricAggregator {
    fn drop(&mut self) {
        let buckets = {
            let mut guard = self.local_worker.shared.lock().unwrap();
            guard.running = false;
            guard.take_buckets()
        };

        self.local_worker.flush_buckets(buckets);

        if let Some(handle) = self.handle.take() {
            handle.thread().unpark();
            handle.join().unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test::{with_captured_envelopes, with_captured_envelopes_options};
    use crate::ClientOptions;

    use super::*;

    /// Returns the current system time and rounded bucket timestamp.
    fn current_time() -> (SystemTime, u64) {
        let now = SystemTime::now();
        let timestamp = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let timestamp = timestamp / 10 * 10;

        (now, timestamp)
    }

    fn get_single_metrics(envelopes: &[Envelope]) -> &str {
        assert_eq!(envelopes.len(), 1, "expected exactly one envelope");

        let mut items = envelopes[0].items();
        let Some(EnvelopeItem::Statsd(payload)) = items.next() else {
            panic!("expected metrics item");
        };

        std::str::from_utf8(payload).unwrap().trim()
    }

    #[test]
    fn test_tags() {
        let (time, ts) = current_time();

        let envelopes = with_captured_envelopes(|| {
            Metric::count("my.metric")
                .with_tag("foo", "bar")
                .with_tag("and", "more")
                .with_time(time)
                .send();
        });

        let metrics = get_single_metrics(&envelopes);
        assert_eq!(
            metrics,
            format!("my.metric@none:1|c|#and:more,environment:production,foo:bar|T{ts}")
        );
    }

    #[test]
    fn test_unit() {
        let (time, ts) = current_time();

        let envelopes = with_captured_envelopes(|| {
            Metric::count("my.metric")
                .with_time(time)
                .with_unit("custom")
                .send();
        });

        let metrics = get_single_metrics(&envelopes);
        assert_eq!(
            metrics,
            format!("my.metric@custom:1|c|#environment:production|T{ts}")
        );
    }

    #[test]
    fn test_metric_sanitation() {
        let (time, ts) = current_time();

        let envelopes = with_captured_envelopes(|| {
            Metric::count("my$$$metric").with_time(time).send();
        });

        let metrics = get_single_metrics(&envelopes);
        assert_eq!(
            metrics,
            format!("my___metric@none:1|c|#environment:production|T{ts}")
        );
    }

    #[test]
    fn test_tag_sanitation() {
        let (time, ts) = current_time();

        let envelopes = with_captured_envelopes(|| {
            Metric::count("my.metric")
                .with_tag("foo-bar$$$blub", "%$föö{}")
                .with_time(time)
                .send();
        });

        let metrics = get_single_metrics(&envelopes);
        assert_eq!(
            metrics,
            format!("my.metric@none:1|c|#environment:production,foo-barblub:%$föö{{}}|T{ts}")
        );
    }

    #[test]
    fn test_default_tags() {
        let (time, ts) = current_time();

        let options = ClientOptions {
            release: Some("myapp@1.0.0".into()),
            environment: Some("development".into()),
            ..Default::default()
        };

        let envelopes = with_captured_envelopes_options(
            || {
                Metric::count("requests")
                    .with_tag("foo", "bar")
                    .with_time(time)
                    .send();
            },
            options,
        );

        let metrics = get_single_metrics(&envelopes);
        assert_eq!(
            metrics,
            format!("requests@none:1|c|#environment:development,foo:bar,release:myapp@1.0.0|T{ts}")
        );
    }

    #[test]
    fn test_empty_default_tags() {
        let (time, ts) = current_time();
        let options = ClientOptions {
            release: Some("".into()),
            environment: Some("".into()),
            ..Default::default()
        };

        let envelopes = with_captured_envelopes_options(
            || {
                Metric::count("requests")
                    .with_tag("foo", "bar")
                    .with_time(time)
                    .send();
            },
            options,
        );

        let metrics = get_single_metrics(&envelopes);
        assert_eq!(
            metrics,
            format!("requests@none:1|c|#environment:production,foo:bar|T{ts}")
        );
    }

    #[test]
    fn test_override_default_tags() {
        let (time, ts) = current_time();
        let options = ClientOptions {
            release: Some("default_release".into()),
            environment: Some("default_env".into()),
            ..Default::default()
        };

        let envelopes = with_captured_envelopes_options(
            || {
                Metric::count("requests")
                    .with_tag("environment", "custom_env")
                    .with_tag("release", "custom_release")
                    .with_time(time)
                    .send();
            },
            options,
        );

        let metrics = get_single_metrics(&envelopes);
        assert_eq!(
            metrics,
            format!("requests@none:1|c|#environment:custom_env,release:custom_release|T{ts}")
        );
    }

    #[test]
    fn test_counter() {
        let (time, ts) = current_time();

        let envelopes = with_captured_envelopes(|| {
            Metric::count("my.metric").with_time(time).send();
            Metric::incr("my.metric", 2.0).with_time(time).send();
        });

        let metrics = get_single_metrics(&envelopes);
        assert_eq!(
            metrics,
            format!("my.metric@none:3|c|#environment:production|T{ts}")
        );
    }

    #[test]
    fn test_timing() {
        let (time, ts) = current_time();

        let envelopes = with_captured_envelopes(|| {
            Metric::timing("my.metric", Duration::from_millis(200))
                .with_time(time)
                .send();
            Metric::timing("my.metric", Duration::from_millis(100))
                .with_time(time)
                .send();
        });

        let metrics = get_single_metrics(&envelopes);
        assert_eq!(
            metrics,
            format!("my.metric@second:0.2:0.1|d|#environment:production|T{ts}")
        );
    }

    #[test]
    fn test_distribution() {
        let (time, ts) = current_time();

        let envelopes = with_captured_envelopes(|| {
            Metric::distribution("my.metric", 2.0)
                .with_time(time)
                .send();
            Metric::distribution("my.metric", 1.0)
                .with_time(time)
                .send();
        });

        let metrics = get_single_metrics(&envelopes);
        assert_eq!(
            metrics,
            format!("my.metric@none:2:1|d|#environment:production|T{ts}")
        );
    }

    #[test]
    fn test_set() {
        let (time, ts) = current_time();

        let envelopes = with_captured_envelopes(|| {
            Metric::set("my.metric", "hello").with_time(time).send();
            // Duplicate that should not be reflected twice
            Metric::set("my.metric", "hello").with_time(time).send();
            Metric::set("my.metric", "world").with_time(time).send();
        });

        let metrics = get_single_metrics(&envelopes);
        assert_eq!(
            metrics,
            format!("my.metric@none:907060870:980881731|s|#environment:production|T{ts}")
        );
    }

    #[test]
    fn test_gauge() {
        let (time, ts) = current_time();

        let envelopes = with_captured_envelopes(|| {
            Metric::gauge("my.metric", 2.0).with_time(time).send();
            Metric::gauge("my.metric", 1.0).with_time(time).send();
            Metric::gauge("my.metric", 1.5).with_time(time).send();
        });

        let metrics = get_single_metrics(&envelopes);
        assert_eq!(
            metrics,
            format!("my.metric@none:1.5:1:2:4.5:3|g|#environment:production|T{ts}")
        );
    }

    #[test]
    fn test_multiple() {
        let (time, ts) = current_time();

        let envelopes = with_captured_envelopes(|| {
            Metric::count("my.metric").with_time(time).send();
            Metric::distribution("my.dist", 2.0).with_time(time).send();
        });

        let metrics = get_single_metrics(&envelopes);
        println!("{metrics}");

        assert!(metrics.contains(&format!("my.metric@none:1|c|#environment:production|T{ts}")));
        assert!(metrics.contains(&format!("my.dist@none:2|d|#environment:production|T{ts}")));
    }

    #[test]
    fn test_regression_parse_statsd() {
        let payload = "docker.net.bytes_rcvd:27763.20237096717:27763.20237096717:27763.20237096717:27763.20237096717:1|g|#container_id:97df61f5c55b58ec9c04da3e03edc8a875ec90eb405eb5645ad9a86d0a7cd3ee,container_name:app_sidekiq_1";
        let metric = Metric::parse_statsd(payload).unwrap();
        assert_eq!(metric.name, "docker.net.bytes_rcvd");
        assert_eq!(metric.value, MetricValue::Gauge(27763.20237096717));
        assert_eq!(
            metric.tags["container_id"],
            "97df61f5c55b58ec9c04da3e03edc8a875ec90eb405eb5645ad9a86d0a7cd3ee"
        );
        assert_eq!(metric.tags["container_name"], "app_sidekiq_1");
    }
}
