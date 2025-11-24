#![allow(clippy::disallowed_names)]
#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]

use futures::StreamExt;
use tokio::time::{self, sleep, sleep_until, Duration, Instant};
use tokio_test::{assert_pending, assert_ready, task};
use tokio_util::time::DelayQueue;

macro_rules! poll {
    ($queue:ident) => {
        $queue.enter(|cx, mut queue| queue.poll_expired(cx))
    };
}

macro_rules! assert_ready_some {
    ($e:expr) => {{
        match assert_ready!($e) {
            Some(v) => v,
            None => panic!("None"),
        }
    }};
}

#[tokio::test]
async fn single_immediate_delay() {
    time::pause();

    let mut queue = task::spawn(DelayQueue::new());
    let _key = queue.insert_at("foo", Instant::now());

    // Advance time by 1ms to handle thee rounding
    sleep(ms(1)).await;

    assert_ready_some!(poll!(queue));

    let entry = assert_ready!(poll!(queue));
    assert!(entry.is_none())
}

#[tokio::test]
async fn multi_immediate_delays() {
    time::pause();

    let mut queue = task::spawn(DelayQueue::new());

    let _k = queue.insert_at("1", Instant::now());
    let _k = queue.insert_at("2", Instant::now());
    let _k = queue.insert_at("3", Instant::now());

    sleep(ms(1)).await;

    let mut res = vec![];

    while res.len() < 3 {
        let entry = assert_ready_some!(poll!(queue));
        res.push(entry.into_inner());
    }

    let entry = assert_ready!(poll!(queue));
    assert!(entry.is_none());

    res.sort_unstable();

    assert_eq!("1", res[0]);
    assert_eq!("2", res[1]);
    assert_eq!("3", res[2]);
}

#[tokio::test]
async fn single_short_delay() {
    time::pause();

    let mut queue = task::spawn(DelayQueue::new());
    let _key = queue.insert_at("foo", Instant::now() + ms(5));

    assert_pending!(poll!(queue));

    sleep(ms(1)).await;

    assert!(!queue.is_woken());

    sleep(ms(5)).await;

    assert!(queue.is_woken());

    let entry = assert_ready_some!(poll!(queue));
    assert_eq!(*entry.get_ref(), "foo");

    let entry = assert_ready!(poll!(queue));
    assert!(entry.is_none());
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // Too slow on miri.
async fn multi_delay_at_start() {
    time::pause();

    let long = 262_144 + 9 * 4096;
    let delays = &[1000, 2, 234, long, 60, 10];

    let mut queue = task::spawn(DelayQueue::new());

    // Setup the delays
    for &i in delays {
        let _key = queue.insert_at(i, Instant::now() + ms(i));
    }

    assert_pending!(poll!(queue));
    assert!(!queue.is_woken());

    let start = Instant::now();
    for elapsed in 0..1200 {
        println!("elapsed: {elapsed:?}");
        let elapsed = elapsed + 1;
        tokio::time::sleep_until(start + ms(elapsed)).await;

        if delays.contains(&elapsed) {
            assert!(queue.is_woken());
            assert_ready!(poll!(queue));
            assert_pending!(poll!(queue));
        } else if queue.is_woken() {
            let cascade = &[192, 960];
            assert!(
                cascade.contains(&elapsed),
                "elapsed={} dt={:?}",
                elapsed,
                Instant::now() - start
            );

            assert_pending!(poll!(queue));
        }
    }
    println!("finished multi_delay_start");
}

#[tokio::test]
async fn insert_in_past_fires_immediately() {
    println!("running insert_in_past_fires_immediately");
    time::pause();

    let mut queue = task::spawn(DelayQueue::new());
    let now = Instant::now();

    sleep(ms(10)).await;

    queue.insert_at("foo", now);

    assert_ready!(poll!(queue));
    println!("finished insert_in_past_fires_immediately");
}

#[tokio::test]
async fn remove_entry() {
    time::pause();

    let mut queue = task::spawn(DelayQueue::new());

    let key = queue.insert_at("foo", Instant::now() + ms(5));

    assert_pending!(poll!(queue));

    let entry = queue.remove(&key);
    assert_eq!(entry.into_inner(), "foo");

    sleep(ms(10)).await;

    let entry = assert_ready!(poll!(queue));
    assert!(entry.is_none());
}

#[tokio::test]
async fn reset_entry() {
    time::pause();

    let mut queue = task::spawn(DelayQueue::new());

    let now = Instant::now();
    let key = queue.insert_at("foo", now + ms(5));

    assert_pending!(poll!(queue));
    sleep(ms(1)).await;

    queue.reset_at(&key, now + ms(10));

    assert_pending!(poll!(queue));

    sleep(ms(7)).await;

    assert!(!queue.is_woken());

    assert_pending!(poll!(queue));

    sleep(ms(3)).await;

    assert!(queue.is_woken());

    let entry = assert_ready_some!(poll!(queue));
    assert_eq!(*entry.get_ref(), "foo");

    let entry = assert_ready!(poll!(queue));
    assert!(entry.is_none())
}

// Reproduces tokio-rs/tokio#849.
#[tokio::test]
async fn reset_much_later() {
    time::pause();

    let mut queue = task::spawn(DelayQueue::new());

    let now = Instant::now();
    sleep(ms(1)).await;

    let key = queue.insert_at("foo", now + ms(200));
    assert_pending!(poll!(queue));

    sleep(ms(3)).await;

    queue.reset_at(&key, now + ms(10));

    sleep(ms(20)).await;

    assert!(queue.is_woken());
}

// Reproduces tokio-rs/tokio#849.
#[tokio::test]
async fn reset_twice() {
    time::pause();

    let mut queue = task::spawn(DelayQueue::new());
    let now = Instant::now();

    sleep(ms(1)).await;

    let key = queue.insert_at("foo", now + ms(200));

    assert_pending!(poll!(queue));

    sleep(ms(3)).await;

    queue.reset_at(&key, now + ms(50));

    sleep(ms(20)).await;

    queue.reset_at(&key, now + ms(40));

    sleep(ms(20)).await;

    assert!(queue.is_woken());
}

/// Regression test: Given an entry inserted with a deadline in the past, so
/// that it is placed directly on the expired queue, reset the entry to a
/// deadline in the future. Validate that this leaves the entry and queue in an
/// internally consistent state by running an additional reset on the entry
/// before polling it to completion.
#[tokio::test]
async fn repeatedly_reset_entry_inserted_as_expired() {
    time::pause();

    // Instants before the start of the test seem to break in wasm.
    time::sleep(ms(1000)).await;

    let mut queue = task::spawn(DelayQueue::new());
    let now = Instant::now();

    let key = queue.insert_at("foo", now - ms(100));

    queue.reset_at(&key, now + ms(100));
    queue.reset_at(&key, now + ms(50));

    assert_pending!(poll!(queue));

    time::sleep_until(now + ms(60)).await;

    assert!(queue.is_woken());

    let entry = assert_ready_some!(poll!(queue)).into_inner();
    assert_eq!(entry, "foo");

    let entry = assert_ready!(poll!(queue));
    assert!(entry.is_none());
}

#[tokio::test]
async fn remove_expired_item() {
    time::pause();

    let mut queue = DelayQueue::new();

    let now = Instant::now();

    sleep(ms(10)).await;

    let key = queue.insert_at("foo", now);

    let entry = queue.remove(&key);
    assert_eq!(entry.into_inner(), "foo");
}

/// Regression test: it should be possible to remove entries which fall in the
/// 0th slot of the internal timer wheel — that is, entries whose expiration
/// (a) falls at the beginning of one of the wheel's hierarchical levels and (b)
/// is equal to the wheel's current elapsed time.
#[tokio::test]
async fn remove_at_timer_wheel_threshold() {
    time::pause();

    let mut queue = task::spawn(DelayQueue::new());

    let now = Instant::now();

    let key1 = queue.insert_at("foo", now + ms(64));
    let key2 = queue.insert_at("bar", now + ms(64));

    sleep(ms(80)).await;

    let entry = assert_ready_some!(poll!(queue)).into_inner();

    match entry {
        "foo" => {
            let entry = queue.remove(&key2).into_inner();
            assert_eq!(entry, "bar");
        }
        "bar" => {
            let entry = queue.remove(&key1).into_inner();
            assert_eq!(entry, "foo");
        }
        other => panic!("other: {other:?}"),
    }
}

#[tokio::test]
async fn expires_before_last_insert() {
    time::pause();

    let mut queue = task::spawn(DelayQueue::new());

    let now = Instant::now();

    queue.insert_at("foo", now + ms(10_000));

    // Delay should be set to 8.192s here.
    assert_pending!(poll!(queue));

    // Delay should be set to the delay of the new item here
    queue.insert_at("bar", now + ms(600));

    assert_pending!(poll!(queue));

    sleep(ms(600)).await;

    assert!(queue.is_woken());

    let entry = assert_ready_some!(poll!(queue)).into_inner();
    assert_eq!(entry, "bar");
}

#[tokio::test]
async fn multi_reset() {
    time::pause();

    let mut queue = task::spawn(DelayQueue::new());

    let now = Instant::now();

    let one = queue.insert_at("one", now + ms(200));
    let two = queue.insert_at("two", now + ms(250));

    assert_pending!(poll!(queue));

    queue.reset_at(&one, now + ms(300));
    queue.reset_at(&two, now + ms(350));
    queue.reset_at(&one, now + ms(400));

    sleep(ms(310)).await;

    assert_pending!(poll!(queue));

    sleep(ms(50)).await;

    let entry = assert_ready_some!(poll!(queue));
    assert_eq!(*entry.get_ref(), "two");

    assert_pending!(poll!(queue));

    sleep(ms(50)).await;

    let entry = assert_ready_some!(poll!(queue));
    assert_eq!(*entry.get_ref(), "one");

    let entry = assert_ready!(poll!(queue));
    assert!(entry.is_none())
}

#[tokio::test]
async fn expire_first_key_when_reset_to_expire_earlier() {
    time::pause();

    let mut queue = task::spawn(DelayQueue::new());

    let now = Instant::now();

    let one = queue.insert_at("one", now + ms(200));
    queue.insert_at("two", now + ms(250));

    assert_pending!(poll!(queue));

    queue.reset_at(&one, now + ms(100));

    sleep(ms(100)).await;

    assert!(queue.is_woken());

    let entry = assert_ready_some!(poll!(queue)).into_inner();
    assert_eq!(entry, "one");
}

#[tokio::test]
async fn expire_second_key_when_reset_to_expire_earlier() {
    time::pause();

    let mut queue = task::spawn(DelayQueue::new());

    let now = Instant::now();

    queue.insert_at("one", now + ms(200));
    let two = queue.insert_at("two", now + ms(250));

    assert_pending!(poll!(queue));

    queue.reset_at(&two, now + ms(100));

    sleep(ms(100)).await;

    assert!(queue.is_woken());

    let entry = assert_ready_some!(poll!(queue)).into_inner();
    assert_eq!(entry, "two");
}

#[tokio::test]
async fn reset_first_expiring_item_to_expire_later() {
    time::pause();

    let mut queue = task::spawn(DelayQueue::new());

    let now = Instant::now();

    let one = queue.insert_at("one", now + ms(200));
    let _two = queue.insert_at("two", now + ms(250));

    assert_pending!(poll!(queue));

    queue.reset_at(&one, now + ms(300));
    sleep(ms(250)).await;

    assert!(queue.is_woken());

    let entry = assert_ready_some!(poll!(queue)).into_inner();
    assert_eq!(entry, "two");
}

#[tokio::test]
async fn insert_before_first_after_poll() {
    time::pause();

    let mut queue = task::spawn(DelayQueue::new());

    let now = Instant::now();

    let _one = queue.insert_at("one", now + ms(200));

    assert_pending!(poll!(queue));

    let _two = queue.insert_at("two", now + ms(100));

    sleep(ms(99)).await;

    assert_pending!(poll!(queue));

    sleep(ms(1)).await;

    assert!(queue.is_woken());

    let entry = assert_ready_some!(poll!(queue)).into_inner();
    assert_eq!(entry, "two");
}

#[tokio::test]
async fn insert_after_ready_poll() {
    time::pause();

    let mut queue = task::spawn(DelayQueue::new());

    let now = Instant::now();

    queue.insert_at("1", now + ms(100));
    queue.insert_at("2", now + ms(100));
    queue.insert_at("3", now + ms(100));

    assert_pending!(poll!(queue));

    sleep(ms(100)).await;

    assert!(queue.is_woken());

    let mut res = vec![];

    while res.len() < 3 {
        let entry = assert_ready_some!(poll!(queue));
        res.push(entry.into_inner());
        queue.insert_at("foo", now + ms(500));
    }

    res.sort_unstable();

    assert_eq!("1", res[0]);
    assert_eq!("2", res[1]);
    assert_eq!("3", res[2]);
}

#[tokio::test]
async fn reset_later_after_slot_starts() {
    time::pause();

    let mut queue = task::spawn(DelayQueue::new());

    let now = Instant::now();

    let foo = queue.insert_at("foo", now + ms(100));

    assert_pending!(poll!(queue));

    sleep_until(now + Duration::from_millis(80)).await;

    assert!(!queue.is_woken());

    // At this point the queue hasn't been polled, so `elapsed` on the wheel
    // for the queue is still at 0 and hence the 1ms resolution slots cover
    // [0-64).  Resetting the time on the entry to 120 causes it to get put in
    // the [64-128) slot.  As the queue knows that the first entry is within
    // that slot, but doesn't know when, it must wake immediately to advance
    // the wheel.
    queue.reset_at(&foo, now + ms(120));
    assert!(queue.is_woken());

    assert_pending!(poll!(queue));

    sleep_until(now + Duration::from_millis(119)).await;
    assert!(!queue.is_woken());

    sleep(ms(1)).await;
    assert!(queue.is_woken());

    let entry = assert_ready_some!(poll!(queue)).into_inner();
    assert_eq!(entry, "foo");
}

#[tokio::test]
async fn reset_inserted_expired() {
    time::pause();

    // Instants before the start of the test seem to break in wasm.
    time::sleep(ms(1000)).await;

    let mut queue = task::spawn(DelayQueue::new());
    let now = Instant::now();

    let key = queue.insert_at("foo", now - ms(100));

    // this causes the panic described in #2473
    queue.reset_at(&key, now + ms(100));

    assert_eq!(1, queue.len());

    sleep(ms(200)).await;

    let entry = assert_ready_some!(poll!(queue)).into_inner();
    assert_eq!(entry, "foo");

    assert_eq!(queue.len(), 0);
}

#[tokio::test]
async fn reset_earlier_after_slot_starts() {
    time::pause();

    let mut queue = task::spawn(DelayQueue::new());

    let now = Instant::now();

    let foo = queue.insert_at("foo", now + ms(200));

    assert_pending!(poll!(queue));

    sleep_until(now + Duration::from_millis(80)).await;

    assert!(!queue.is_woken());

    // At this point the queue hasn't been polled, so `elapsed` on the wheel
    // for the queue is still at 0 and hence the 1ms resolution slots cover
    // [0-64).  Resetting the time on the entry to 120 causes it to get put in
    // the [64-128) slot.  As the queue knows that the first entry is within
    // that slot, but doesn't know when, it must wake immediately to advance
    // the wheel.
    queue.reset_at(&foo, now + ms(120));
    assert!(queue.is_woken());

    assert_pending!(poll!(queue));

    sleep_until(now + Duration::from_millis(119)).await;
    assert!(!queue.is_woken());

    sleep(ms(1)).await;
    assert!(queue.is_woken());

    let entry = assert_ready_some!(poll!(queue)).into_inner();
    assert_eq!(entry, "foo");
}

#[tokio::test]
async fn insert_in_past_after_poll_fires_immediately() {
    time::pause();

    let mut queue = task::spawn(DelayQueue::new());

    let now = Instant::now();

    queue.insert_at("foo", now + ms(200));

    assert_pending!(poll!(queue));

    sleep(ms(80)).await;

    assert!(!queue.is_woken());
    queue.insert_at("bar", now + ms(40));

    assert!(queue.is_woken());

    let entry = assert_ready_some!(poll!(queue)).into_inner();
    assert_eq!(entry, "bar");
}

#[tokio::test]
async fn delay_queue_poll_expired_when_empty() {
    let mut delay_queue = task::spawn(DelayQueue::new());
    let key = delay_queue.insert(0, std::time::Duration::from_secs(10));
    assert_pending!(poll!(delay_queue));

    delay_queue.remove(&key);
    assert!(assert_ready!(poll!(delay_queue)).is_none());
}

#[tokio::test(start_paused = true)]
async fn compact_expire_empty() {
    let mut queue = task::spawn(DelayQueue::new());

    let now = Instant::now();

    queue.insert_at("foo1", now + ms(10));
    queue.insert_at("foo2", now + ms(10));

    sleep(ms(10)).await;

    let mut res = vec![];
    while res.len() < 2 {
        let entry = assert_ready_some!(poll!(queue));
        res.push(entry.into_inner());
    }

    queue.compact();

    assert_eq!(queue.len(), 0);
    assert_eq!(queue.capacity(), 0);
}

#[tokio::test(start_paused = true)]
async fn compact_remove_empty() {
    let mut queue = task::spawn(DelayQueue::new());

    let now = Instant::now();

    let key1 = queue.insert_at("foo1", now + ms(10));
    let key2 = queue.insert_at("foo2", now + ms(10));

    queue.remove(&key1);
    queue.remove(&key2);

    queue.compact();

    assert_eq!(queue.len(), 0);
    assert_eq!(queue.capacity(), 0);
}

#[tokio::test(start_paused = true)]
// Trigger a re-mapping of keys in the slab due to a `compact` call and
// test removal of re-mapped keys
async fn compact_remove_remapped_keys() {
    let mut queue = task::spawn(DelayQueue::new());

    let now = Instant::now();

    queue.insert_at("foo1", now + ms(10));
    queue.insert_at("foo2", now + ms(10));

    // should be assigned indices 3 and 4
    let key3 = queue.insert_at("foo3", now + ms(20));
    let key4 = queue.insert_at("foo4", now + ms(20));

    sleep(ms(10)).await;

    let mut res = vec![];
    while res.len() < 2 {
        let entry = assert_ready_some!(poll!(queue));
        res.push(entry.into_inner());
    }

    // items corresponding to `foo3` and `foo4` will be assigned
    // new indices here
    queue.compact();

    queue.insert_at("foo5", now + ms(10));

    // test removal of re-mapped keys
    let expired3 = queue.remove(&key3);
    let expired4 = queue.remove(&key4);

    assert_eq!(expired3.into_inner(), "foo3");
    assert_eq!(expired4.into_inner(), "foo4");

    queue.compact();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue.capacity(), 1);
}

#[tokio::test(start_paused = true)]
async fn compact_change_deadline() {
    let mut queue = task::spawn(DelayQueue::new());

    let mut now = Instant::now();

    queue.insert_at("foo1", now + ms(10));
    queue.insert_at("foo2", now + ms(10));

    // should be assigned indices 3 and 4
    queue.insert_at("foo3", now + ms(20));
    let key4 = queue.insert_at("foo4", now + ms(20));

    sleep(ms(10)).await;

    let mut res = vec![];
    while res.len() < 2 {
        let entry = assert_ready_some!(poll!(queue));
        res.push(entry.into_inner());
    }

    // items corresponding to `foo3` and `foo4` should be assigned
    // new indices
    queue.compact();

    now = Instant::now();

    queue.insert_at("foo5", now + ms(10));
    let key6 = queue.insert_at("foo6", now + ms(10));

    queue.reset_at(&key4, now + ms(20));
    queue.reset_at(&key6, now + ms(20));

    // foo3 and foo5 will expire
    sleep(ms(10)).await;

    while res.len() < 4 {
        let entry = assert_ready_some!(poll!(queue));
        res.push(entry.into_inner());
    }

    sleep(ms(10)).await;

    while res.len() < 6 {
        let entry = assert_ready_some!(poll!(queue));
        res.push(entry.into_inner());
    }

    let entry = assert_ready!(poll!(queue));
    assert!(entry.is_none());
}

#[tokio::test(start_paused = true)]
async fn item_expiry_greater_than_wheel() {
    // This function tests that a delay queue that has existed for at least 2^36 milliseconds won't panic when a new item is inserted.
    let mut queue = DelayQueue::new();
    for _ in 0..2 {
        tokio::time::advance(Duration::from_millis(1 << 35)).await;
        queue.insert(0, Duration::from_millis(0));
        queue.next().await;
    }
    // This should not panic
    let no_panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        queue.insert(1, Duration::from_millis(1));
    }));
    assert!(no_panic.is_ok());
}

#[cfg_attr(target_os = "wasi", ignore = "FIXME: Does not seem to work with WASI")]
#[tokio::test(start_paused = true)]
#[cfg(panic = "unwind")]
async fn remove_after_compact() {
    let now = Instant::now();
    let mut queue = DelayQueue::new();

    let foo_key = queue.insert_at("foo", now + ms(10));
    queue.insert_at("bar", now + ms(20));
    queue.remove(&foo_key);
    queue.compact();

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        queue.remove(&foo_key);
    }));
    assert!(panic.is_err());
}

#[cfg_attr(target_os = "wasi", ignore = "FIXME: Does not seem to work with WASI")]
#[tokio::test(start_paused = true)]
#[cfg(panic = "unwind")]
async fn remove_after_compact_poll() {
    let now = Instant::now();
    let mut queue = task::spawn(DelayQueue::new());

    let foo_key = queue.insert_at("foo", now + ms(10));
    queue.insert_at("bar", now + ms(20));

    sleep(ms(10)).await;
    assert_eq!(assert_ready_some!(poll!(queue)).key(), foo_key);

    queue.compact();

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        queue.remove(&foo_key);
    }));
    assert!(panic.is_err());
}

#[tokio::test(start_paused = true)]
async fn peek() {
    let mut queue = task::spawn(DelayQueue::new());

    let now = Instant::now();

    let key = queue.insert_at("foo", now + ms(5));
    let key2 = queue.insert_at("bar", now);
    let key3 = queue.insert_at("baz", now + ms(10));

    assert_eq!(queue.peek(), Some(key2));

    sleep(ms(6)).await;

    assert_eq!(queue.peek(), Some(key2));

    let entry = assert_ready_some!(poll!(queue));
    assert_eq!(entry.get_ref(), &"bar");

    assert_eq!(queue.peek(), Some(key));

    let entry = assert_ready_some!(poll!(queue));
    assert_eq!(entry.get_ref(), &"foo");

    assert_eq!(queue.peek(), Some(key3));

    assert_pending!(poll!(queue));

    sleep(ms(5)).await;

    assert_eq!(queue.peek(), Some(key3));

    let entry = assert_ready_some!(poll!(queue));
    assert_eq!(entry.get_ref(), &"baz");

    assert!(queue.peek().is_none());
}

#[tokio::test(start_paused = true)]
async fn wake_after_remove_last() {
    let mut queue = task::spawn(DelayQueue::new());
    let key = queue.insert("foo", ms(1000));

    assert_pending!(poll!(queue));

    queue.remove(&key);

    assert!(queue.is_woken());
    assert!(assert_ready!(poll!(queue)).is_none());
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
