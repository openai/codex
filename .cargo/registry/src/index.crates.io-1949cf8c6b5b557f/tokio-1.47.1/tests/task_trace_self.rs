#![allow(unknown_lints, unexpected_cfgs)]
#![cfg(all(
    tokio_unstable,
    tokio_taskdump,
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86", target_arch = "x86_64")
))]

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use tokio::runtime::dump::{Root, Trace};

pin_project_lite::pin_project! {
    pub struct PrettyFuture<F: Future> {
        #[pin]
        f: Root<F>,
        t_last: State,
        logs: Arc<Mutex<Vec<Trace>>>,
    }
}

enum State {
    NotStarted,
    Running { since: Instant },
    Alerted,
}

impl<F: Future> PrettyFuture<F> {
    pub fn pretty(f: F, logs: Arc<Mutex<Vec<Trace>>>) -> Self {
        PrettyFuture {
            f: Trace::root(f),
            t_last: State::NotStarted,
            logs,
        }
    }
}

impl<F: Future> Future for PrettyFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
        let mut this = self.project();
        let now = Instant::now();
        let t_last = match this.t_last {
            State::Running { since } => Some(*since),
            State::NotStarted => {
                *this.t_last = State::Running { since: now };
                None
            }
            State::Alerted => {
                // don't double-alert for the same future
                None
            }
        };
        if t_last.is_some_and(|t_last| now.duration_since(t_last) > Duration::from_millis(500)) {
            let (res, trace) = tokio::runtime::dump::Trace::capture(|| this.f.as_mut().poll(cx));
            this.logs.lock().unwrap().push(trace);
            *this.t_last = State::Alerted;
            return res;
        }
        this.f.poll(cx)
    }
}

#[tokio::test]
async fn task_trace_self() {
    let log = Arc::new(Mutex::new(vec![]));
    let log2 = Arc::new(Mutex::new(vec![]));
    let mut good_line = vec![];
    let mut bad_line = vec![];
    PrettyFuture::pretty(
        PrettyFuture::pretty(
            async {
                bad_line.push(line!() + 1);
                tokio::task::yield_now().await;
                bad_line.push(line!() + 1);
                tokio::time::sleep(Duration::from_millis(1)).await;
                for _ in 0..100 {
                    good_line.push(line!() + 1);
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            },
            log.clone(),
        ),
        log2.clone(),
    )
    .await;
    for line in good_line {
        let s = format!("{}:{}:", file!(), line);
        assert!(log.lock().unwrap().iter().any(|x| {
            eprintln!("{}", x);
            format!("{}", x).contains(&s)
        }));
    }
    for line in bad_line {
        let s = format!("{}:{}:", file!(), line);
        assert!(!log
            .lock()
            .unwrap()
            .iter()
            .any(|x| format!("{}", x).contains(&s)));
    }
}
