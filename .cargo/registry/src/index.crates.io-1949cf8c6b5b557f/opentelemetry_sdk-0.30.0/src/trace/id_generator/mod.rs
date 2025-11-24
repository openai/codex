use opentelemetry::trace::{SpanId, TraceId};
use rand::{rngs, Rng, SeedableRng};
use std::cell::RefCell;
use std::fmt;

/// Interface for generating IDs
pub trait IdGenerator: Send + Sync + fmt::Debug {
    /// Generate a new `TraceId`
    fn new_trace_id(&self) -> TraceId;

    /// Generate a new `SpanId`
    fn new_span_id(&self) -> SpanId;
}

/// Default [`IdGenerator`] implementation.
///
/// Generates Trace and Span ids using a random number generator.
#[derive(Clone, Debug, Default)]
pub struct RandomIdGenerator {
    _private: (),
}

impl IdGenerator for RandomIdGenerator {
    fn new_trace_id(&self) -> TraceId {
        CURRENT_RNG.with(|rng| TraceId::from(rng.borrow_mut().random::<u128>()))
    }

    fn new_span_id(&self) -> SpanId {
        CURRENT_RNG.with(|rng| SpanId::from(rng.borrow_mut().random::<u64>()))
    }
}

thread_local! {
    /// Store random number generator for each thread
    static CURRENT_RNG: RefCell<rngs::SmallRng> = RefCell::new(rngs::SmallRng::from_os_rng());
}
