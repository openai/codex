use crate::trace::sampler::jaeger_remote::remote::{
    PerOperationSamplingStrategies, ProbabilisticSamplingStrategy, RateLimitingSamplingStrategy,
    SamplingStrategyResponse,
};
use crate::trace::sampler::sample_based_on_probability;
use opentelemetry::trace::{
    SamplingDecision, SamplingResult, TraceContextExt, TraceId, TraceState,
};
use opentelemetry::{otel_warn, Context};
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::sync::Mutex;

use super::rate_limit::LeakyBucket;

// todo: remove the mutex as probabilistic doesn't require mutable ref
// sampling strategy that sent by remote agents or collectors.
enum Strategy {
    // probability to sample between [0.0, 1.0]
    Probabilistic(f64),
    //maxTracesPerSecond
    RateLimiting(LeakyBucket),
    PerOperation(PerOperationStrategies),
}

pub(crate) struct Inner {
    strategy: Mutex<Option<Strategy>>,
    // initial configuration for leaky bucket
    leaky_bucket_size: f64,
    shut_down: futures_channel::mpsc::Sender<()>,
}

impl Debug for Inner {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        //todo: add more debug information
        f.debug_struct("JaegerRemoteSamplerInner")
            .field("leaky_bucket_size", &self.leaky_bucket_size)
            .finish()
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        let _ = self.shut_down.try_send(());
    }
}

impl Inner {
    pub(crate) fn new(
        leaky_bucket_size: f64,
        shut_down: futures_channel::mpsc::Sender<()>,
    ) -> Self {
        Inner {
            strategy: Mutex::new(None),
            leaky_bucket_size,
            shut_down,
        }
    }

    pub(crate) fn update(&self, remote_strategy_resp: SamplingStrategyResponse) {
        self.strategy
            .lock()
            .map(|mut old_strategy_opt| {
                *old_strategy_opt = match old_strategy_opt.take() {
                    Some(mut old_strategy) => {
                        // update sample strategy
                        // the response should be an union type where
                        // - operation_sampling
                        // - rate_limiting_sampling
                        // - probabilistic_sampling
                        // are mutually exclusive.
                        match (
                            remote_strategy_resp.operation_sampling,
                            remote_strategy_resp.rate_limiting_sampling,
                            remote_strategy_resp.probabilistic_sampling,
                            &mut old_strategy,
                        ) {
                            (
                                None,
                                Some(rate_limiting),
                                None,
                                Strategy::RateLimiting(leaky_bucket),
                            ) => {
                                leaky_bucket.update(rate_limiting.max_traces_per_second as f64);
                                // in the future the remote response may support f64
                                Some(old_strategy)
                            }
                            // only leaky bucket is a stateful sampler, meaning it's update is different from the initialization
                            // for other sampler, we can just re-init it
                            (
                                operation_sampling,
                                rate_limiting_sampling,
                                probabilistic_sampling,
                                _,
                            ) => self.init_strategy(
                                operation_sampling,
                                rate_limiting_sampling,
                                probabilistic_sampling,
                            ),
                        }
                    }
                    None => self.init_strategy(
                        remote_strategy_resp.operation_sampling,
                        remote_strategy_resp.rate_limiting_sampling,
                        remote_strategy_resp.probabilistic_sampling,
                    ),
                }
            })
            .unwrap_or_else(|_err| {
                otel_warn!(
                    name: "JaegerRemoteSampler.MutexPoisoned",
                    message = "Unable to update Jaeger Remote sampling strategy: the sampler's internal mutex is poisoned, likely due to a panic in another thread holding the lock. No further attempts to update the strategy will be made until the application or process restarts, and the last known configuration will continue to be used.",
                );
            });
    }

    fn init_strategy(
        &self,
        operation_sampling: Option<PerOperationSamplingStrategies>,
        rate_limiting_sampling: Option<RateLimitingSamplingStrategy>,
        probabilistic_sampling: Option<ProbabilisticSamplingStrategy>,
    ) -> Option<Strategy> {
        match (
            operation_sampling,
            rate_limiting_sampling,
            probabilistic_sampling,
        ) {
            (Some(op_sampling), _, _) => {
                // ops sampling
                let mut per_ops_sampling = PerOperationStrategies::default();
                per_ops_sampling.update(op_sampling);
                Some(Strategy::PerOperation(per_ops_sampling))
            }
            (_, Some(rate_limiting), _) => Some(Strategy::RateLimiting(LeakyBucket::new(
                self.leaky_bucket_size,
                rate_limiting.max_traces_per_second as f64,
            ))),
            (_, _, Some(probabilistic)) => {
                Some(Strategy::Probabilistic(probabilistic.sampling_rate))
            }
            _ => {
                otel_warn!(
                    name: "JaegerRemoteSampler.InvalidStrategyReceived",
                    message = "Invalid sampling strategy received from the remote endpoint. Expected one of: OperationSampling, RateLimitingSampling, or ProbabilisticSampling. Continuing to use the previous strategy or default sampler until a successful update.",
                );
                None
            }
        }
    }

    pub(crate) fn should_sample(
        &self,
        parent_context: Option<&Context>,
        trace_id: TraceId,
        name: &str,
    ) -> Option<SamplingResult> {
        self.strategy
            .lock()
            .map(|mut inner_opt| match inner_opt.as_mut() {
                Some(inner) => {
                    let decision = match inner {
                        Strategy::RateLimiting(leaky_bucket) => {
                            if leaky_bucket.should_sample() {
                                SamplingDecision::RecordAndSample
                            } else {
                                SamplingDecision::Drop
                            }
                        }
                        Strategy::Probabilistic(prob) => {
                            sample_based_on_probability(prob, trace_id)
                        }
                        Strategy::PerOperation(per_operation_strategies) => {
                            sample_based_on_probability(
                                &per_operation_strategies.get_probability(name),
                                trace_id,
                            )
                        }
                    };

                    Some(SamplingResult {
                        decision,
                        attributes: Vec::new(),
                        trace_state: match parent_context {
                            Some(ctx) => ctx.span().span_context().trace_state().clone(),
                            None => TraceState::default(),
                        },
                    })
                }
                None => None,
            })
            .unwrap_or_else(|_| None)
    }
}

#[derive(Default)]
pub(crate) struct PerOperationStrategies {
    default_prob: f64,
    default_lower_bound_traces_per_second: f64,
    operation_prob: HashMap<String, f64>,
    // todo: guarantee the throughput using lower bound and upper bound
    default_upper_bound_traces_per_second: f64,
}

impl PerOperationStrategies {
    pub(crate) fn update(&mut self, remote_strategies: PerOperationSamplingStrategies) {
        self.default_prob = remote_strategies.default_sampling_probability;
        self.default_lower_bound_traces_per_second =
            remote_strategies.default_lower_bound_traces_per_second;
        self.default_upper_bound_traces_per_second =
            remote_strategies.default_upper_bound_traces_per_second;

        self.operation_prob = remote_strategies
            .per_operation_strategies
            .into_iter()
            .map(|op_strategy| {
                (
                    op_strategy.operation,
                    op_strategy.probabilistic_sampling.sampling_rate,
                )
            })
            .collect();
    }

    pub(crate) fn get_probability(&self, operation: &str) -> f64 {
        *self
            .operation_prob
            .get(operation)
            .unwrap_or(&self.default_prob)
    }
}

#[cfg(test)]
mod tests {}
