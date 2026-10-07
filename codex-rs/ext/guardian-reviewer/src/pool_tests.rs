use super::*;
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

struct Session(usize);

impl ReviewerSession for Session {
    type Setup = ();
    type Context = ();
    type Snapshot = usize;

    fn context(&self) -> &() {
        &()
    }
    async fn snapshot(&self) -> Option<usize> {
        Some(self.0)
    }
}

struct Request {
    fresh: bool,
    selected: tokio::sync::mpsc::UnboundedSender<usize>,
    release: Option<Arc<Semaphore>>,
}

impl ReviewerRequest for Request {
    type Session = Session;

    fn setup(&self) -> Arc<()> {
        Arc::new(())
    }
    fn context(&self, _previous: Option<&Session>) {}
    fn requires_fresh_session(&self) -> bool {
        self.fresh
    }
    fn deadline(&self) -> Instant {
        Instant::now() + std::time::Duration::from_secs(5)
    }
    fn cancellation(&self) -> Option<&CancellationToken> {
        None
    }
    async fn run(
        &self,
        session: &Session,
        _kind: GuardianReviewSessionKind,
    ) -> ReviewSessionResult {
        self.selected
            .send(session.0)
            .expect("observe selected session");
        if let Some(release) = &self.release {
            let _permit = release.acquire().await.expect("release busy review");
        }
        ReviewSessionResult {
            outcome: GuardianReviewSessionOutcome::Completed(Err(anyhow::anyhow!("test verdict"))),
            disposition: SessionDisposition::Reusable,
            analytics: GuardianReviewAnalyticsResult::without_session(),
        }
    }
}

#[test_case::test_case(false; "idle trunk is replaced")]
#[test_case::test_case(true; "busy trunk is preserved")]
#[tokio::test]
async fn fresh_attempt_bypasses_cached_session_and_snapshot(busy: bool) {
    let spawns = Arc::new(AtomicUsize::default());
    let pool = Arc::new(ReviewerPool::new(
        Arc::new(ReviewerTasks::default()),
        move |_, _, _, snapshot, _| {
            assert_eq!(
                snapshot, None,
                "fresh recovery must not inherit a cached snapshot"
            );
            let id = spawns.fetch_add(/*val*/ 1, Ordering::SeqCst);
            Box::pin(async move { Ok(Session(id)) })
        },
    ));
    pool.prewarm(Arc::new(()), ()).await.expect("prewarm trunk");
    let (selected, mut selections) = tokio::sync::mpsc::unbounded_channel();
    let release = Arc::new(Semaphore::new(/*permits*/ 0));
    let active = if busy {
        let pool = Arc::clone(&pool);
        let request = Request {
            fresh: false,
            selected: selected.clone(),
            release: Some(Arc::clone(&release)),
        };
        let task = tokio::spawn(async move { pool.review(request).await });
        assert_eq!(selections.recv().await, Some(0));
        Some(task)
    } else {
        None
    };

    pool.review(Request {
        fresh: true,
        selected: selected.clone(),
        release: None,
    })
    .await;
    assert_eq!(selections.recv().await, Some(1));
    if let Some(active) = active {
        release.add_permits(/*n*/ 1);
        active.await.expect("finish concurrent review");
    }
    // Idle recovery becomes the reusable trunk; busy recovery leaves the active trunk intact.
    pool.review(Request {
        fresh: false,
        selected,
        release: None,
    })
    .await;
    assert_eq!(selections.recv().await, Some(if busy { 0 } else { 1 }));
}
