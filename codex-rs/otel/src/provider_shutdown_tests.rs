use super::Context;
use super::OTelSdkResult;
use super::OtelProvider;
use super::SdkTracerProvider;
use super::Span;
use super::SpanData;
use super::SpanProcessor;
use pretty_assertions::assert_eq;
use std::io::ErrorKind;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShutdownBehavior {
    Complete,
    WaitForRelease,
}

#[derive(Debug, Default)]
struct ShutdownState {
    force_flushes: AtomicUsize,
    shutdowns: AtomicUsize,
    released: Mutex<bool>,
    release_notification: Condvar,
    started: Mutex<Option<mpsc::Sender<()>>>,
    completed: Mutex<Option<mpsc::Sender<()>>>,
}

#[derive(Debug)]
struct ControlledSpanProcessor {
    behavior: ShutdownBehavior,
    state: Arc<ShutdownState>,
}

impl SpanProcessor for ControlledSpanProcessor {
    fn on_start(&self, _span: &mut Span, _context: &Context) {}

    fn on_end(&self, _span: SpanData) {}

    fn force_flush(&self) -> OTelSdkResult {
        self.state
            .force_flushes
            .fetch_add(/*val*/ 1, Ordering::Relaxed);
        Ok(())
    }

    fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
        self.state.shutdowns.fetch_add(/*val*/ 1, Ordering::Relaxed);
        if let Some(started) = self.state.started.lock().expect("started lock").take() {
            let _ = started.send(());
        }

        if self.behavior == ShutdownBehavior::WaitForRelease {
            drop(
                self.state
                    .release_notification
                    .wait_while(
                        self.state.released.lock().expect("release lock"),
                        |released| !*released,
                    )
                    .expect("release notification"),
            );
        }

        if let Some(completed) = self.state.completed.lock().expect("completed lock").take() {
            let _ = completed.send(());
        }
        Ok(())
    }
}

struct TestProvider {
    provider: OtelProvider,
    state: Arc<ShutdownState>,
    started: mpsc::Receiver<()>,
    completed: mpsc::Receiver<()>,
}

fn test_provider(behavior: ShutdownBehavior) -> TestProvider {
    let (started_tx, started) = mpsc::channel();
    let (completed_tx, completed) = mpsc::channel();
    let state = Arc::new(ShutdownState {
        started: Mutex::new(Some(started_tx)),
        completed: Mutex::new(Some(completed_tx)),
        ..ShutdownState::default()
    });
    let processor = ControlledSpanProcessor {
        behavior,
        state: Arc::clone(&state),
    };
    let tracer_provider = SdkTracerProvider::builder()
        .with_span_processor(processor)
        .build();

    TestProvider {
        provider: OtelProvider {
            logger: None,
            tracer_provider: Some(tracer_provider),
            tracer: None,
            metrics: None,
            shutdown_started: AtomicBool::default(),
        },
        state,
        started,
        completed,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn bounded_shutdown_does_not_flush_when_worker_creation_fails() {
    let TestProvider {
        provider,
        state,
        started,
        completed,
    } = test_provider(ShutdownBehavior::Complete);

    let result = provider
        .shutdown_with_timeout_and_spawner(Duration::from_secs(/*secs*/ 1), |_worker| {
            Err(std::io::Error::new(
                ErrorKind::WouldBlock,
                "shutdown worker could not be created",
            ))
        })
        .await;

    assert_eq!(
        result.as_ref().map_err(std::io::Error::kind),
        Err(ErrorKind::WouldBlock)
    );
    assert_eq!(state.shutdowns.load(Ordering::Relaxed), 0);
    assert_eq!(state.force_flushes.load(Ordering::Relaxed), 0);
    assert_eq!(started.try_recv(), Err(mpsc::TryRecvError::Empty));
    assert_eq!(completed.try_recv(), Err(mpsc::TryRecvError::Empty));
}

#[tokio::test(flavor = "current_thread")]
async fn bounded_shutdown_times_out_without_blocking_the_runtime() {
    let TestProvider {
        provider,
        state,
        started,
        completed,
    } = test_provider(ShutdownBehavior::WaitForRelease);

    let result = provider
        .shutdown_with_timeout(Duration::from_millis(/*millis*/ 50))
        .await;

    assert_eq!(
        result.as_ref().map_err(std::io::Error::kind),
        Err(ErrorKind::TimedOut)
    );
    started
        .recv_timeout(Duration::from_secs(/*secs*/ 1))
        .expect("shutdown worker started");
    assert_eq!(state.shutdowns.load(Ordering::Relaxed), 1);
    assert_eq!(state.force_flushes.load(Ordering::Relaxed), 0);

    *state.released.lock().expect("release lock") = true;
    state.release_notification.notify_one();
    completed
        .recv_timeout(Duration::from_secs(/*secs*/ 1))
        .expect("shutdown worker completed after release");
}

async fn assert_bounded_shutdown_completes() {
    let TestProvider {
        provider,
        state,
        started,
        completed,
    } = test_provider(ShutdownBehavior::Complete);

    provider
        .shutdown_with_timeout(Duration::from_secs(/*secs*/ 1))
        .await
        .expect("healthy processor shuts down");

    started
        .recv_timeout(Duration::from_secs(/*secs*/ 1))
        .expect("shutdown worker started");
    completed
        .recv_timeout(Duration::from_secs(/*secs*/ 1))
        .expect("shutdown worker completed");
    assert_eq!(state.shutdowns.load(Ordering::Relaxed), 1);
    assert_eq!(state.force_flushes.load(Ordering::Relaxed), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn bounded_shutdown_completes_on_current_thread_runtime() {
    assert_bounded_shutdown_completes().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bounded_shutdown_completes_on_multi_thread_runtime() {
    assert_bounded_shutdown_completes().await;
}

#[test]
fn explicit_shutdown_and_drop_shut_down_exporters_once_without_force_flush() {
    let TestProvider {
        provider,
        state,
        started,
        completed,
    } = test_provider(ShutdownBehavior::Complete);

    provider.shutdown();
    provider.shutdown();
    drop(provider);

    started
        .recv_timeout(Duration::from_secs(/*secs*/ 1))
        .expect("shutdown started");
    completed
        .recv_timeout(Duration::from_secs(/*secs*/ 1))
        .expect("shutdown completed");
    assert_eq!(state.shutdowns.load(Ordering::Relaxed), 1);
    assert_eq!(state.force_flushes.load(Ordering::Relaxed), 0);
}

#[test]
fn drop_shuts_down_exporters_without_force_flush() {
    let TestProvider {
        provider,
        state,
        started,
        completed,
    } = test_provider(ShutdownBehavior::Complete);

    drop(provider);

    started
        .recv_timeout(Duration::from_secs(/*secs*/ 1))
        .expect("shutdown started");
    completed
        .recv_timeout(Duration::from_secs(/*secs*/ 1))
        .expect("shutdown completed");
    assert_eq!(state.shutdowns.load(Ordering::Relaxed), 1);
    assert_eq!(state.force_flushes.load(Ordering::Relaxed), 0);
}
