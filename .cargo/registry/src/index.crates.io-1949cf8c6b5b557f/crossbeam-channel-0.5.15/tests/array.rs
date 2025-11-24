//! Tests for the array channel flavor.

use std::any::Any;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{bounded, select, Receiver};
use crossbeam_channel::{RecvError, RecvTimeoutError, TryRecvError};
use crossbeam_channel::{SendError, SendTimeoutError, TrySendError};
use crossbeam_utils::thread::scope;
use rand::{thread_rng, Rng};

fn ms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

#[test]
fn smoke() {
    let (s, r) = bounded(1);
    s.send(7).unwrap();
    assert_eq!(r.try_recv(), Ok(7));

    s.send(8).unwrap();
    assert_eq!(r.recv(), Ok(8));

    assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
    assert_eq!(r.recv_timeout(ms(1000)), Err(RecvTimeoutError::Timeout));
}

#[test]
fn capacity() {
    for i in 1..10 {
        let (s, r) = bounded::<()>(i);
        assert_eq!(s.capacity(), Some(i));
        assert_eq!(r.capacity(), Some(i));
    }
}

#[test]
fn len_empty_full() {
    let (s, r) = bounded(2);

    assert_eq!(s.len(), 0);
    assert!(s.is_empty());
    assert!(!s.is_full());
    assert_eq!(r.len(), 0);
    assert!(r.is_empty());
    assert!(!r.is_full());

    s.send(()).unwrap();

    assert_eq!(s.len(), 1);
    assert!(!s.is_empty());
    assert!(!s.is_full());
    assert_eq!(r.len(), 1);
    assert!(!r.is_empty());
    assert!(!r.is_full());

    s.send(()).unwrap();

    assert_eq!(s.len(), 2);
    assert!(!s.is_empty());
    assert!(s.is_full());
    assert_eq!(r.len(), 2);
    assert!(!r.is_empty());
    assert!(r.is_full());

    r.recv().unwrap();

    assert_eq!(s.len(), 1);
    assert!(!s.is_empty());
    assert!(!s.is_full());
    assert_eq!(r.len(), 1);
    assert!(!r.is_empty());
    assert!(!r.is_full());
}

#[test]
fn try_recv() {
    let (s, r) = bounded(100);

    scope(|scope| {
        scope.spawn(move |_| {
            assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
            thread::sleep(ms(1500));
            assert_eq!(r.try_recv(), Ok(7));
            thread::sleep(ms(500));
            assert_eq!(r.try_recv(), Err(TryRecvError::Disconnected));
        });
        scope.spawn(move |_| {
            thread::sleep(ms(1000));
            s.send(7).unwrap();
        });
    })
    .unwrap();
}

#[test]
fn recv() {
    let (s, r) = bounded(100);

    scope(|scope| {
        scope.spawn(move |_| {
            assert_eq!(r.recv(), Ok(7));
            thread::sleep(ms(1000));
            assert_eq!(r.recv(), Ok(8));
            thread::sleep(ms(1000));
            assert_eq!(r.recv(), Ok(9));
            assert_eq!(r.recv(), Err(RecvError));
        });
        scope.spawn(move |_| {
            thread::sleep(ms(1500));
            s.send(7).unwrap();
            s.send(8).unwrap();
            s.send(9).unwrap();
        });
    })
    .unwrap();
}

#[test]
fn recv_timeout() {
    let (s, r) = bounded::<i32>(100);

    scope(|scope| {
        scope.spawn(move |_| {
            assert_eq!(r.recv_timeout(ms(1000)), Err(RecvTimeoutError::Timeout));
            assert_eq!(r.recv_timeout(ms(1000)), Ok(7));
            assert_eq!(
                r.recv_timeout(ms(1000)),
                Err(RecvTimeoutError::Disconnected)
            );
        });
        scope.spawn(move |_| {
            thread::sleep(ms(1500));
            s.send(7).unwrap();
        });
    })
    .unwrap();
}

#[test]
fn try_send() {
    let (s, r) = bounded(1);

    scope(|scope| {
        scope.spawn(move |_| {
            assert_eq!(s.try_send(1), Ok(()));
            assert_eq!(s.try_send(2), Err(TrySendError::Full(2)));
            thread::sleep(ms(1500));
            assert_eq!(s.try_send(3), Ok(()));
            thread::sleep(ms(500));
            assert_eq!(s.try_send(4), Err(TrySendError::Disconnected(4)));
        });
        scope.spawn(move |_| {
            thread::sleep(ms(1000));
            assert_eq!(r.try_recv(), Ok(1));
            assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
            assert_eq!(r.recv(), Ok(3));
        });
    })
    .unwrap();
}

#[test]
fn send() {
    let (s, r) = bounded(1);

    scope(|scope| {
        scope.spawn(|_| {
            s.send(7).unwrap();
            thread::sleep(ms(1000));
            s.send(8).unwrap();
            thread::sleep(ms(1000));
            s.send(9).unwrap();
            thread::sleep(ms(1000));
            s.send(10).unwrap();
        });
        scope.spawn(|_| {
            thread::sleep(ms(1500));
            assert_eq!(r.recv(), Ok(7));
            assert_eq!(r.recv(), Ok(8));
            assert_eq!(r.recv(), Ok(9));
        });
    })
    .unwrap();
}

#[test]
fn send_timeout() {
    let (s, r) = bounded(2);

    scope(|scope| {
        scope.spawn(move |_| {
            assert_eq!(s.send_timeout(1, ms(1000)), Ok(()));
            assert_eq!(s.send_timeout(2, ms(1000)), Ok(()));
            assert_eq!(
                s.send_timeout(3, ms(500)),
                Err(SendTimeoutError::Timeout(3))
            );
            thread::sleep(ms(1000));
            assert_eq!(s.send_timeout(4, ms(1000)), Ok(()));
            thread::sleep(ms(1000));
            assert_eq!(s.send(5), Err(SendError(5)));
        });
        scope.spawn(move |_| {
            thread::sleep(ms(1000));
            assert_eq!(r.recv(), Ok(1));
            thread::sleep(ms(1000));
            assert_eq!(r.recv(), Ok(2));
            assert_eq!(r.recv(), Ok(4));
        });
    })
    .unwrap();
}

#[test]
fn send_after_disconnect() {
    let (s, r) = bounded(100);

    s.send(1).unwrap();
    s.send(2).unwrap();
    s.send(3).unwrap();

    drop(r);

    assert_eq!(s.send(4), Err(SendError(4)));
    assert_eq!(s.try_send(5), Err(TrySendError::Disconnected(5)));
    assert_eq!(
        s.send_timeout(6, ms(500)),
        Err(SendTimeoutError::Disconnected(6))
    );
}

#[test]
fn recv_after_disconnect() {
    let (s, r) = bounded(100);

    s.send(1).unwrap();
    s.send(2).unwrap();
    s.send(3).unwrap();

    drop(s);

    assert_eq!(r.recv(), Ok(1));
    assert_eq!(r.recv(), Ok(2));
    assert_eq!(r.recv(), Ok(3));
    assert_eq!(r.recv(), Err(RecvError));
}

#[test]
fn len() {
    #[cfg(miri)]
    const COUNT: usize = 50;
    #[cfg(not(miri))]
    const COUNT: usize = 25_000;
    #[cfg(miri)]
    const CAP: usize = 50;
    #[cfg(not(miri))]
    const CAP: usize = 1000;

    let (s, r) = bounded(CAP);

    assert_eq!(s.len(), 0);
    assert_eq!(r.len(), 0);

    for _ in 0..CAP / 10 {
        for i in 0..50 {
            s.send(i).unwrap();
            assert_eq!(s.len(), i + 1);
        }

        for i in 0..50 {
            r.recv().unwrap();
            assert_eq!(r.len(), 50 - i - 1);
        }
    }

    assert_eq!(s.len(), 0);
    assert_eq!(r.len(), 0);

    for i in 0..CAP {
        s.send(i).unwrap();
        assert_eq!(s.len(), i + 1);
    }

    for _ in 0..CAP {
        r.recv().unwrap();
    }

    assert_eq!(s.len(), 0);
    assert_eq!(r.len(), 0);

    scope(|scope| {
        scope.spawn(|_| {
            for i in 0..COUNT {
                assert_eq!(r.recv(), Ok(i));
                let len = r.len();
                assert!(len <= CAP);
            }
        });

        scope.spawn(|_| {
            for i in 0..COUNT {
                s.send(i).unwrap();
                let len = s.len();
                assert!(len <= CAP);
            }
        });
    })
    .unwrap();

    assert_eq!(s.len(), 0);
    assert_eq!(r.len(), 0);
}

#[test]
fn disconnect_wakes_sender() {
    let (s, r) = bounded(1);

    scope(|scope| {
        scope.spawn(move |_| {
            assert_eq!(s.send(()), Ok(()));
            assert_eq!(s.send(()), Err(SendError(())));
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
    let (s, r) = bounded::<()>(1);

    scope(|scope| {
        scope.spawn(move |_| {
            assert_eq!(r.recv(), Err(RecvError));
        });
        scope.spawn(move |_| {
            thread::sleep(ms(1000));
            drop(s);
        });
    })
    .unwrap();
}

#[test]
fn spsc() {
    #[cfg(miri)]
    const COUNT: usize = 100;
    #[cfg(not(miri))]
    const COUNT: usize = 100_000;

    let (s, r) = bounded(3);

    scope(|scope| {
        scope.spawn(move |_| {
            for i in 0..COUNT {
                assert_eq!(r.recv(), Ok(i));
            }
            assert_eq!(r.recv(), Err(RecvError));
        });
        scope.spawn(move |_| {
            for i in 0..COUNT {
                s.send(i).unwrap();
            }
        });
    })
    .unwrap();
}

#[test]
fn mpmc() {
    #[cfg(miri)]
    const COUNT: usize = 50;
    #[cfg(not(miri))]
    const COUNT: usize = 25_000;
    const THREADS: usize = 4;

    let (s, r) = bounded::<usize>(3);
    let v = (0..COUNT).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>();

    scope(|scope| {
        for _ in 0..THREADS {
            scope.spawn(|_| {
                for _ in 0..COUNT {
                    let n = r.recv().unwrap();
                    v[n].fetch_add(1, Ordering::SeqCst);
                }
            });
        }
        for _ in 0..THREADS {
            scope.spawn(|_| {
                for i in 0..COUNT {
                    s.send(i).unwrap();
                }
            });
        }
    })
    .unwrap();

    for c in v {
        assert_eq!(c.load(Ordering::SeqCst), THREADS);
    }
}

#[test]
fn stress_oneshot() {
    #[cfg(miri)]
    const COUNT: usize = 100;
    #[cfg(not(miri))]
    const COUNT: usize = 10_000;

    for _ in 0..COUNT {
        let (s, r) = bounded(1);

        scope(|scope| {
            scope.spawn(|_| r.recv().unwrap());
            scope.spawn(|_| s.send(0).unwrap());
        })
        .unwrap();
    }
}

#[test]
fn stress_iter() {
    #[cfg(miri)]
    const COUNT: usize = 100;
    #[cfg(not(miri))]
    const COUNT: usize = 100_000;

    let (request_s, request_r) = bounded(1);
    let (response_s, response_r) = bounded(1);

    scope(|scope| {
        scope.spawn(move |_| {
            let mut count = 0;
            loop {
                for x in response_r.try_iter() {
                    count += x;
                    if count == COUNT {
                        return;
                    }
                }
                request_s.send(()).unwrap();
            }
        });

        for _ in request_r.iter() {
            if response_s.send(1).is_err() {
                break;
            }
        }
    })
    .unwrap();
}

#[test]
fn stress_timeout_two_threads() {
    const COUNT: usize = 100;

    let (s, r) = bounded(2);

    scope(|scope| {
        scope.spawn(|_| {
            for i in 0..COUNT {
                if i % 2 == 0 {
                    thread::sleep(ms(50));
                }
                loop {
                    if let Ok(()) = s.send_timeout(i, ms(10)) {
                        break;
                    }
                }
            }
        });

        scope.spawn(|_| {
            for i in 0..COUNT {
                if i % 2 == 0 {
                    thread::sleep(ms(50));
                }
                loop {
                    if let Ok(x) = r.recv_timeout(ms(10)) {
                        assert_eq!(x, i);
                        break;
                    }
                }
            }
        });
    })
    .unwrap();
}

#[test]
fn drops() {
    #[cfg(miri)]
    const RUNS: usize = 10;
    #[cfg(not(miri))]
    const RUNS: usize = 100;
    #[cfg(miri)]
    const STEPS: usize = 100;
    #[cfg(not(miri))]
    const STEPS: usize = 10_000;

    static DROPS: AtomicUsize = AtomicUsize::new(0);

    #[derive(Debug, PartialEq)]
    struct DropCounter;

    impl Drop for DropCounter {
        fn drop(&mut self) {
            DROPS.fetch_add(1, Ordering::SeqCst);
        }
    }

    let mut rng = thread_rng();

    for _ in 0..RUNS {
        let steps = rng.gen_range(0..STEPS);
        let additional = rng.gen_range(0..50);

        DROPS.store(0, Ordering::SeqCst);
        let (s, r) = bounded::<DropCounter>(50);

        scope(|scope| {
            scope.spawn(|_| {
                for _ in 0..steps {
                    r.recv().unwrap();
                }
            });

            scope.spawn(|_| {
                for _ in 0..steps {
                    s.send(DropCounter).unwrap();
                }
            });
        })
        .unwrap();

        for _ in 0..additional {
            s.send(DropCounter).unwrap();
        }

        assert_eq!(DROPS.load(Ordering::SeqCst), steps);
        drop(s);
        drop(r);
        assert_eq!(DROPS.load(Ordering::SeqCst), steps + additional);
    }
}

#[test]
fn linearizable() {
    #[cfg(miri)]
    const COUNT: usize = 50;
    #[cfg(not(miri))]
    const COUNT: usize = 25_000;
    const THREADS: usize = 4;

    let (s, r) = bounded(THREADS);

    scope(|scope| {
        for _ in 0..THREADS {
            scope.spawn(|_| {
                for _ in 0..COUNT {
                    s.send(0).unwrap();
                    r.try_recv().unwrap();
                }
            });
        }
    })
    .unwrap();
}

#[test]
fn fairness() {
    #[cfg(miri)]
    const COUNT: usize = 100;
    #[cfg(not(miri))]
    const COUNT: usize = 10_000;

    let (s1, r1) = bounded::<()>(COUNT);
    let (s2, r2) = bounded::<()>(COUNT);

    for _ in 0..COUNT {
        s1.send(()).unwrap();
        s2.send(()).unwrap();
    }

    let mut hits = [0usize; 2];
    for _ in 0..COUNT {
        select! {
            recv(r1) -> _  => hits[0] += 1,
            recv(r2) -> _  => hits[1] += 1,
        }
    }
    assert!(hits.iter().all(|x| *x >= COUNT / hits.len() / 2));
}

#[test]
fn fairness_duplicates() {
    #[cfg(miri)]
    const COUNT: usize = 100;
    #[cfg(not(miri))]
    const COUNT: usize = 10_000;

    let (s, r) = bounded::<()>(COUNT);

    for _ in 0..COUNT {
        s.send(()).unwrap();
    }

    let mut hits = [0usize; 5];
    for _ in 0..COUNT {
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

#[test]
fn recv_in_send() {
    let (s, _r) = bounded(1);
    s.send(()).unwrap();

    #[allow(unreachable_code)]
    {
        select! {
            send(s, panic!()) -> _ => panic!(),
            default => {}
        }
    }

    let (s, r) = bounded(2);
    s.send(()).unwrap();

    select! {
        send(s, assert_eq!(r.recv(), Ok(()))) -> _ => {}
    }
}

#[test]
fn channel_through_channel() {
    #[cfg(miri)]
    const COUNT: usize = 100;
    #[cfg(not(miri))]
    const COUNT: usize = 1000;

    type T = Box<dyn Any + Send>;

    let (s, r) = bounded::<T>(1);

    scope(|scope| {
        scope.spawn(move |_| {
            let mut s = s;

            for _ in 0..COUNT {
                let (new_s, new_r) = bounded(1);
                let new_r: T = Box::new(Some(new_r));

                s.send(new_r).unwrap();
                s = new_s;
            }
        });

        scope.spawn(move |_| {
            let mut r = r;

            for _ in 0..COUNT {
                r = r
                    .recv()
                    .unwrap()
                    .downcast_mut::<Option<Receiver<T>>>()
                    .unwrap()
                    .take()
                    .unwrap()
            }
        });
    })
    .unwrap();
}

#[test]
fn panic_on_drop() {
    struct Msg1<'a>(&'a mut bool);
    impl Drop for Msg1<'_> {
        fn drop(&mut self) {
            if *self.0 && !std::thread::panicking() {
                panic!("double drop");
            } else {
                *self.0 = true;
            }
        }
    }

    struct Msg2<'a>(&'a mut bool);
    impl Drop for Msg2<'_> {
        fn drop(&mut self) {
            if *self.0 {
                panic!("double drop");
            } else {
                *self.0 = true;
                panic!("first drop");
            }
        }
    }

    // normal
    let (s, r) = bounded(2);
    let (mut a, mut b) = (false, false);
    s.send(Msg1(&mut a)).unwrap();
    s.send(Msg1(&mut b)).unwrap();
    drop(s);
    drop(r);
    assert!(a);
    assert!(b);

    // panic on drop
    let (s, r) = bounded(2);
    let (mut a, mut b) = (false, false);
    s.send(Msg2(&mut a)).unwrap();
    s.send(Msg2(&mut b)).unwrap();
    drop(s);
    let res = std::panic::catch_unwind(move || {
        drop(r);
    });
    assert_eq!(
        *res.unwrap_err().downcast_ref::<&str>().unwrap(),
        "first drop"
    );
    assert!(a);
    // Elements after the panicked element will leak.
    assert!(!b);
}
