//! Exercises load balancers with mocked services.

use futures_core::{Stream, TryStream};
use futures_util::{stream, stream::StreamExt, stream::TryStreamExt};
use hdrhistogram::Histogram;
use pin_project_lite::pin_project;
use rand::{self, Rng};
use std::hash::Hash;
use std::time::Duration;
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tokio::time::{self, Instant};
use tower::balance as lb;
use tower::discover::{Change, Discover};
use tower::limit::concurrency::ConcurrencyLimit;
use tower::load;
use tower::util::ServiceExt;
use tower_service::Service;

const REQUESTS: usize = 100_000;
const CONCURRENCY: usize = 500;
const DEFAULT_RTT: Duration = Duration::from_millis(30);
static ENDPOINT_CAPACITY: usize = CONCURRENCY;
static MAX_ENDPOINT_LATENCIES: [Duration; 10] = [
    Duration::from_millis(1),
    Duration::from_millis(5),
    Duration::from_millis(10),
    Duration::from_millis(10),
    Duration::from_millis(10),
    Duration::from_millis(100),
    Duration::from_millis(100),
    Duration::from_millis(100),
    Duration::from_millis(500),
    Duration::from_millis(1000),
];

struct Summary {
    latencies: Histogram<u64>,
    start: Instant,
    count_by_instance: [usize; 10],
}

#[tokio::main]
async fn main() {
    tracing::subscriber::set_global_default(tracing_subscriber::FmtSubscriber::default()).unwrap();

    println!("REQUESTS={}", REQUESTS);
    println!("CONCURRENCY={}", CONCURRENCY);
    println!("ENDPOINT_CAPACITY={}", ENDPOINT_CAPACITY);
    print!("MAX_ENDPOINT_LATENCIES=[");
    for max in &MAX_ENDPOINT_LATENCIES {
        let l = max.as_secs() * 1_000 + u64::from(max.subsec_millis());
        print!("{}ms, ", l);
    }
    println!("]");

    let decay = Duration::from_secs(10);
    let d = gen_disco();
    let pe = lb::p2c::Balance::new(load::PeakEwmaDiscover::new(
        d,
        DEFAULT_RTT,
        decay,
        load::CompleteOnResponse::default(),
    ));
    run("P2C+PeakEWMA...", pe).await;

    let d = gen_disco();
    let ll = lb::p2c::Balance::new(load::PendingRequestsDiscover::new(
        d,
        load::CompleteOnResponse::default(),
    ));
    run("P2C+LeastLoaded...", ll).await;
}

type Error = Box<dyn std::error::Error + Send + Sync>;

type Key = usize;

pin_project! {
    struct Disco<S> {
        services: Vec<(Key, S)>
    }
}

impl<S> Disco<S> {
    fn new(services: Vec<(Key, S)>) -> Self {
        Self { services }
    }
}

impl<S> Stream for Disco<S>
where
    S: Service<Req, Response = Rsp, Error = Error>,
{
    type Item = Result<Change<Key, S>, Error>;

    fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.project().services.pop() {
            Some((k, service)) => Poll::Ready(Some(Ok(Change::Insert(k, service)))),
            None => {
                // there may be more later
                Poll::Pending
            }
        }
    }
}

fn gen_disco() -> impl Discover<
    Key = Key,
    Error = Error,
    Service = ConcurrencyLimit<
        impl Service<Req, Response = Rsp, Error = Error, Future = impl Send> + Send,
    >,
> + Send {
    Disco::new(
        MAX_ENDPOINT_LATENCIES
            .iter()
            .enumerate()
            .map(|(instance, latency)| {
                let svc = tower::service_fn(move |_| {
                    let start = Instant::now();

                    let maxms = u64::from(latency.subsec_millis())
                        .saturating_add(latency.as_secs().saturating_mul(1_000));
                    let latency = Duration::from_millis(rand::thread_rng().gen_range(0..maxms));

                    async move {
                        time::sleep_until(start + latency).await;
                        let latency = start.elapsed();
                        Ok(Rsp { latency, instance })
                    }
                });

                (instance, ConcurrencyLimit::new(svc, ENDPOINT_CAPACITY))
            })
            .collect(),
    )
}

async fn run<D>(name: &'static str, lb: lb::p2c::Balance<D, Req>)
where
    D: Discover + Unpin + Send + 'static,
    D::Error: Into<Error>,
    D::Key: Clone + Send + Hash,
    D::Service: Service<Req, Response = Rsp> + load::Load + Send,
    <D::Service as Service<Req>>::Error: Into<Error>,
    <D::Service as Service<Req>>::Future: Send,
    <D::Service as load::Load>::Metric: std::fmt::Debug,
{
    println!("{}", name);

    let requests = stream::repeat(Req).take(REQUESTS);
    let service = ConcurrencyLimit::new(lb, CONCURRENCY);
    let responses = service.call_all(requests).unordered();

    compute_histo(responses).await.unwrap().report();
}

async fn compute_histo<S>(mut times: S) -> Result<Summary, Error>
where
    S: TryStream<Ok = Rsp, Error = Error> + 'static + Unpin,
{
    let mut summary = Summary::new();
    while let Some(rsp) = times.try_next().await? {
        summary.count(rsp);
    }
    Ok(summary)
}

impl Summary {
    fn new() -> Self {
        Self {
            // The max delay is 2000ms. At 3 significant figures.
            latencies: Histogram::<u64>::new_with_max(3_000, 3).unwrap(),
            start: Instant::now(),
            count_by_instance: [0; 10],
        }
    }

    fn count(&mut self, rsp: Rsp) {
        let ms = rsp.latency.as_secs() * 1_000;
        let ms = ms + u64::from(rsp.latency.subsec_nanos()) / 1_000 / 1_000;
        self.latencies += ms;
        self.count_by_instance[rsp.instance] += 1;
    }

    fn report(&self) {
        let mut total = 0;
        for c in &self.count_by_instance {
            total += c;
        }
        for (i, c) in self.count_by_instance.iter().enumerate() {
            let p = *c as f64 / total as f64 * 100.0;
            println!("  [{:02}] {:>5.01}%", i, p);
        }

        println!("  wall {:4}s", self.start.elapsed().as_secs());

        if self.latencies.len() < 2 {
            return;
        }
        println!("  p50  {:4}ms", self.latencies.value_at_quantile(0.5));

        if self.latencies.len() < 10 {
            return;
        }
        println!("  p90  {:4}ms", self.latencies.value_at_quantile(0.9));

        if self.latencies.len() < 50 {
            return;
        }
        println!("  p95  {:4}ms", self.latencies.value_at_quantile(0.95));

        if self.latencies.len() < 100 {
            return;
        }
        println!("  p99  {:4}ms", self.latencies.value_at_quantile(0.99));

        if self.latencies.len() < 1000 {
            return;
        }
        println!("  p999 {:4}ms", self.latencies.value_at_quantile(0.999));
    }
}

#[derive(Debug, Clone)]
struct Req;

#[derive(Debug)]
struct Rsp {
    latency: Duration,
    instance: usize,
}
