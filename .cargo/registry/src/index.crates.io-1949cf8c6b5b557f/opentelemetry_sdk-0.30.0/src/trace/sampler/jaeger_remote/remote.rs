/// Generate types based on proto
/// ProbabilisticSamplingStrategy samples traces with a fixed probability.
#[derive(serde::Serialize, serde::Deserialize, PartialOrd, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProbabilisticSamplingStrategy {
    /// samplingRate is the sampling probability in the range [0.0, 1.0].
    pub(crate) sampling_rate: f64,
}

/// RateLimitingSamplingStrategy samples a fixed number of traces per time interval.
/// The typical implementations use the leaky bucket algorithm.
#[derive(serde::Serialize, serde::Deserialize, PartialOrd, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RateLimitingSamplingStrategy {
    /// TODO this field type should be changed to double, to support rates like 1 per minute.
    pub(crate) max_traces_per_second: i32,
}

/// OperationSamplingStrategy is a sampling strategy for a given operation
/// (aka endpoint, span name). Only probabilistic sampling is currently supported.
#[derive(serde::Serialize, serde::Deserialize, PartialOrd, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OperationSamplingStrategy {
    pub(crate) operation: String,
    pub(crate) probabilistic_sampling: ProbabilisticSamplingStrategy,
}

/// PerOperationSamplingStrategies is a combination of strategies for different endpoints
/// as well as some service-wide defaults. It is particularly useful for services whose
/// endpoints receive vastly different traffic, so that any single rate of sampling would
/// result in either too much data for some endpoints or almost no data for other endpoints.
#[derive(serde::Serialize, serde::Deserialize, PartialOrd, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PerOperationSamplingStrategies {
    /// defaultSamplingProbability is the sampling probability for spans that do not match
    /// any of the perOperationStrategies.
    pub(crate) default_sampling_probability: f64,
    /// defaultLowerBoundTracesPerSecond defines a lower-bound rate limit used to ensure that
    /// there is some minimal amount of traces sampled for an endpoint that might otherwise
    /// be never sampled via probabilistic strategies. The limit is local to a service instance,
    /// so if a service is deployed with many (N) instances, the effective minimum rate of sampling
    /// will be N times higher. This setting applies to ALL operations, whether or not they match
    /// one of the perOperationStrategies.
    pub(crate) default_lower_bound_traces_per_second: f64,
    /// perOperationStrategies describes sampling strategiesf for individual operations within
    /// a given service.
    pub(crate) per_operation_strategies: Vec<OperationSamplingStrategy>,
    /// defaultUpperBoundTracesPerSecond defines an upper bound rate limit.
    /// However, almost no Jaeger SDKs support this parameter.
    pub(crate) default_upper_bound_traces_per_second: f64,
}

/// SamplingStrategyResponse contains an overall sampling strategy for a given service.
/// This type should be treated as a union where only one of the strategy field is present.
#[derive(serde::Serialize, serde::Deserialize, PartialOrd, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SamplingStrategyResponse {
    /// Legacy field that was meant to indicate which one of the strategy fields
    /// below is present. This enum was not extended when per-operation strategy
    /// was introduced, because extending enum has backwards compatiblity issues.
    /// The recommended approach for consumers is to ignore this field and instead
    /// checks the other fields being not null (starting with operationSampling).
    /// For producers, it is recommended to set this field correctly for probabilistic
    /// and rate-limiting strategies, but if per-operation strategy is returned,
    /// the enum can be set to 0 (probabilistic).
    pub(crate) strategy_type: SamplingStrategyType,
    pub(crate) probabilistic_sampling: Option<ProbabilisticSamplingStrategy>,
    pub(crate) rate_limiting_sampling: Option<RateLimitingSamplingStrategy>,
    pub(crate) operation_sampling: Option<PerOperationSamplingStrategies>,
}

/// SamplingStrategyParameters defines request parameters for remote sampler.
#[derive(serde::Serialize, serde::Deserialize, PartialOrd, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SamplingStrategyParameters {
    /// serviceName is a required argument.
    pub(crate) service_name: String,
}

/// See description of the SamplingStrategyResponse.strategyType field.
#[derive(serde::Serialize, serde::Deserialize, PartialOrd, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub(crate) enum SamplingStrategyType {
    Probabilistic,
    RateLimiting,
}
