mod rate_limit;
#[allow(dead_code)]
mod remote;
mod sampler;
mod sampling_strategy;

pub use sampler::{JaegerRemoteSampler, JaegerRemoteSamplerBuilder};

#[cfg(test)]
mod tests {}
