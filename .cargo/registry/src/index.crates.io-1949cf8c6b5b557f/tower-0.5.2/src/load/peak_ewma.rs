//! A `Load` implementation that measures load using the PeakEWMA response latency.

#[cfg(feature = "discover")]
use crate::discover::{Change, Discover};
#[cfg(feature = "discover")]
use futures_core::{ready, Stream};
#[cfg(feature = "discover")]
use pin_project_lite::pin_project;
#[cfg(feature = "discover")]
use std::pin::Pin;

use super::completion::{CompleteOnResponse, TrackCompletion, TrackCompletionFuture};
use super::Load;
use std::task::{Context, Poll};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::Instant;
use tower_service::Service;
use tracing::trace;

/// Measures the load of the underlying service using Peak-EWMA load measurement.
///
/// [`PeakEwma`] implements [`Load`] with the [`Cost`] metric that estimates the amount of
/// pending work to an endpoint. Work is calculated by multiplying the
/// exponentially-weighted moving average (EWMA) of response latencies by the number of
/// pending requests. The Peak-EWMA algorithm is designed to be especially sensitive to
/// worst-case latencies. Over time, the peak latency value decays towards the moving
/// average of latencies to the endpoint.
///
/// When no latency information has been measured for an endpoint, an arbitrary default
/// RTT of 1 second is used to prevent the endpoint from being overloaded before a
/// meaningful baseline can be established..
///
/// ## Note
///
/// This is derived from [Finagle][finagle], which is distributed under the Apache V2
/// license. Copyright 2017, Twitter Inc.
///
/// [finagle]:
/// https://github.com/twitter/finagle/blob/9cc08d15216497bb03a1cafda96b7266cfbbcff1/finagle-core/src/main/scala/com/twitter/finagle/loadbalancer/PeakEwma.scala
#[derive(Debug)]
pub struct PeakEwma<S, C = CompleteOnResponse> {
    service: S,
    decay_ns: f64,
    rtt_estimate: Arc<Mutex<RttEstimate>>,
    completion: C,
}

#[cfg(feature = "discover")]
pin_project! {
    /// Wraps a `D`-typed stream of discovered services with `PeakEwma`.
    #[cfg_attr(docsrs, doc(cfg(feature = "discover")))]
    #[derive(Debug)]
    pub struct PeakEwmaDiscover<D, C = CompleteOnResponse> {
        #[pin]
        discover: D,
        decay_ns: f64,
        default_rtt: Duration,
        completion: C,
    }
}

/// Represents the relative cost of communicating with a service.
///
/// The underlying value estimates the amount of pending work to a service: the Peak-EWMA
/// latency estimate multiplied by the number of pending requests.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Cost(f64);

/// Tracks an in-flight request and updates the RTT-estimate on Drop.
#[derive(Debug)]
pub struct Handle {
    sent_at: Instant,
    decay_ns: f64,
    rtt_estimate: Arc<Mutex<RttEstimate>>,
}

/// Holds the current RTT estimate and the last time this value was updated.
#[derive(Debug)]
struct RttEstimate {
    update_at: Instant,
    rtt_ns: f64,
}

const NANOS_PER_MILLI: f64 = 1_000_000.0;

// ===== impl PeakEwma =====

impl<S, C> PeakEwma<S, C> {
    /// Wraps an `S`-typed service so that its load is tracked by the EWMA of its peak latency.
    pub fn new(service: S, default_rtt: Duration, decay_ns: f64, completion: C) -> Self {
        debug_assert!(decay_ns > 0.0, "decay_ns must be positive");
        Self {
            service,
            decay_ns,
            rtt_estimate: Arc::new(Mutex::new(RttEstimate::new(nanos(default_rtt)))),
            completion,
        }
    }

    fn handle(&self) -> Handle {
        Handle {
            decay_ns: self.decay_ns,
            sent_at: Instant::now(),
            rtt_estimate: self.rtt_estimate.clone(),
        }
    }
}

impl<S, C, Request> Service<Request> for PeakEwma<S, C>
where
    S: Service<Request>,
    C: TrackCompletion<Handle, S::Response>,
{
    type Response = C::Output;
    type Error = S::Error;
    type Future = TrackCompletionFuture<S::Future, C, Handle>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        TrackCompletionFuture::new(
            self.completion.clone(),
            self.handle(),
            self.service.call(req),
        )
    }
}

impl<S, C> Load for PeakEwma<S, C> {
    type Metric = Cost;

    fn load(&self) -> Self::Metric {
        let pending = Arc::strong_count(&self.rtt_estimate) as u32 - 1;

        // Update the RTT estimate to account for decay since the last update.
        // If an estimate has not been established, a default is provided
        let estimate = self.update_estimate();

        let cost = Cost(estimate * f64::from(pending + 1));
        trace!(
            "load estimate={:.0}ms pending={} cost={:?}",
            estimate / NANOS_PER_MILLI,
            pending,
            cost,
        );
        cost
    }
}

impl<S, C> PeakEwma<S, C> {
    fn update_estimate(&self) -> f64 {
        let mut rtt = self.rtt_estimate.lock().expect("peak ewma prior_estimate");
        rtt.decay(self.decay_ns)
    }
}

// ===== impl PeakEwmaDiscover =====

#[cfg(feature = "discover")]
impl<D, C> PeakEwmaDiscover<D, C> {
    /// Wraps a `D`-typed [`Discover`] so that services have a [`PeakEwma`] load metric.
    ///
    /// The provided `default_rtt` is used as the default RTT estimate for newly
    /// added services.
    ///
    /// They `decay` value determines over what time period a RTT estimate should
    /// decay.
    pub fn new<Request>(discover: D, default_rtt: Duration, decay: Duration, completion: C) -> Self
    where
        D: Discover,
        D::Service: Service<Request>,
        C: TrackCompletion<Handle, <D::Service as Service<Request>>::Response>,
    {
        PeakEwmaDiscover {
            discover,
            decay_ns: nanos(decay),
            default_rtt,
            completion,
        }
    }
}

#[cfg(feature = "discover")]
impl<D, C> Stream for PeakEwmaDiscover<D, C>
where
    D: Discover,
    C: Clone,
{
    type Item = Result<Change<D::Key, PeakEwma<D::Service, C>>, D::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let change = match ready!(this.discover.poll_discover(cx)).transpose()? {
            None => return Poll::Ready(None),
            Some(Change::Remove(k)) => Change::Remove(k),
            Some(Change::Insert(k, svc)) => {
                let peak_ewma = PeakEwma::new(
                    svc,
                    *this.default_rtt,
                    *this.decay_ns,
                    this.completion.clone(),
                );
                Change::Insert(k, peak_ewma)
            }
        };

        Poll::Ready(Some(Ok(change)))
    }
}

// ===== impl RttEstimate =====

impl RttEstimate {
    fn new(rtt_ns: f64) -> Self {
        debug_assert!(0.0 < rtt_ns, "rtt must be positive");
        Self {
            rtt_ns,
            update_at: Instant::now(),
        }
    }

    /// Decays the RTT estimate with a decay period of `decay_ns`.
    fn decay(&mut self, decay_ns: f64) -> f64 {
        // Updates with a 0 duration so that the estimate decays towards 0.
        let now = Instant::now();
        self.update(now, now, decay_ns)
    }

    /// Updates the Peak-EWMA RTT estimate.
    ///
    /// The elapsed time from `sent_at` to `recv_at` is added
    fn update(&mut self, sent_at: Instant, recv_at: Instant, decay_ns: f64) -> f64 {
        debug_assert!(
            sent_at <= recv_at,
            "recv_at={:?} after sent_at={:?}",
            recv_at,
            sent_at
        );
        let rtt = nanos(recv_at.saturating_duration_since(sent_at));

        let now = Instant::now();
        debug_assert!(
            self.update_at <= now,
            "update_at={:?} in the future",
            self.update_at
        );

        self.rtt_ns = if self.rtt_ns < rtt {
            // For Peak-EWMA, always use the worst-case (peak) value as the estimate for
            // subsequent requests.
            trace!(
                "update peak rtt={}ms prior={}ms",
                rtt / NANOS_PER_MILLI,
                self.rtt_ns / NANOS_PER_MILLI,
            );
            rtt
        } else {
            // When an RTT is observed that is less than the estimated RTT, we decay the
            // prior estimate according to how much time has elapsed since the last
            // update. The inverse of the decay is used to scale the estimate towards the
            // observed RTT value.
            let elapsed = nanos(now.saturating_duration_since(self.update_at));
            let decay = (-elapsed / decay_ns).exp();
            let recency = 1.0 - decay;
            let next_estimate = (self.rtt_ns * decay) + (rtt * recency);
            trace!(
                "update rtt={:03.0}ms decay={:06.0}ns; next={:03.0}ms",
                rtt / NANOS_PER_MILLI,
                self.rtt_ns - next_estimate,
                next_estimate / NANOS_PER_MILLI,
            );
            next_estimate
        };
        self.update_at = now;

        self.rtt_ns
    }
}

// ===== impl Handle =====

impl Drop for Handle {
    fn drop(&mut self) {
        let recv_at = Instant::now();

        if let Ok(mut rtt) = self.rtt_estimate.lock() {
            rtt.update(self.sent_at, recv_at, self.decay_ns);
        }
    }
}

// ===== impl Cost =====

// Utility that converts durations to nanos in f64.
//
// Due to a lossy transformation, the maximum value that can be represented is ~585 years,
// which, I hope, is more than enough to represent request latencies.
fn nanos(d: Duration) -> f64 {
    const NANOS_PER_SEC: u64 = 1_000_000_000;
    let n = f64::from(d.subsec_nanos());
    let s = d.as_secs().saturating_mul(NANOS_PER_SEC) as f64;
    n + s
}

#[cfg(test)]
mod tests {
    use futures_util::future;
    use std::time::Duration;
    use tokio::time;
    use tokio_test::{assert_ready, assert_ready_ok, task};

    use super::*;

    struct Svc;
    impl Service<()> for Svc {
        type Response = ();
        type Error = ();
        type Future = future::Ready<Result<(), ()>>;

        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), ()>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, (): ()) -> Self::Future {
            future::ok(())
        }
    }

    /// The default RTT estimate decays, so that new nodes are considered if the
    /// default RTT is too high.
    #[tokio::test]
    async fn default_decay() {
        time::pause();

        let svc = PeakEwma::new(
            Svc,
            Duration::from_millis(10),
            NANOS_PER_MILLI * 1_000.0,
            CompleteOnResponse,
        );
        let Cost(load) = svc.load();
        assert_eq!(load, 10.0 * NANOS_PER_MILLI);

        time::advance(Duration::from_millis(100)).await;
        let Cost(load) = svc.load();
        assert!(9.0 * NANOS_PER_MILLI < load && load < 10.0 * NANOS_PER_MILLI);

        time::advance(Duration::from_millis(100)).await;
        let Cost(load) = svc.load();
        assert!(8.0 * NANOS_PER_MILLI < load && load < 9.0 * NANOS_PER_MILLI);
    }

    // The default RTT estimate decays, so that new nodes are considered if the default RTT is too
    // high.
    #[tokio::test]
    async fn compound_decay() {
        time::pause();

        let mut svc = PeakEwma::new(
            Svc,
            Duration::from_millis(20),
            NANOS_PER_MILLI * 1_000.0,
            CompleteOnResponse,
        );
        assert_eq!(svc.load(), Cost(20.0 * NANOS_PER_MILLI));

        time::advance(Duration::from_millis(100)).await;
        let mut rsp0 = task::spawn(svc.call(()));
        assert!(svc.load() > Cost(20.0 * NANOS_PER_MILLI));

        time::advance(Duration::from_millis(100)).await;
        let mut rsp1 = task::spawn(svc.call(()));
        assert!(svc.load() > Cost(40.0 * NANOS_PER_MILLI));

        time::advance(Duration::from_millis(100)).await;
        let () = assert_ready_ok!(rsp0.poll());
        assert_eq!(svc.load(), Cost(400_000_000.0));

        time::advance(Duration::from_millis(100)).await;
        let () = assert_ready_ok!(rsp1.poll());
        assert_eq!(svc.load(), Cost(200_000_000.0));

        // Check that values decay as time elapses
        time::advance(Duration::from_secs(1)).await;
        assert!(svc.load() < Cost(100_000_000.0));

        time::advance(Duration::from_secs(10)).await;
        assert!(svc.load() < Cost(100_000.0));
    }

    #[test]
    fn nanos() {
        assert_eq!(super::nanos(Duration::new(0, 0)), 0.0);
        assert_eq!(super::nanos(Duration::new(0, 123)), 123.0);
        assert_eq!(super::nanos(Duration::new(1, 23)), 1_000_000_023.0);
        assert_eq!(
            super::nanos(Duration::new(::std::u64::MAX, 999_999_999)),
            18446744074709553000.0
        );
    }
}
