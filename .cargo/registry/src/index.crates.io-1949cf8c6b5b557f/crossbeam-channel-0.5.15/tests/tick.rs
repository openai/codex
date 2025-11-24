//! Tests for the tick channel flavor.

#![cfg(not(miri))] // TODO: many assertions failed due to Miri is slow

use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{after, select, tick, Select, TryRecvError};
use crossbeam_utils::thread::scope;

fn ms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

#[test]
fn fire() {
    let start = Instant::now();
    let r = tick(ms(50));

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
        recv(r) -> _ => {}
        recv(tick(ms(200))) -> _ => panic!(),
    }
}

#[test]
fn intervals() {
    let start = Instant::now();
    let r = tick(ms(50));

    let t1 = r.recv().unwrap();
    assert!(start + ms(50) <= t1);
    assert!(start + ms(100) > t1);

    thread::sleep(ms(300));
    let t2 = r.try_recv().unwrap();
    assert!(start + ms(100) <= t2);
    assert!(start + ms(150) > t2);

    assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
    let t3 = r.recv().unwrap();
    assert!(start + ms(400) <= t3);
    assert!(start + ms(450) > t3);

    assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
}

#[test]
fn capacity() {
    const COUNT: usize = 10;

    for i in 0..COUNT {
        let r = tick(ms(i as u64));
        assert_eq!(r.capacity(), Some(1));
    }
}

#[test]
fn len_empty_full() {
    let r = tick(ms(50));

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
    let r = tick(ms(200));
    assert!(r.try_recv().is_err());

    thread::sleep(ms(100));
    assert!(r.try_recv().is_err());

    thread::sleep(ms(200));
    assert!(r.try_recv().is_ok());
    assert!(r.try_recv().is_err());

    thread::sleep(ms(200));
    assert!(r.try_recv().is_ok());
    assert!(r.try_recv().is_err());
}

#[test]
fn recv() {
    let start = Instant::now();
    let r = tick(ms(50));

    let fired = r.recv().unwrap();
    assert!(start < fired);
    assert!(fired - start >= ms(50));

    let now = Instant::now();
    assert!(fired < now);
    assert!(now - fired < fired - start);

    assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
}

#[cfg(not(crossbeam_sanitize))] // TODO: assertions failed due to tsan is slow
#[test]
fn recv_timeout() {
    let start = Instant::now();
    let r = tick(ms(200));

    assert!(r.recv_timeout(ms(100)).is_err());
    let now = Instant::now();
    assert!(now - start >= ms(100));
    assert!(now - start <= ms(150));

    let fired = r.recv_timeout(ms(200)).unwrap();
    assert!(fired - start >= ms(200));
    assert!(fired - start <= ms(250));

    assert!(r.recv_timeout(ms(100)).is_err());
    let now = Instant::now();
    assert!(now - start >= ms(300));
    assert!(now - start <= ms(350));

    let fired = r.recv_timeout(ms(200)).unwrap();
    assert!(fired - start >= ms(400));
    assert!(fired - start <= ms(450));
}

#[test]
fn recv_two() {
    let r1 = tick(ms(50));
    let r2 = tick(ms(50));

    scope(|scope| {
        scope.spawn(|_| {
            for _ in 0..10 {
                select! {
                    recv(r1) -> _ => {}
                    recv(r2) -> _ => {}
                }
            }
        });
        scope.spawn(|_| {
            for _ in 0..10 {
                select! {
                    recv(r1) -> _ => {}
                    recv(r2) -> _ => {}
                }
            }
        });
    })
    .unwrap();
}

#[test]
fn recv_race() {
    select! {
        recv(tick(ms(50))) -> _ => {}
        recv(tick(ms(100))) -> _ => panic!(),
    }

    select! {
        recv(tick(ms(100))) -> _ => panic!(),
        recv(tick(ms(50))) -> _ => {}
    }
}

#[test]
fn stress_default() {
    const COUNT: usize = 10;

    for _ in 0..COUNT {
        select! {
            recv(tick(ms(0))) -> _ => {}
            default => panic!(),
        }
    }

    for _ in 0..COUNT {
        select! {
            recv(tick(ms(100))) -> _ => panic!(),
            default => {}
        }
    }
}

#[test]
fn select() {
    const THREADS: usize = 4;

    let hits = AtomicUsize::new(0);
    let r1 = tick(ms(200));
    let r2 = tick(ms(300));

    scope(|scope| {
        for _ in 0..THREADS {
            scope.spawn(|_| {
                let timeout = after(ms(1100));
                loop {
                    let mut sel = Select::new();
                    let oper1 = sel.recv(&r1);
                    let oper2 = sel.recv(&r2);
                    let oper3 = sel.recv(&timeout);
                    let oper = sel.select();
                    match oper.index() {
                        i if i == oper1 => {
                            oper.recv(&r1).unwrap();
                            hits.fetch_add(1, Ordering::SeqCst);
                        }
                        i if i == oper2 => {
                            oper.recv(&r2).unwrap();
                            hits.fetch_add(1, Ordering::SeqCst);
                        }
                        i if i == oper3 => {
                            oper.recv(&timeout).unwrap();
                            break;
                        }
                        _ => unreachable!(),
                    }
                }
            });
        }
    })
    .unwrap();

    assert_eq!(hits.load(Ordering::SeqCst), 8);
}

#[cfg(not(crossbeam_sanitize))] // TODO: assertions failed due to tsan is slow
#[test]
fn ready() {
    const THREADS: usize = 4;

    let hits = AtomicUsize::new(0);
    let r1 = tick(ms(200));
    let r2 = tick(ms(300));

    scope(|scope| {
        for _ in 0..THREADS {
            scope.spawn(|_| {
                let timeout = after(ms(1100));
                'outer: loop {
                    let mut sel = Select::new();
                    sel.recv(&r1);
                    sel.recv(&r2);
                    sel.recv(&timeout);
                    loop {
                        match sel.ready() {
                            0 => {
                                if r1.try_recv().is_ok() {
                                    hits.fetch_add(1, Ordering::SeqCst);
                                    break;
                                }
                            }
                            1 => {
                                if r2.try_recv().is_ok() {
                                    hits.fetch_add(1, Ordering::SeqCst);
                                    break;
                                }
                            }
                            2 => {
                                if timeout.try_recv().is_ok() {
                                    break 'outer;
                                }
                            }
                            _ => unreachable!(),
                        }
                    }
                }
            });
        }
    })
    .unwrap();

    assert_eq!(hits.load(Ordering::SeqCst), 8);
}

#[test]
fn fairness() {
    const COUNT: usize = 30;

    for &dur in &[0, 1] {
        let mut hits = [0usize; 2];

        for _ in 0..COUNT {
            let r1 = tick(ms(dur));
            let r2 = tick(ms(dur));

            for _ in 0..COUNT {
                select! {
                    recv(r1) -> _ => hits[0] += 1,
                    recv(r2) -> _ => hits[1] += 1,
                }
            }
        }

        assert!(hits.iter().all(|x| *x >= COUNT / hits.len() / 2));
    }
}

#[test]
fn fairness_duplicates() {
    const COUNT: usize = 30;

    for &dur in &[0, 1] {
        let mut hits = [0usize; 5];

        for _ in 0..COUNT {
            let r = tick(ms(dur));

            for _ in 0..COUNT {
                select! {
                    recv(r) -> _ => hits[0] += 1,
                    recv(r) -> _ => hits[1] += 1,
                    recv(r) -> _ => hits[2] += 1,
                    recv(r) -> _ => hits[3] += 1,
                    recv(r) -> _ => hits[4] += 1,
                }
            }
        }

        assert!(hits.iter().all(|x| *x >= COUNT / hits.len() / 2));
    }
}
