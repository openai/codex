//! Tests for the after channel flavor.

#![cfg(not(miri))] // TODO: many assertions failed due to Miri is slow

use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{after, select, Select, TryRecvError};
use crossbeam_utils::thread::scope;

fn ms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

#[test]
fn fire() {
    let start = Instant::now();
    let r = after(ms(50));

    assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
    thread::sleep(ms(100));

    let fired = r.try_recv().unwrap();
    assert!(start < fired);
    assert!(fired - start >= ms(50));

    let now = Instant::now();
    assert!(fired < now);
    assert!(now - fired >= ms(50));

    assert_eq!(r.try_recv(), Err(TryRecvError::Empty));

    select! {
        recv(r) -> _ => panic!(),
        default => {}
    }

    select! {
        recv(r) -> _ => panic!(),
        recv(after(ms(200))) -> _ => {}
    }
}

#[test]
fn capacity() {
    const COUNT: usize = 10;

    for i in 0..COUNT {
        let r = after(ms(i as u64));
        assert_eq!(r.capacity(), Some(1));
    }
}

#[test]
fn len_empty_full() {
    let r = after(ms(50));

    assert_eq!(r.len(), 0);
    assert!(r.is_empty());
    assert!(!r.is_full());

    thread::sleep(ms(100));

    assert_eq!(r.len(), 1);
    assert!(!r.is_empty());
    assert!(r.is_full());

    r.try_recv().unwrap();

    assert_eq!(r.len(), 0);
    assert!(r.is_empty());
    assert!(!r.is_full());
}

#[test]
fn try_recv() {
    let r = after(ms(200));
    assert!(r.try_recv().is_err());

    thread::sleep(ms(100));
    assert!(r.try_recv().is_err());

    thread::sleep(ms(200));
    assert!(r.try_recv().is_ok());
    assert!(r.try_recv().is_err());

    thread::sleep(ms(200));
    assert!(r.try_recv().is_err());
}

#[test]
fn recv() {
    let start = Instant::now();
    let r = after(ms(50));

    let fired = r.recv().unwrap();
    assert!(start < fired);
    assert!(fired - start >= ms(50));

    let now = Instant::now();
    assert!(fired < now);
    assert!(now - fired < fired - start);

    assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
}

#[test]
fn recv_timeout() {
    let start = Instant::now();
    let r = after(ms(200));

    assert!(r.recv_timeout(ms(100)).is_err());
    let now = Instant::now();
    assert!(now - start >= ms(100));
    assert!(now - start <= ms(150));

    let fired = r.recv_timeout(ms(200)).unwrap();
    assert!(fired - start >= ms(200));
    assert!(fired - start <= ms(250));

    assert!(r.recv_timeout(ms(200)).is_err());
    let now = Instant::now();
    assert!(now - start >= ms(400));
    assert!(now - start <= ms(450));

    assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
}

#[test]
fn recv_two() {
    let r1 = after(ms(50));
    let r2 = after(ms(50));

    scope(|scope| {
        scope.spawn(|_| {
            select! {
                recv(r1) -> _ => {}
                recv(r2) -> _ => {}
            }
        });
        scope.spawn(|_| {
            select! {
                recv(r1) -> _ => {}
                recv(r2) -> _ => {}
            }
        });
    })
    .unwrap();
}

#[test]
fn recv_race() {
    select! {
        recv(after(ms(50))) -> _ => {}
        recv(after(ms(100))) -> _ => panic!(),
    }

    select! {
        recv(after(ms(100))) -> _ => panic!(),
        recv(after(ms(50))) -> _ => {}
    }
}

#[test]
fn stress_default() {
    const COUNT: usize = 10;

    for _ in 0..COUNT {
        select! {
            recv(after(ms(0))) -> _ => {}
            default => panic!(),
        }
    }

    for _ in 0..COUNT {
        select! {
            recv(after(ms(100))) -> _ => panic!(),
            default => {}
        }
    }
}

#[test]
fn select() {
    const THREADS: usize = 4;
    const COUNT: usize = 1000;
    const TIMEOUT_MS: u64 = 100;

    let v = (0..COUNT)
        .map(|i| after(ms(i as u64 / TIMEOUT_MS / 2)))
        .collect::<Vec<_>>();
    let hits = AtomicUsize::new(0);

    scope(|scope| {
        for _ in 0..THREADS {
            scope.spawn(|_| {
                let v: Vec<&_> = v.iter().collect();

                loop {
                    let timeout = after(ms(TIMEOUT_MS));
                    let mut sel = Select::new();
                    for r in &v {
                        sel.recv(r);
                    }
                    let oper_timeout = sel.recv(&timeout);

                    let oper = sel.select();
                    match oper.index() {
                        i if i == oper_timeout => {
                            oper.recv(&timeout).unwrap();
                            break;
                        }
                        i => {
                            oper.recv(v[i]).unwrap();
                            hits.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                }
            });
        }
    })
    .unwrap();

    assert_eq!(hits.load(Ordering::SeqCst), COUNT);
}

#[test]
fn ready() {
    const THREADS: usize = 4;
    const COUNT: usize = 1000;
    const TIMEOUT_MS: u64 = 100;

    let v = (0..COUNT)
        .map(|i| after(ms(i as u64 / TIMEOUT_MS / 2)))
        .collect::<Vec<_>>();
    let hits = AtomicUsize::new(0);

    scope(|scope| {
        for _ in 0..THREADS {
            scope.spawn(|_| {
                let v: Vec<&_> = v.iter().collect();

                loop {
                    let timeout = after(ms(TIMEOUT_MS));
                    let mut sel = Select::new();
                    for r in &v {
                        sel.recv(r);
                    }
                    let oper_timeout = sel.recv(&timeout);

                    loop {
                        let i = sel.ready();
                        if i == oper_timeout {
                            timeout.try_recv().unwrap();
                            return;
                        } else if v[i].try_recv().is_ok() {
                            hits.fetch_add(1, Ordering::SeqCst);
                            break;
                        }
                    }
                }
            });
        }
    })
    .unwrap();

    assert_eq!(hits.load(Ordering::SeqCst), COUNT);
}

#[test]
fn stress_clone() {
    const RUNS: usize = 1000;
    const THREADS: usize = 10;
    const COUNT: usize = 50;

    for i in 0..RUNS {
        let r = after(ms(i as u64));

        scope(|scope| {
            for _ in 0..THREADS {
                scope.spawn(|_| {
                    let r = r.clone();
                    let _ = r.try_recv();

                    for _ in 0..COUNT {
                        drop(r.clone());
                        thread::yield_now();
                    }
                });
            }
        })
        .unwrap();
    }
}

#[test]
fn fairness() {
    const COUNT: usize = 1000;

    for &dur in &[0, 1] {
        let mut hits = [0usize; 2];

        for _ in 0..COUNT {
            select! {
                recv(after(ms(dur))) -> _ => hits[0] += 1,
                recv(after(ms(dur))) -> _ => hits[1] += 1,
            }
        }

        assert!(hits.iter().all(|x| *x >= COUNT / hits.len() / 2));
    }
}

#[test]
fn fairness_duplicates() {
    const COUNT: usize = 1000;

    for &dur in &[0, 1] {
        let mut hits = [0usize; 5];

        for _ in 0..COUNT {
            let r = after(ms(dur));
            select! {
                recv(r) -> _ => hits[0] += 1,
                recv(r) -> _ => hits[1] += 1,
                recv(r) -> _ => hits[2] += 1,
                recv(r) -> _ => hits[3] += 1,
                recv(r) -> _ => hits[4] += 1,
            }
        }

        assert!(hits.iter().all(|x| *x >= COUNT / hits.len() / 2));
    }
}
