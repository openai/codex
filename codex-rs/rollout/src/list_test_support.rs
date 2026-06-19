//! Test-only operation counts for filesystem thread listing.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ThreadListWork {
    pub rollout_opens: usize,
    pub session_meta_records: usize,
    pub full_head_summaries: usize,
}

#[derive(Default)]
struct ThreadListWorkRecorder {
    rollout_opens: AtomicUsize,
    session_meta_records: AtomicUsize,
    full_head_summaries: AtomicUsize,
}

#[derive(Clone)]
pub(crate) struct ThreadListWorkScope(Option<Arc<ThreadListWorkRecorder>>);

impl ThreadListWorkScope {
    pub(crate) async fn scope<F>(self, future: F) -> F::Output
    where
        F: Future,
    {
        match self.0 {
            Some(recorder) => THREAD_LIST_WORK.scope(recorder, future).await,
            None => future.await,
        }
    }
}

impl ThreadListWorkRecorder {
    fn snapshot(&self) -> ThreadListWork {
        ThreadListWork {
            rollout_opens: self.rollout_opens.load(Ordering::Relaxed),
            session_meta_records: self.session_meta_records.load(Ordering::Relaxed),
            full_head_summaries: self.full_head_summaries.load(Ordering::Relaxed),
        }
    }
}

tokio::task_local! {
    static THREAD_LIST_WORK: Arc<ThreadListWorkRecorder>;
}

pub(crate) async fn record_thread_list_work<F>(future: F) -> (F::Output, ThreadListWork)
where
    F: Future,
{
    let recorder = Arc::new(ThreadListWorkRecorder::default());
    let output = THREAD_LIST_WORK.scope(Arc::clone(&recorder), future).await;
    (output, recorder.snapshot())
}

pub(crate) fn capture_thread_list_work() -> ThreadListWorkScope {
    ThreadListWorkScope(THREAD_LIST_WORK.try_with(Arc::clone).ok())
}

pub(crate) fn record_session_meta() {
    let _ = THREAD_LIST_WORK.try_with(|work| {
        work.session_meta_records.fetch_add(1, Ordering::Relaxed);
    });
}

pub(crate) fn record_rollout_open() {
    let _ = THREAD_LIST_WORK.try_with(|work| {
        work.rollout_opens.fetch_add(1, Ordering::Relaxed);
    });
}

pub(crate) fn record_full_head_summary() {
    let _ = THREAD_LIST_WORK.try_with(|work| {
        work.full_head_summaries.fetch_add(1, Ordering::Relaxed);
    });
}
