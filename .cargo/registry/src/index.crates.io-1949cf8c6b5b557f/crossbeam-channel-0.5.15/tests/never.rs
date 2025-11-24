//! Tests for the never channel flavor.

use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{never, select, tick, unbounded};

fn ms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

#[test]
fn smoke() {
    select! {
        recv(never::<i32>()) -> _ => panic!(),
        default => {}
    }
}

#[test]
fn optional() {
    let (s, r) = unbounded::<i32>();
    s.send(1).unwrap();
    s.send(2).unwrap();

    let mut r = Some(&r);
    select! {
        recv(r.unwrap_or(&never())) -> _ => {}
        default => panic!(),
    }

    r = None;
    select! {
        recv(r.unwrap_or(&never())) -> _ => panic!(),
        default => {}
    }
}

#[test]
fn tick_n() {
    let mut r = tick(ms(100));
    let mut step = 0;

    loop {
        select! {
            recv(r) -> _ => step += 1,
            default(ms(500)) => break,
        }

        if step == 10 {
            r = never();
        }
    }

    assert_eq!(step, 10);
}

#[test]
fn capacity() {
    let r = never::<i32>();
    assert_eq!(r.capacity(), Some(0));
}

#[test]
fn len_empty_full() {
    let r = never::<i32>();
    assert_eq!(r.len(), 0);
    assert!(r.is_empty());
    assert!(r.is_full());
}

#[test]
fn try_recv() {
    let r = never::<i32>();
    assert!(r.try_recv().is_err());

    thread::sleep(ms(100));
    assert!(r.try_recv().is_err());
}

#[test]
fn recv_timeout() {
    let start = Instant::now();
    let r = never::<i32>();

    assert!(r.recv_timeout(ms(100)).is_err());
    let now = Instant::now();
    assert!(now - start >= ms(100));
    assert!(now - start <= ms(150));

    assert!(r.recv_timeout(ms(100)).is_err());
    let now = Instant::now();
    assert!(now - start >= ms(200));
    assert!(now - start <= ms(250));
}
