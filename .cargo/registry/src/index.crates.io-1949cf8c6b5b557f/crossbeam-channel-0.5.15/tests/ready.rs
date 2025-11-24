//! Tests for channel readiness using the `Select` struct.

use std::any::Any;
use std::cell::Cell;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{after, bounded, tick, unbounded};
use crossbeam_channel::{Receiver, Select, TryRecvError, TrySendError};
use crossbeam_utils::thread::scope;

fn ms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

#[test]
fn smoke1() {
    let (s1, r1) = unbounded::<usize>();
    let (s2, r2) = unbounded::<usize>();

    s1.send(1).unwrap();

    let mut sel = Select::new();
    sel.recv(&r1);
    sel.recv(&r2);
    assert_eq!(sel.ready(), 0);
    assert_eq!(r1.try_recv(), Ok(1));

    s2.send(2).unwrap();

    let mut sel = Select::new();
    sel.recv(&r1);
    sel.recv(&r2);
    assert_eq!(sel.ready(), 1);
    assert_eq!(r2.try_recv(), Ok(2));
}

#[test]
fn smoke2() {
    let (_s1, r1) = unbounded::<i32>();
    let (_s2, r2) = unbounded::<i32>();
    let (_s3, r3) = unbounded::<i32>();
    let (_s4, r4) = unbounded::<i32>();
    let (s5, r5) = unbounded::<i32>();

    s5.send(5).unwrap();

    let mut sel = Select::new();
    sel.recv(&r1);
    sel.recv(&r2);
    sel.recv(&r3);
    sel.recv(&r4);
    sel.recv(&r5);
    assert_eq!(sel.ready(), 4);
    assert_eq!(r5.try_recv(), Ok(5));
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

        let mut sel = Select::new();
        sel.recv(&r1);
        sel.recv(&r2);
        match sel.ready_timeout(ms(1000)) {
            Ok(0) => assert_eq!(r1.try_recv(), Err(TryRecvError::Disconnected)),
            _ => panic!(),
        }

        r2.recv().unwrap();
    })
    .unwrap();

    let mut sel = Select::new();
    sel.recv(&r1);
    sel.recv(&r2);
    match sel.ready_timeout(ms(1000)) {
        Ok(0) => assert_eq!(r1.try_recv(), Err(TryRecvError::Disconnected)),
        _ => panic!(),
    }

    scope(|scope| {
        scope.spawn(|_| {
            thread::sleep(ms(500));
            drop(s2);
        });

        let mut sel = Select::new();
        sel.recv(&r2);
        match sel.ready_timeout(ms(1000)) {
            Ok(0) => assert_eq!(r2.try_recv(), Err(TryRecvError::Disconnected)),
            _ => panic!(),
        }
    })
    .unwrap();
}

#[test]
fn default() {
    let (s1, r1) = unbounded::<i32>();
    let (s2, r2) = unbounded::<i32>();

    let mut sel = Select::new();
    sel.recv(&r1);
    sel.recv(&r2);
    assert!(sel.try_ready().is_err());

    drop(s1);

    let mut sel = Select::new();
    sel.recv(&r1);
    sel.recv(&r2);
    match sel.try_ready() {
        Ok(0) => assert!(r1.try_recv().is_err()),
        _ => panic!(),
    }

    s2.send(2).unwrap();

    let mut sel = Select::new();
    sel.recv(&r2);
    match sel.try_ready() {
        Ok(0) => assert_eq!(r2.try_recv(), Ok(2)),
        _ => panic!(),
    }

    let mut sel = Select::new();
    sel.recv(&r2);
    assert!(sel.try_ready().is_err());

    let mut sel = Select::new();
    assert!(sel.try_ready().is_err());
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

        let mut sel = Select::new();
        sel.recv(&r1);
        sel.recv(&r2);
        assert!(sel.ready_timeout(ms(1000)).is_err());

        let mut sel = Select::new();
        sel.recv(&r1);
        sel.recv(&r2);
        match sel.ready_timeout(ms(1000)) {
            Ok(1) => assert_eq!(r2.try_recv(), Ok(2)),
            _ => panic!(),
        }
    })
    .unwrap();

    scope(|scope| {
        let (s, r) = unbounded::<i32>();

        scope.spawn(move |_| {
            thread::sleep(ms(500));
            drop(s);
        });

        let mut sel = Select::new();
        assert!(sel.ready_timeout(ms(1000)).is_err());

        let mut sel = Select::new();
        sel.recv(&r);
        match sel.try_ready() {
            Ok(0) => assert_eq!(r.try_recv(), Err(TryRecvError::Disconnected)),
            _ => panic!(),
        }
    })
    .unwrap();
}

#[test]
fn default_when_disconnected() {
    let (_, r) = unbounded::<i32>();

    let mut sel = Select::new();
    sel.recv(&r);
    match sel.try_ready() {
        Ok(0) => assert_eq!(r.try_recv(), Err(TryRecvError::Disconnected)),
        _ => panic!(),
    }

    let (_, r) = unbounded::<i32>();

    let mut sel = Select::new();
    sel.recv(&r);
    match sel.ready_timeout(ms(1000)) {
        Ok(0) => assert_eq!(r.try_recv(), Err(TryRecvError::Disconnected)),
        _ => panic!(),
    }

    let (s, _) = bounded::<i32>(0);

    let mut sel = Select::new();
    sel.send(&s);
    match sel.try_ready() {
        Ok(0) => assert_eq!(s.try_send(0), Err(TrySendError::Disconnected(0))),
        _ => panic!(),
    }

    let (s, _) = bounded::<i32>(0);

    let mut sel = Select::new();
    sel.send(&s);
    match sel.ready_timeout(ms(1000)) {
        Ok(0) => assert_eq!(s.try_send(0), Err(TrySendError::Disconnected(0))),
        _ => panic!(),
    }
}

#[test]
#[cfg_attr(miri, ignore)] // this test makes timing assumptions, but Miri is so slow it violates them
fn default_only() {
    let start = Instant::now();

    let mut sel = Select::new();
    assert!(sel.try_ready().is_err());
    let now = Instant::now();
    assert!(now - start <= ms(50));

    let start = Instant::now();
    let mut sel = Select::new();
    assert!(sel.ready_timeout(ms(500)).is_err());
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

        let mut sel = Select::new();
        sel.recv(&r1);
        sel.recv(&r2);
        match sel.ready_timeout(ms(1000)) {
            Ok(1) => assert_eq!(r2.try_recv(), Ok(2)),
            _ => panic!(),
        }
    })
    .unwrap();

    scope(|scope| {
        scope.spawn(|_| {
            thread::sleep(ms(500));
            assert_eq!(r1.recv().unwrap(), 1);
        });

        let mut sel = Select::new();
        let oper1 = sel.send(&s1);
        let oper2 = sel.send(&s2);
        let oper = sel.select_timeout(ms(1000));
        match oper {
            Err(_) => panic!(),
            Ok(oper) => match oper.index() {
                i if i == oper1 => oper.send(&s1, 1).unwrap(),
                i if i == oper2 => panic!(),
                _ => unreachable!(),
            },
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
            let mut sel = Select::new();
            sel.recv(&r1);
            sel.send(&s2);
            match sel.ready() {
                0 => assert_eq!(r1.try_recv(), Ok(1)),
                1 => s2.try_send(2).unwrap(),
                _ => panic!(),
            }
        }
    })
    .unwrap();
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
            assert!(r3.try_recv().is_err());
            s1.send(1).unwrap();
            r3.recv().unwrap();
        });

        s3.send(()).unwrap();

        let mut sel = Select::new();
        sel.recv(&r1);
        sel.recv(&r2);
        match sel.ready() {
            0 => drop(r1.try_recv()),
            1 => drop(r2.try_recv()),
            _ => panic!(),
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
            let mut sel = Select::new();
            sel.recv(&r1);
            sel.recv(&r2);
            match sel.ready() {
                0 => panic!(),
                1 => drop(r2.try_recv()),
                _ => panic!(),
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

    let mut sel = Select::new();
    sel.recv(&r);
    match sel.ready() {
        0 => drop(r.try_recv()),
        _ => panic!(),
    }
}

#[test]
fn preflight2() {
    let (s, r) = unbounded();
    drop(s.clone());
    s.send(()).unwrap();
    drop(s);

    let mut sel = Select::new();
    sel.recv(&r);
    match sel.ready() {
        0 => assert_eq!(r.try_recv(), Ok(())),
        _ => panic!(),
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

    let mut sel = Select::new();
    sel.recv(&r);
    match sel.ready() {
        0 => assert_eq!(r.try_recv(), Err(TryRecvError::Disconnected)),
        _ => panic!(),
    }
}

#[test]
fn duplicate_operations() {
    let (s, r) = unbounded::<i32>();
    let hit = vec![Cell::new(false); 4];

    while hit.iter().map(|h| h.get()).any(|hit| !hit) {
        let mut sel = Select::new();
        sel.recv(&r);
        sel.recv(&r);
        sel.send(&s);
        sel.send(&s);
        match sel.ready() {
            0 => {
                assert!(r.try_recv().is_ok());
                hit[0].set(true);
            }
            1 => {
                assert!(r.try_recv().is_ok());
                hit[1].set(true);
            }
            2 => {
                assert!(s.try_send(0).is_ok());
                hit[2].set(true);
            }
            3 => {
                assert!(s.try_send(0).is_ok());
                hit[3].set(true);
            }
            _ => panic!(),
        }
    }
}

#[test]
fn nesting() {
    let (s, r) = unbounded::<i32>();

    let mut sel = Select::new();
    sel.send(&s);
    match sel.ready() {
        0 => {
            assert!(s.try_send(0).is_ok());

            let mut sel = Select::new();
            sel.recv(&r);
            match sel.ready() {
                0 => {
                    assert_eq!(r.try_recv(), Ok(0));

                    let mut sel = Select::new();
                    sel.send(&s);
                    match sel.ready() {
                        0 => {
                            assert!(s.try_send(1).is_ok());

                            let mut sel = Select::new();
                            sel.recv(&r);
                            match sel.ready() {
                                0 => {
                                    assert_eq!(r.try_recv(), Ok(1));
                                }
                                _ => panic!(),
                            }
                        }
                        _ => panic!(),
                    }
                }
                _ => panic!(),
            }
        }
        _ => panic!(),
    }
}

#[test]
fn stress_recv() {
    #[cfg(miri)]
    const COUNT: usize = 100;
    #[cfg(not(miri))]
    const COUNT: usize = 10_000;

    let (s1, r1) = unbounded();
    let (s2, r2) = bounded(5);
    let (s3, r3) = bounded(0);

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
                let mut sel = Select::new();
                sel.recv(&r1);
                sel.recv(&r2);
                match sel.ready() {
                    0 => assert_eq!(r1.try_recv(), Ok(i)),
                    1 => assert_eq!(r2.try_recv(), Ok(i)),
                    _ => panic!(),
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
                let mut sel = Select::new();
                sel.send(&s1);
                sel.send(&s2);
                match sel.ready() {
                    0 => assert!(s1.try_send(i).is_ok()),
                    1 => assert!(s2.try_send(i).is_ok()),
                    _ => panic!(),
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
                let mut sel = Select::new();
                sel.recv(&r1);
                sel.send(&s2);
                match sel.ready() {
                    0 => assert_eq!(r1.try_recv(), Ok(i)),
                    1 => assert!(s2.try_send(i).is_ok()),
                    _ => panic!(),
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
                    let mut sel = Select::new();
                    sel.send(&s);
                    match sel.ready_timeout(ms(100)) {
                        Err(_) => {}
                        Ok(0) => {
                            assert!(s.try_send(i).is_ok());
                            break;
                        }
                        Ok(_) => panic!(),
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
                    let mut sel = Select::new();
                    sel.recv(&r);
                    match sel.ready_timeout(ms(100)) {
                        Err(_) => {}
                        Ok(0) => {
                            assert_eq!(r.try_recv(), Ok(i));
                            break;
                        }
                        Ok(_) => panic!(),
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
    let mut sel = Select::new();
    sel.send(&s);
    sel.recv(&r);
    assert!(sel.ready_timeout(ms(100)).is_err());

    let (s, r) = unbounded::<i32>();
    let mut sel = Select::new();
    sel.send(&s);
    sel.recv(&r);
    match sel.ready_timeout(ms(100)) {
        Err(_) => panic!(),
        Ok(0) => assert!(s.try_send(0).is_ok()),
        Ok(_) => panic!(),
    }
}

#[test]
fn channel_through_channel() {
    #[cfg(miri)]
    const COUNT: usize = 100;
    #[cfg(not(miri))]
    const COUNT: usize = 1000;

    type T = Box<dyn Any + Send>;

    for cap in 1..4 {
        let (s, r) = bounded::<T>(cap);

        scope(|scope| {
            scope.spawn(move |_| {
                let mut s = s;

                for _ in 0..COUNT {
                    let (new_s, new_r) = bounded(cap);
                    let new_r: T = Box::new(Some(new_r));

                    {
                        let mut sel = Select::new();
                        sel.send(&s);
                        match sel.ready() {
                            0 => assert!(s.try_send(new_r).is_ok()),
                            _ => panic!(),
                        }
                    }

                    s = new_s;
                }
            });

            scope.spawn(move |_| {
                let mut r = r;

                for _ in 0..COUNT {
                    let new = {
                        let mut sel = Select::new();
                        sel.recv(&r);
                        match sel.ready() {
                            0 => r
                                .try_recv()
                                .unwrap()
                                .downcast_mut::<Option<Receiver<T>>>()
                                .unwrap()
                                .take()
                                .unwrap(),
                            _ => panic!(),
                        }
                    };
                    r = new;
                }
            });
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

    let hits = vec![Cell::new(0usize); 4];
    for _ in 0..COUNT {
        let after = after(ms(0));
        let tick = tick(ms(0));

        let mut sel = Select::new();
        sel.recv(&r1);
        sel.recv(&r2);
        sel.recv(&after);
        sel.recv(&tick);
        match sel.ready() {
            0 => {
                r1.try_recv().unwrap();
                hits[0].set(hits[0].get() + 1);
            }
            1 => {
                r2.try_recv().unwrap();
                hits[1].set(hits[1].get() + 1);
            }
            2 => {
                after.try_recv().unwrap();
                hits[2].set(hits[2].get() + 1);
            }
            3 => {
                tick.try_recv().unwrap();
                hits[3].set(hits[3].get() + 1);
            }
            _ => panic!(),
        }
    }
    assert!(hits.iter().all(|x| x.get() >= COUNT / hits.len() / 2));
}

#[test]
fn fairness2() {
    #[cfg(miri)]
    const COUNT: usize = 100;
    #[cfg(not(miri))]
    const COUNT: usize = 100_000;

    let (s1, r1) = unbounded::<()>();
    let (s2, r2) = bounded::<()>(1);
    let (s3, r3) = bounded::<()>(0);

    scope(|scope| {
        scope.spawn(|_| {
            for _ in 0..COUNT {
                let mut sel = Select::new();
                let mut oper1 = None;
                let mut oper2 = None;
                if s1.is_empty() {
                    oper1 = Some(sel.send(&s1));
                }
                if s2.is_empty() {
                    oper2 = Some(sel.send(&s2));
                }
                let oper3 = sel.send(&s3);
                let oper = sel.select();
                match oper.index() {
                    i if Some(i) == oper1 => assert!(oper.send(&s1, ()).is_ok()),
                    i if Some(i) == oper2 => assert!(oper.send(&s2, ()).is_ok()),
                    i if i == oper3 => assert!(oper.send(&s3, ()).is_ok()),
                    _ => unreachable!(),
                }
            }
        });

        let hits = vec![Cell::new(0usize); 3];
        for _ in 0..COUNT {
            let mut sel = Select::new();
            sel.recv(&r1);
            sel.recv(&r2);
            sel.recv(&r3);
            loop {
                match sel.ready() {
                    0 => {
                        if r1.try_recv().is_ok() {
                            hits[0].set(hits[0].get() + 1);
                            break;
                        }
                    }
                    1 => {
                        if r2.try_recv().is_ok() {
                            hits[1].set(hits[1].get() + 1);
                            break;
                        }
                    }
                    2 => {
                        if r3.try_recv().is_ok() {
                            hits[2].set(hits[2].get() + 1);
                            break;
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
        assert!(hits.iter().all(|x| x.get() > 0));
    })
    .unwrap();
}
