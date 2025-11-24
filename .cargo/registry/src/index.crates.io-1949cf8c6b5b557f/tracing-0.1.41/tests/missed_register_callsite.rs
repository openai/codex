use std::{
    ptr,
    sync::atomic::{AtomicPtr, Ordering},
    thread::{self, JoinHandle},
    time::Duration,
};

use tracing::Subscriber;
use tracing_core::{span, Metadata};

struct TestSubscriber {
    creator_thread: String,
    sleep: Duration,
    callsite: AtomicPtr<Metadata<'static>>,
}

impl TestSubscriber {
    fn new(sleep_micros: u64) -> Self {
        let creator_thread = thread::current()
            .name()
            .unwrap_or("<unknown thread>")
            .to_owned();
        Self {
            creator_thread,
            sleep: Duration::from_micros(sleep_micros),
            callsite: AtomicPtr::new(ptr::null_mut()),
        }
    }
}

impl Subscriber for TestSubscriber {
    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> tracing_core::Interest {
        if !self.sleep.is_zero() {
            thread::sleep(self.sleep);
        }

        self.callsite
            .store(metadata as *const _ as *mut _, Ordering::SeqCst);
        println!(
            "{creator} from {thread:?}: register_callsite: {callsite:#?}",
            creator = self.creator_thread,
            callsite = metadata as *const _,
            thread = thread::current().name(),
        );
        tracing_core::Interest::always()
    }

    fn event(&self, event: &tracing_core::Event<'_>) {
        let stored_callsite = self.callsite.load(Ordering::SeqCst);
        let event_callsite: *mut Metadata<'static> = event.metadata() as *const _ as *mut _;

        println!(
            "{creator} from {thread:?}: event (with callsite): {event_callsite:#?} (stored callsite: {stored_callsite:#?})",
            creator = self.creator_thread,
            thread = thread::current().name(),
        );

        // This assert is the actual test.
        assert_eq!(
            stored_callsite, event_callsite,
            "stored callsite: {stored_callsite:#?} does not match event \
            callsite: {event_callsite:#?}. Was `event` called before \
            `register_callsite`?"
        );
    }

    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _span: &span::Attributes<'_>) -> span::Id {
        span::Id::from_u64(0)
    }
    fn record(&self, _span: &span::Id, _values: &span::Record<'_>) {}
    fn record_follows_from(&self, _span: &span::Id, _follows: &span::Id) {}
    fn enter(&self, _span: &tracing_core::span::Id) {}
    fn exit(&self, _span: &tracing_core::span::Id) {}
}

fn subscriber_thread(idx: usize, register_sleep_micros: u64) -> JoinHandle<()> {
    thread::Builder::new()
        .name(format!("subscriber-{idx}"))
        .spawn(move || {
            // We use a sleep to ensure the starting order of the 2 threads.
            let subscriber = TestSubscriber::new(register_sleep_micros);
            let _subscriber_guard = tracing::subscriber::set_default(subscriber);

            tracing::info!("event-from-{idx}", idx = idx);

            // Wait a bit for everything to end (we don't want to remove the subscriber
            // immediately because that will mix up the test).
            thread::sleep(Duration::from_millis(100));
        })
        .expect("failed to spawn thread")
}

#[test]
fn event_before_register() {
    let subscriber_1_register_sleep_micros = 100;
    let subscriber_2_register_sleep_micros = 0;

    let jh1 = subscriber_thread(1, subscriber_1_register_sleep_micros);

    // This delay ensures that the event!() in the first thread is executed first.
    thread::sleep(Duration::from_micros(50));
    let jh2 = subscriber_thread(2, subscriber_2_register_sleep_micros);

    jh1.join().expect("failed to join thread");
    jh2.join().expect("failed to join thread");
}
