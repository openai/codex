//! Tests for the `select!` macro.

#![forbid(unsafe_code)] // select! is safe.
#![allow(clippy::match_single_binding)]

use std::any::Any;
use std::cell::Cell;
use std::ops::Deref;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{after, bounded, never, select, select_biased, tick, unbounded};
use crossbeam_channel::{Receiver, RecvError, SendError, Sender, TryRecvError};
use crossbeam_utils::thread::scope;

fn ms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

#[test]
fn smoke1() {
    let (s1, r1) = unbounded::<usize>();
    let (s2, r2) = unbounded::<usize>();

    s1.send(1).unwrap();

    select! {
        recv(r1) -> v => assert_eq!(v, Ok(1)),
        recv(r2) -> _ => panic!(),
    }

    s2.send(2).unwrap();

    select! {
        recv(r1) -> _ => panic!(),
        recv(r2) -> v => assert_eq!(v, Ok(2)),
    }
}

#[test]
fn smoke2() {
    let (_s1, r1) = unbounded::<i32>();
    let (_s2, r2) = unbounded::<i32>();
    let (_s3, r3) = unbounded::<i32>();
    let (_s4, r4) = unbounded::<i32>();
    let (s5, r5) = unbounded::<i32>();

    s5.send(5).unwrap();

    select! {
        recv(r1) -> _ => panic!(),
        recv(r2) -> _ => panic!(),
        recv(r3) -> _ => panic!(),
        recv(r4) -> _ => panic!(),
        recv(r5) -> v => assert_eq!(v, Ok(5)),
    }
}

#[test]
fn disconnected() {
    let (s1, r1) = unbounded::<i32>();
    let (s2, r2) = unbounded::<i32>();

    scope(|scope| {
        scope.spawn(|_| {
            drop(s1);
            thread::sleep(ms(500));
            s2.send(5).unwrap();
        });

        select! {
            recv(r1) -> v => assert!(v.is_err()),
            recv(r2) -> _ => panic!(),
            default(ms(1000)) => panic!(),
        }

        r2.recv().unwrap();
    })
    .unwrap();

    select! {
        recv(r1) -> v => assert!(v.is_err()),
        recv(r2) -> _ => panic!(),
        default(ms(1000)) => panic!(),
    }

    scope(|scope| {
        scope.spawn(|_| {
            thread::sleep(ms(500));
            drop(s2);
        });

        select! {
            recv(r2) -> v => assert!(v.is_err()),
            default(ms(1000)) => panic!(),
        }
    })
    .unwrap();
}

#[test]
fn default() {
    let (s1, r1) = unbounded::<i32>();
    let (s2, r2) = unbounded::<i32>();

    select! {
        recv(r1) -> _ => panic!(),
        recv(r2) -> _ => panic!(),
        default => {}
    }

    drop(s1);

    select! {
        recv(r1) -> v => assert!(v.is_err()),
        recv(r2) -> _ => panic!(),
        default => panic!(),
    }

    s2.send(2).unwrap();

    select! {
        recv(r2) -> v => assert_eq!(v, Ok(2)),
        default => panic!(),
    }

    select! {
        recv(r2) -> _ => panic!(),
        default => {},
    }

    select! {
        default => {},
    }
}

#[test]
fn timeout() {
    let (_s1, r1) = unbounded::<i32>();
    let (s2, r2) = unbounded::<i32>();

    scope(|scope| {
        scope.spawn(|_| {
            thread::sleep(ms(1500));
            s2.send(2).unwrap();
        });

        select! {
            recv(r1) -> _ => panic!(),
            recv(r2) -> _ => panic!(),
            default(ms(1000)) => {},
        }

        select! {
            recv(r1) -> _ => panic!(),
            recv(r2) -> v => assert_eq!(v, Ok(2)),
            default(ms(1000)) => panic!(),
        }
    })
    .unwrap();

    scope(|scope| {
        let (s, r) = unbounded::<i32>();

        scope.spawn(move |_| {
            thread::sleep(ms(500));
            drop(s);
        });

        select! {
            default(ms(1000)) => {
                select! {
                    recv(r) -> v => assert!(v.is_err()),
                    default => panic!(),
                }
            }
        }
    })
    .unwrap();
}

#[test]
fn default_when_disconnected() {
    let (_, r) = unbounded::<i32>();

    select! {
        recv(r) -> res => assert!(res.is_err()),
        default => panic!(),
    }

    let (_, r) = unbounded::<i32>();

    select! {
        recv(r) -> res => assert!(res.is_err()),
        default(ms(1000)) => panic!(),
    }

    let (s, _) = bounded::<i32>(0);

    select! {
        send(s, 0) -> res => assert!(res.is_err()),
        default => panic!(),
    }

    let (s, _) = bounded::<i32>(0);

    select! {
        send(s, 0) -> res => assert!(res.is_err()),
        default(ms(1000)) => panic!(),
    }
}

#[test]
fn default_only() {
    let start = Instant::now();
    select! {
        default => {}
    }
    let now = Instant::now();
    assert!(now - start <= ms(50));

    let start = Instant::now();
    select! {
        default(ms(500)) => {}
    }
    let now = Instant::now();
    assert!(now - start >= ms(450));
    assert!(now - start <= ms(550));
}

#[test]
fn unblocks() {
    let (s1, r1) = bounded::<i32>(0);
    let (s2, r2) = bounded::<i32>(0);

    scope(|scope| {
        scope.spawn(|_| {
            thread::sleep(ms(500));
            s2.send(2).unwrap();
        });

        select! {
            recv(r1) -> _ => panic!(),
            recv(r2) -> v => assert_eq!(v, Ok(2)),
            default(ms(1000)) => panic!(),
        }
    })
    .unwrap();

    scope(|scope| {
        scope.spawn(|_| {
            thread::sleep(ms(500));
            assert_eq!(r1.recv().unwrap(), 1);
        });

        select! {
            send(s1, 1) -> _ => {},
            send(s2, 2) -> _ => panic!(),
            default(ms(1000)) => panic!(),
        }
    })
    .unwrap();
}

#[test]
fn both_ready() {
    let (s1, r1) = bounded(0);
    let (s2, r2) = bounded(0);

    scope(|scope| {
        scope.spawn(|_| {
            thread::sleep(ms(500));
            s1.send(1).unwrap();
            assert_eq!(r2.recv().unwrap(), 2);
        });

        for _ in 0..2 {
            select! {
                recv(r1) -> v => assert_eq!(v, Ok(1)),
                send(s2, 2) -> _ => {},
            }
        }
    })
    .unwrap();
}

#[test]
fn loop_try() {
    const RUNS: usize = 20;

    for _ in 0..RUNS {
        let (s1, r1) = bounded::<i32>(0);
        let (s2, r2) = bounded::<i32>(0);
        let (s_end, r_end) = bounded::<()>(0);

        scope(|scope| {
            scope.spawn(|_| loop {
                select! {
                    send(s1, 1) -> _ => break,
                    default => {}
                }

                select! {
                    recv(r_end) -> _ => break,
                    default => {}
                }
            });

            scope.spawn(|_| loop {
                if let Ok(x) = r2.try_recv() {
                    assert_eq!(x, 2);
                    break;
                }

                select! {
                    recv(r_end) -> _ => break,
                    default => {}
                }
            });

            scope.spawn(|_| {
                thread::sleep(ms(500));

                select! {
                    recv(r1) -> v => assert_eq!(v, Ok(1)),
                    send(s2, 2) -> _ => {},
                    default(ms(500)) => panic!(),
                }

                drop(s_end);
            });
        })
        .unwrap();
    }
}

#[test]
fn cloning1() {
    scope(|scope| {
        let (s1, r1) = unbounded::<i32>();
        let (_s2, r2) = unbounded::<i32>();
        let (s3, r3) = unbounded::<()>();

        scope.spawn(move |_| {
            r3.recv().unwrap();
            drop(s1.clone());
            assert_eq!(r3.try_recv(), Err(TryRecvError::Empty));
            s1.send(1).unwrap();
            r3.recv().unwrap();
        });

        s3.send(()).unwrap();

        select! {
            recv(r1) -> _ => {},
            recv(r2) -> _ => {},
        }

        s3.send(()).unwrap();
    })
    .unwrap();
}

#[test]
fn cloning2() {
    let (s1, r1) = unbounded::<()>();
    let (s2, r2) = unbounded::<()>();
    let (_s3, _r3) = unbounded::<()>();

    scope(|scope| {
        scope.spawn(move |_| {
            select! {
                recv(r1) -> _ => panic!(),
                recv(r2) -> _ => {},
            }
        });

        thread::sleep(ms(500));
        drop(s1.clone());
        s2.send(()).unwrap();
    })
    .unwrap();
}

#[test]
fn preflight1() {
    let (s, r) = unbounded();
    s.send(()).unwrap();

    select! {
        recv(r) -> _ => {}
    }
}

#[test]
fn preflight2() {
    let (s, r) = unbounded();
    drop(s.clone());
    s.send(()).unwrap();
    drop(s);

    select! {
        recv(r) -> v => assert!(v.is_ok()),
    }
    assert_eq!(r.try_recv(), Err(TryRecvError::Disconnected));
}

#[test]
fn preflight3() {
    let (s, r) = unbounded();
    drop(s.clone());
    s.send(()).unwrap();
    drop(s);
    r.recv().unwrap();

    select! {
        recv(r) -> v => assert!(v.is_err())
    }
}

#[test]
fn duplicate_operations() {
    let (s, r) = unbounded::<i32>();
    let mut hit = [false; 4];

    while hit.iter().any(|hit| !hit) {
        select! {
            recv(r) -> _ => hit[0] = true,
            recv(r) -> _ => hit[1] = true,
            send(s, 0) -> _ => hit[2] = true,
            send(s, 0) -> _ => hit[3] = true,
        }
    }
}

#[test]
fn nesting() {
    let (s, r) = unbounded::<i32>();

    select! {
        send(s, 0) -> _ => {
            select! {
                recv(r) -> v => {
                    assert_eq!(v, Ok(0));
                    select! {
                        send(s, 1) -> _ => {
                            select! {
                                recv(r) -> v => {
                                    assert_eq!(v, Ok(1));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
#[should_panic(expected = "send panicked")]
fn panic_sender() {
    fn get() -> Sender<i32> {
        panic!("send panicked")
    }

    #[allow(unreachable_code)]
    {
        select! {
            send(get(), panic!()) -> _ => {}
        }
    }
}

#[test]
#[should_panic(expected = "recv panicked")]
fn panic_receiver() {
    fn get() -> Receiver<i32> {
        panic!("recv panicked")
    }

    select! {
        recv(get()) -> _ => {}
    }
}

#[test]
fn stress_recv() {
    #[cfg(miri)]
    const COUNT: usize = 50;
    #[cfg(not(miri))]
    const COUNT: usize = 10_000;

    let (s1, r1) = unbounded();
    let (s2, r2) = bounded(5);
    let (s3, r3) = bounded(100);

    scope(|scope| {
        scope.spawn(|_| {
            for i in 0..COUNT {
                s1.send(i).unwrap();
                r3.recv().unwrap();

                s2.send(i).unwrap();
                r3.recv().unwrap();
            }
        });

        for i in 0..COUNT {
            for _ in 0..2 {
                select! {
                    recv(r1) -> v => assert_eq!(v, Ok(i)),
                    recv(r2) -> v => assert_eq!(v, Ok(i)),
                }

                s3.send(()).unwrap();
            }
        }
    })
    .unwrap();
}

#[test]
fn stress_send() {
    #[cfg(miri)]
    const COUNT: usize = 100;
    #[cfg(not(miri))]
    const COUNT: usize = 10_000;

    let (s1, r1) = bounded(0);
    let (s2, r2) = bounded(0);
    let (s3, r3) = bounded(100);

    scope(|scope| {
        scope.spawn(|_| {
            for i in 0..COUNT {
                assert_eq!(r1.recv().unwrap(), i);
                assert_eq!(r2.recv().unwrap(), i);
                r3.recv().unwrap();
            }
        });

        for i in 0..COUNT {
            for _ in 0..2 {
                select! {
                    send(s1, i) -> _ => {},
                    send(s2, i) -> _ => {},
                }
            }
            s3.send(()).unwrap();
        }
    })
    .unwrap();
}

#[test]
fn stress_mixed() {
    #[cfg(miri)]
    const COUNT: usize = 100;
    #[cfg(not(miri))]
    const COUNT: usize = 10_000;

    let (s1, r1) = bounded(0);
    let (s2, r2) = bounded(0);
    let (s3, r3) = bounded(100);

    scope(|scope| {
        scope.spawn(|_| {
            for i in 0..COUNT {
                s1.send(i).unwrap();
                assert_eq!(r2.recv().unwrap(), i);
                r3.recv().unwrap();
            }
        });

        for i in 0..COUNT {
            for _ in 0..2 {
                select! {
                    recv(r1) -> v => assert_eq!(v, Ok(i)),
                    send(s2, i) -> _ => {},
                }
            }
            s3.send(()).unwrap();
        }
    })
    .unwrap();
}

#[test]
fn stress_timeout_two_threads() {
    const COUNT: usize = 20;

    let (s, r) = bounded(2);

    scope(|scope| {
        scope.spawn(|_| {
            for i in 0..COUNT {
                if i % 2 == 0 {
                    thread::sleep(ms(500));
                }

                loop {
                    select! {
                        send(s, i) -> _ => break,
                        default(ms(100)) => {}
                    }
                }
            }
        });

        scope.spawn(|_| {
            for i in 0..COUNT {
                if i % 2 == 0 {
                    thread::sleep(ms(500));
                }

                loop {
                    select! {
                        recv(r) -> v => {
                            assert_eq!(v, Ok(i));
                            break;
                        }
                        default(ms(100)) => {}
                    }
                }
            }
        });
    })
    .unwrap();
}

#[test]
fn send_recv_same_channel() {
    let (s, r) = bounded::<i32>(0);
    select! {
        send(s, 0) -> _ => panic!(),
        recv(r) -> _ => panic!(),
        default(ms(500)) => {}
    }

    let (s, r) = unbounded::<i32>();
    select! {
        send(s, 0) -> _ => {},
        recv(r) -> _ => panic!(),
        default(ms(500)) => panic!(),
    }
}

#[test]
fn matching() {
    const THREADS: usize = 44;

    let (s, r) = &bounded::<usize>(0);

    scope(|scope| {
        for i in 0..THREADS {
            scope.spawn(move |_| {
                select! {
                    recv(r) -> v => assert_ne!(v.unwrap(), i),
                    send(s, i) -> _ => {},
                }
            });
        }
    })
    .unwrap();

    assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
}

#[test]
fn matching_with_leftover() {
    const THREADS: usize = 55;

    let (s, r) = &bounded::<usize>(0);

    scope(|scope| {
        for i in 0..THREADS {
            scope.spawn(move |_| {
                select! {
                    recv(r) -> v => assert_ne!(v.unwrap(), i),
                    send(s, i) -> _ => {},
                }
            });
        }
        s.send(!0).unwrap();
    })
    .unwrap();

    assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
}

#[test]
fn channel_through_channel() {
    #[cfg(miri)]
    const COUNT: usize = 100;
    #[cfg(not(miri))]
    const COUNT: usize = 1000;

    type T = Box<dyn Any + Send>;

    for cap in 0..3 {
        let (s, r) = bounded::<T>(cap);

        scope(|scope| {
            scope.spawn(move |_| {
                let mut s = s;

                for _ in 0..COUNT {
                    let (new_s, new_r) = bounded(cap);
                    let new_r: T = Box::new(Some(new_r));

                    select! {
                        send(s, new_r) -> _ => {}
                    }

                    s = new_s;
                }
            });

            scope.spawn(move |_| {
                let mut r = r;

                for _ in 0..COUNT {
                    r = select! {
                        recv(r) -> msg => {
                            msg.unwrap()
                                .downcast_mut::<Option<Receiver<T>>>()
                                .unwrap()
                                .take()
                                .unwrap()
                        }
                    }
                }
            });
        })
        .unwrap();
    }
}

#[test]
fn linearizable_default() {
    #[cfg(miri)]
    const COUNT: usize = 100;
    #[cfg(not(miri))]
    const COUNT: usize = 100_000;

    for step in 0..2 {
        let (start_s, start_r) = bounded::<()>(0);
        let (end_s, end_r) = bounded::<()>(0);

        let ((s1, r1), (s2, r2)) = if step == 0 {
            (bounded::<i32>(1), bounded::<i32>(1))
        } else {
            (unbounded::<i32>(), unbounded::<i32>())
        };

        scope(|scope| {
            scope.spawn(|_| {
                for _ in 0..COUNT {
                    start_s.send(()).unwrap();

                    s1.send(1).unwrap();
                    select! {
                        recv(r1) -> _ => {}
                        recv(r2) -> _ => {}
                        default => unreachable!()
                    }

                    end_s.send(()).unwrap();
                    let _ = r2.try_recv();
                }
            });

            for _ in 0..COUNT {
                start_r.recv().unwrap();

                s2.send(1).unwrap();
                let _ = r1.try_recv();

                end_r.recv().unwrap();
            }
        })
        .unwrap();
    }
}

#[test]
fn linearizable_timeout() {
    #[cfg(miri)]
    const COUNT: usize = 100;
    #[cfg(not(miri))]
    const COUNT: usize = 100_000;

    for step in 0..2 {
        let (start_s, start_r) = bounded::<()>(0);
        let (end_s, end_r) = bounded::<()>(0);

        let ((s1, r1), (s2, r2)) = if step == 0 {
            (bounded::<i32>(1), bounded::<i32>(1))
        } else {
            (unbounded::<i32>(), unbounded::<i32>())
        };

        scope(|scope| {
            scope.spawn(|_| {
                for _ in 0..COUNT {
                    start_s.send(()).unwrap();

                    s1.send(1).unwrap();
                    select! {
                        recv(r1) -> _ => {}
                        recv(r2) -> _ => {}
                        default(ms(0)) => unreachable!()
                    }

                    end_s.send(()).unwrap();
                    let _ = r2.try_recv();
                }
            });

            for _ in 0..COUNT {
                start_r.recv().unwrap();

                s2.send(1).unwrap();
                let _ = r1.try_recv();

                end_r.recv().unwrap();
            }
        })
        .unwrap();
    }
}

#[test]
fn fairness1() {
    #[cfg(miri)]
    const COUNT: usize = 100;
    #[cfg(not(miri))]
    const COUNT: usize = 10_000;

    let (s1, r1) = bounded::<()>(COUNT);
    let (s2, r2) = unbounded::<()>();

    for _ in 0..COUNT {
        s1.send(()).unwrap();
        s2.send(()).unwrap();
    }

    let mut hits = [0usize; 4];
    for _ in 0..COUNT {
        select! {
            recv(r1) -> _ => hits[0] += 1,
            recv(r2) -> _ => hits[1] += 1,
            recv(after(ms(0))) -> _ => hits[2] += 1,
            recv(tick(ms(0))) -> _ => hits[3] += 1,
        }
    }
    assert!(hits.iter().all(|x| *x >= COUNT / hits.len() / 2));
}

#[cfg_attr(crossbeam_sanitize, ignore)] // TODO: flaky: https://github.com/crossbeam-rs/crossbeam/issues/1094
#[test]
fn fairness2() {
    #[cfg(miri)]
    const COUNT: usize = 100;
    #[cfg(not(miri))]
    const COUNT: usize = 10_000;

    let (s1, r1) = unbounded::<()>();
    let (s2, r2) = bounded::<()>(1);
    let (s3, r3) = bounded::<()>(0);

    scope(|scope| {
        scope.spawn(|_| {
            let (hole, _r) = bounded(0);

            for _ in 0..COUNT {
                let s1 = if s1.is_empty() { &s1 } else { &hole };
                let s2 = if s2.is_empty() { &s2 } else { &hole };

                select! {
                    send(s1, ()) -> res => assert!(res.is_ok()),
                    send(s2, ()) -> res => assert!(res.is_ok()),
                    send(s3, ()) -> res => assert!(res.is_ok()),
                }
            }
        });

        let hits = vec![Cell::new(0usize); 3];
        for _ in 0..COUNT {
            select! {
                recv(r1) -> _ => hits[0].set(hits[0].get() + 1),
                recv(r2) -> _ => hits[1].set(hits[1].get() + 1),
                recv(r3) -> _ => hits[2].set(hits[2].get() + 1),
            }
        }
        assert!(hits.iter().all(|x| x.get() >= COUNT / hits.len() / 50));
    })
    .unwrap();
}

#[test]
fn fairness_recv() {
    #[cfg(miri)]
    const COUNT: usize = 100;
    #[cfg(not(miri))]
    const COUNT: usize = 10_000;

    let (s1, r1) = bounded::<()>(COUNT);
    let (s2, r2) = unbounded::<()>();

    for _ in 0..COUNT {
        s1.send(()).unwrap();
        s2.send(()).unwrap();
    }

    let mut hits = [0usize; 2];
    while hits[0] + hits[1] < COUNT {
        select! {
            recv(r1) -> _ => hits[0] += 1,
            recv(r2) -> _ => hits[1] += 1,
        }
    }
    assert!(hits.iter().all(|x| *x >= COUNT / 4));
}

#[test]
fn fairness_send() {
    #[cfg(miri)]
    const COUNT: usize = 100;
    #[cfg(not(miri))]
    const COUNT: usize = 10_000;

    let (s1, _r1) = bounded::<()>(COUNT);
    let (s2, _r2) = unbounded::<()>();

    let mut hits = [0usize; 2];
    for _ in 0..COUNT {
        select! {
            send(s1, ()) -> _ => hits[0] += 1,
            send(s2, ()) -> _ => hits[1] += 1,
        }
    }
    assert!(hits.iter().all(|x| *x >= COUNT / 4));
}

#[test]
fn unfairness() {
    #[cfg(miri)]
    const COUNT: usize = 100;
    #[cfg(not(miri))]
    const COUNT: usize = 10_000;

    let (s1, r1) = unbounded::<()>();
    let (s2, r2) = unbounded::<()>();
    let (s3, r3) = unbounded::<()>();

    for _ in 0..COUNT {
        s1.send(()).unwrap();
        s2.send(()).unwrap();
    }
    s3.send(()).unwrap();

    let mut hits = [0usize; 3];
    for _ in 0..COUNT {
        select_biased! {
            recv(r1) -> _ => hits[0] += 1,
            recv(r2) -> _ => hits[1] += 1,
            recv(r3) -> _ => hits[2] += 1,
        }
    }
    assert_eq!(hits, [COUNT, 0, 0]);

    for _ in 0..COUNT {
        select_biased! {
            recv(r1) -> _ => hits[0] += 1,
            recv(r2) -> _ => hits[1] += 1,
            recv(r3) -> _ => hits[2] += 1,
        }
    }
    assert_eq!(hits, [COUNT, COUNT, 0]);
}

#[test]
fn unfairness_timeout() {
    #[cfg(miri)]
    const COUNT: usize = 100;
    #[cfg(not(miri))]
    const COUNT: usize = 10_000;

    let (s1, r1) = unbounded::<()>();
    let (s2, r2) = unbounded::<()>();
    let (s3, r3) = unbounded::<()>();

    for _ in 0..COUNT {
        s1.send(()).unwrap();
        s2.send(()).unwrap();
    }
    s3.send(()).unwrap();

    let mut hits = [0usize; 3];
    for _ in 0..COUNT {
        select_biased! {
            recv(r1) -> _ => hits[0] += 1,
            recv(r2) -> _ => hits[1] += 1,
            recv(r3) -> _ => hits[2] += 1,
            default(ms(1000)) => unreachable!(),
        }
    }
    assert_eq!(hits, [COUNT, 0, 0]);

    for _ in 0..COUNT {
        select_biased! {
            recv(r1) -> _ => hits[0] += 1,
            recv(r2) -> _ => hits[1] += 1,
            recv(r3) -> _ => hits[2] += 1,
            default(ms(1000)) => unreachable!(),
        }
    }
    assert_eq!(hits, [COUNT, COUNT, 0]);
}

#[test]
fn unfairness_try() {
    #[cfg(miri)]
    const COUNT: usize = 100;
    #[cfg(not(miri))]
    const COUNT: usize = 10_000;

    let (s1, r1) = unbounded::<()>();
    let (s2, r2) = unbounded::<()>();
    let (s3, r3) = unbounded::<()>();

    for _ in 0..COUNT {
        s1.send(()).unwrap();
        s2.send(()).unwrap();
    }
    s3.send(()).unwrap();

    let mut hits = [0usize; 3];
    for _ in 0..COUNT {
        select_biased! {
            recv(r1) -> _ => hits[0] += 1,
            recv(r2) -> _ => hits[1] += 1,
            recv(r3) -> _ => hits[2] += 1,
            default() => unreachable!(),
        }
    }
    assert_eq!(hits, [COUNT, 0, 0]);

    for _ in 0..COUNT {
        select_biased! {
            recv(r1) -> _ => hits[0] += 1,
            recv(r2) -> _ => hits[1] += 1,
            recv(r3) -> _ => hits[2] += 1,
            default() => unreachable!(),
        }
    }
    assert_eq!(hits, [COUNT, COUNT, 0]);
}

#[allow(clippy::or_fun_call, clippy::unnecessary_literal_unwrap)] // This is intentional.
#[test]
fn references() {
    let (s, r) = unbounded::<i32>();
    select! {
        send(s, 0) -> _ => {}
        recv(r) -> _ => {}
    }
    select! {
        send(&&&&s, 0) -> _ => {}
        recv(&&&&r) -> _ => {}
    }
    select! {
        recv(Some(&r).unwrap_or(&never())) -> _ => {},
        default => {}
    }
    select! {
        recv(Some(r).unwrap_or(never())) -> _ => {},
        default => {}
    }
}

#[test]
fn case_blocks() {
    let (s, r) = unbounded::<i32>();

    select! {
        recv(r) -> _ => 3.0,
        recv(r) -> _ => loop {
            unreachable!()
        },
        recv(r) -> _ => match 7 + 3 {
            _ => unreachable!()
        },
        default => 7.
    };

    select! {
        recv(r) -> msg => if msg.is_ok() {
            unreachable!()
        },
        default => ()
    }

    drop(s);
}

#[allow(clippy::redundant_closure_call)] // This is intentional.
#[test]
fn move_handles() {
    let (s, r) = unbounded::<i32>();
    select! {
        recv((move || r)()) -> _ => {}
        send((move || s)(), 0) -> _ => {}
    }
}

#[test]
fn infer_types() {
    let (s, r) = unbounded();
    select! {
        recv(r) -> _ => {}
        default => {}
    }
    s.send(()).unwrap();

    let (s, r) = unbounded();
    select! {
        send(s, ()) -> _ => {}
    }
    r.recv().unwrap();
}

#[test]
fn default_syntax() {
    let (s, r) = bounded::<i32>(0);

    select! {
        recv(r) -> _ => panic!(),
        default => {}
    }
    select! {
        send(s, 0) -> _ => panic!(),
        default() => {}
    }
    select! {
        default => {}
    }
    select! {
        default() => {}
    }
}

#[test]
fn same_variable_name() {
    let (_, r) = unbounded::<i32>();
    select! {
        recv(r) -> r => assert!(r.is_err()),
    }
}

#[test]
fn handles_on_heap() {
    let (s, r) = unbounded::<i32>();
    let (s, r) = (Box::new(s), Box::new(r));

    select! {
        send(*s, 0) -> _ => {}
        recv(*r) -> _ => {}
        default => {}
    }

    drop(s);
    drop(r);
}

#[test]
fn once_blocks() {
    let (s, r) = unbounded::<i32>();

    let once = Box::new(());
    select! {
        send(s, 0) -> _ => drop(once),
    }

    let once = Box::new(());
    select! {
        recv(r) -> _ => drop(once),
    }

    let once1 = Box::new(());
    let once2 = Box::new(());
    select! {
        send(s, 0) -> _ => drop(once1),
        default => drop(once2),
    }

    let once1 = Box::new(());
    let once2 = Box::new(());
    select! {
        recv(r) -> _ => drop(once1),
        default => drop(once2),
    }

    let once1 = Box::new(());
    let once2 = Box::new(());
    select! {
        recv(r) -> _ => drop(once1),
        send(s, 0) -> _ => drop(once2),
    }
}

#[test]
fn once_receiver() {
    let (_, r) = unbounded::<i32>();

    let once = Box::new(());
    let get = move || {
        drop(once);
        r
    };

    select! {
        recv(get()) -> _ => {}
    }
}

#[test]
fn once_sender() {
    let (s, _) = unbounded::<i32>();

    let once = Box::new(());
    let get = move || {
        drop(once);
        s
    };

    select! {
        send(get(), 5) -> _ => {}
    }
}

#[test]
fn parse_nesting() {
    let (_, r) = unbounded::<i32>();

    select! {
        recv(r) -> _ => {}
        recv(r) -> _ => {
            select! {
                recv(r) -> _ => {}
                recv(r) -> _ => {
                    select! {
                        recv(r) -> _ => {}
                        recv(r) -> _ => {
                            select! {
                                default => {}
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn evaluate() {
    let (s, r) = unbounded::<i32>();

    let v = select! {
        recv(r) -> _ => "foo".into(),
        send(s, 0) -> _ => "bar".to_owned(),
        default => "baz".to_string(),
    };
    assert_eq!(v, "bar");

    let v = select! {
        recv(r) -> _ => "foo".into(),
        default => "baz".to_string(),
    };
    assert_eq!(v, "foo");

    let v = select! {
        recv(r) -> _ => "foo".into(),
        default => "baz".to_string(),
    };
    assert_eq!(v, "baz");
}

#[test]
fn deref() {
    use crossbeam_channel as cc;

    struct Sender<T>(cc::Sender<T>);
    struct Receiver<T>(cc::Receiver<T>);

    impl<T> Deref for Receiver<T> {
        type Target = cc::Receiver<T>;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl<T> Deref for Sender<T> {
        type Target = cc::Sender<T>;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    let (s, r) = bounded::<i32>(0);
    let (s, r) = (Sender(s), Receiver(r));

    select! {
        send(s, 0) -> _ => panic!(),
        recv(r) -> _ => panic!(),
        default => {}
    }
}

#[test]
fn result_types() {
    let (s, _) = bounded::<i32>(0);
    let (_, r) = bounded::<i32>(0);

    select! {
        recv(r) -> res => { let _: Result<i32, RecvError> = res; },
    }
    select! {
        recv(r) -> res =>  { let _: Result<i32, RecvError> = res; },
        default => {}
    }
    select! {
        recv(r) -> res =>  { let _: Result<i32, RecvError> = res; },
        default(ms(0)) => {}
    }

    select! {
        send(s, 0) -> res => { let _: Result<(), SendError<i32>> = res; },
    }
    select! {
        send(s, 0) -> res =>  { let _: Result<(), SendError<i32>> = res; },
        default => {}
    }
    select! {
        send(s, 0) -> res =>  { let _: Result<(), SendError<i32>> = res; },
        default(ms(0)) => {}
    }

    select! {
        send(s, 0) -> res =>  { let _: Result<(), SendError<i32>> = res; },
        recv(r) -> res => { let _: Result<i32, RecvError> = res; },
    }
}

#[test]
fn try_recv() {
    let (s, r) = bounded(0);

    scope(|scope| {
        scope.spawn(move |_| {
            select! {
                recv(r) -> _ => panic!(),
                default => {}
            }
            thread::sleep(ms(1500));
            select! {
                recv(r) -> v => assert_eq!(v, Ok(7)),
                default => panic!(),
            }
            thread::sleep(ms(500));
            select! {
                recv(r) -> v => assert_eq!(v, Err(RecvError)),
                default => panic!(),
            }
        });
        scope.spawn(move |_| {
            thread::sleep(ms(1000));
            select! {
                send(s, 7) -> res => res.unwrap(),
            }
        });
    })
    .unwrap();
}

#[test]
fn recv() {
    let (s, r) = bounded(0);

    scope(|scope| {
        scope.spawn(move |_| {
            select! {
                recv(r) -> v => assert_eq!(v, Ok(7)),
            }
            thread::sleep(ms(1000));
            select! {
                recv(r) -> v => assert_eq!(v, Ok(8)),
            }
            thread::sleep(ms(1000));
            select! {
                recv(r) -> v => assert_eq!(v, Ok(9)),
            }
            select! {
                recv(r) -> v => assert_eq!(v, Err(RecvError)),
            }
        });
        scope.spawn(move |_| {
            thread::sleep(ms(1500));
            select! {
                send(s, 7) -> res => res.unwrap(),
            }
            select! {
                send(s, 8) -> res => res.unwrap(),
            }
            select! {
                send(s, 9) -> res => res.unwrap(),
            }
        });
    })
    .unwrap();
}

#[test]
fn recv_timeout() {
    let (s, r) = bounded::<i32>(0);

    scope(|scope| {
        scope.spawn(move |_| {
            select! {
                recv(r) -> _ => panic!(),
                default(ms(1000)) => {}
            }
            select! {
                recv(r) -> v => assert_eq!(v, Ok(7)),
                default(ms(1000)) => panic!(),
            }
            select! {
                recv(r) -> v => assert_eq!(v, Err(RecvError)),
                default(ms(1000)) => panic!(),
            }
        });
        scope.spawn(move |_| {
            thread::sleep(ms(1500));
            select! {
                send(s, 7) -> res => res.unwrap(),
            }
        });
    })
    .unwrap();
}

#[test]
fn try_send() {
    let (s, r) = bounded(0);

    scope(|scope| {
        scope.spawn(move |_| {
            select! {
                send(s, 7) -> _ => panic!(),
                default => {}
            }
            thread::sleep(ms(1500));
            select! {
                send(s, 8) -> res => res.unwrap(),
                default => panic!(),
            }
            thread::sleep(ms(500));
            select! {
                send(s, 8) -> res => assert_eq!(res, Err(SendError(8))),
                default => panic!(),
            }
        });
        scope.spawn(move |_| {
            thread::sleep(ms(1000));
            select! {
                recv(r) -> v => assert_eq!(v, Ok(8)),
            }
        });
    })
    .unwrap();
}

#[test]
fn send() {
    let (s, r) = bounded(0);

    scope(|scope| {
        scope.spawn(move |_| {
            select! {
                send(s, 7) -> res => res.unwrap(),
            }
            thread::sleep(ms(1000));
            select! {
                send(s, 8) -> res => res.unwrap(),
            }
            thread::sleep(ms(1000));
            select! {
                send(s, 9) -> res => res.unwrap(),
            }
        });
        scope.spawn(move |_| {
            thread::sleep(ms(1500));
            select! {
                recv(r) -> v => assert_eq!(v, Ok(7)),
            }
            select! {
                recv(r) -> v => assert_eq!(v, Ok(8)),
            }
            select! {
                recv(r) -> v => assert_eq!(v, Ok(9)),
            }
        });
    })
    .unwrap();
}

#[test]
fn send_timeout() {
    let (s, r) = bounded(0);

    scope(|scope| {
        scope.spawn(move |_| {
            select! {
                send(s, 7) -> _ => panic!(),
                default(ms(1000)) => {}
            }
            select! {
                send(s, 8) -> res => res.unwrap(),
                default(ms(1000)) => panic!(),
            }
            select! {
                send(s, 9) -> res => assert_eq!(res, Err(SendError(9))),
                default(ms(1000)) => panic!(),
            }
        });
        scope.spawn(move |_| {
            thread::sleep(ms(1500));
            select! {
                recv(r) -> v => assert_eq!(v, Ok(8)),
            }
        });
    })
    .unwrap();
}

#[test]
fn disconnect_wakes_sender() {
    let (s, r) = bounded(0);

    scope(|scope| {
        scope.spawn(move |_| {
            select! {
                send(s, ()) -> res => assert_eq!(res, Err(SendError(()))),
            }
        });
        scope.spawn(move |_| {
            thread::sleep(ms(1000));
            drop(r);
        });
    })
    .unwrap();
}

#[test]
fn disconnect_wakes_receiver() {
    let (s, r) = bounded::<()>(0);

    scope(|scope| {
        scope.spawn(move |_| {
            select! {
                recv(r) -> res => assert_eq!(res, Err(RecvError)),
            }
        });
        scope.spawn(move |_| {
            thread::sleep(ms(1000));
            drop(s);
        });
    })
    .unwrap();
}

#[test]
fn trailing_comma() {
    let (s, r) = unbounded::<usize>();

    select! {
        send(s, 1,) -> _ => {},
        recv(r,) -> _ => {},
        default(ms(1000),) => {},
    }
}
