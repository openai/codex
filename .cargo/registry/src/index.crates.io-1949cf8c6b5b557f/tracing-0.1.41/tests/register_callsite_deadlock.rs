use std::{sync::mpsc, thread, time::Duration};
use tracing::{
    metadata::Metadata,
    span,
    subscriber::{self, Interest, Subscriber},
    Event,
};

#[test]
fn register_callsite_doesnt_deadlock() {
    pub struct EvilSubscriber;

    impl Subscriber for EvilSubscriber {
        fn register_callsite(&self, meta: &'static Metadata<'static>) -> Interest {
            tracing::info!(?meta, "registered a callsite");
            Interest::always()
        }

        fn enabled(&self, _: &Metadata<'_>) -> bool {
            true
        }
        fn new_span(&self, _: &span::Attributes<'_>) -> span::Id {
            span::Id::from_u64(1)
        }
        fn record(&self, _: &span::Id, _: &span::Record<'_>) {}
        fn record_follows_from(&self, _: &span::Id, _: &span::Id) {}
        fn event(&self, _: &Event<'_>) {}
        fn enter(&self, _: &span::Id) {}
        fn exit(&self, _: &span::Id) {}
    }

    subscriber::set_global_default(EvilSubscriber).unwrap();

    // spawn a thread, and assert it doesn't hang...
    let (tx, didnt_hang) = mpsc::channel();
    let th = thread::spawn(move || {
        tracing::info!("hello world!");
        tx.send(()).unwrap();
    });

    didnt_hang
        // Note: 60 seconds is *way* more than enough, but let's be generous in
        // case of e.g. slow CI machines.
        .recv_timeout(Duration::from_secs(60))
        .expect("the thread must not have hung!");
    th.join().expect("thread should join successfully");
}
