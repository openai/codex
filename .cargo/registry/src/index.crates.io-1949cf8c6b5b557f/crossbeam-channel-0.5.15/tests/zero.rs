//! Tests for the zero channel flavor.

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
    let (s, r) = bounded(0);
    assert_eq!(s.try_send(7), Err(TrySendError::Full(7)));
    assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
}

#[test]
fn capacity() {
    let (s, r) = bounded::<()>(0);
    assert_eq!(s.capacity(), Some(0));
    assert_eq!(r.capacity(), Some(0));
}

#[test]
fn len_empty_full() {
    let (s, r) = bounded(0);

    assert_eq!(s.len(), 0);
    assert!(s.is_empty());
    assert!(s.is_full());
    assert_eq!(r.len(), 0);
    assert!(r.is_empty());
    assert!(r.is_full());

    scope(|scope| {
        scope.spawn(|_| s.send(0).unwrap());
        scope.spawn(|_| r.recv().unwrap());
    })
    .unwrap();

    assert_eq!(s.len(), 0);
    assert!(s.is_empty());
    assert!(s.is_full());
    assert_eq!(r.len(), 0);
    assert!(r.is_empty());
    assert!(r.is_full());
}

#[test]
fn try_recv() {
    let (s, r) = bounded(0);

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
    let (s, r) = bounded(0);

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
    let (s, r) = bounded::<i32>(0);

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
    let (s, r) = bounded(0);

    scope(|scope| {
        scope.spawn(move |_| {
            assert_eq!(s.try_send(7), Err(TrySendError::Full(7)));
            thread::sleep(ms(1500));
            assert_eq!(s.try_send(8), Ok(()));
            thread::sleep(ms(500));
            assert_eq!(s.try_send(9), Err(TrySendError::Disconnected(9)));
        });
        scope.spawn(move |_| {
            thread::sleep(ms(1000));
            assert_eq!(r.recv(), Ok(8));
        });
    })
    .unwrap();
}

#[test]
fn send() {
    let (s, r) = bounded(0);

    scope(|scope| {
        scope.spawn(move |_| {
            s.send(7).unwrap();
            thread::sleep(ms(1000));
            s.send(8).unwrap();
            thread::sleep(ms(1000));
            s.send(9).unwrap();
        });
        scope.spawn(move |_| {
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
    let (s, r) = bounded(0);

    scope(|scope| {
        scope.spawn(move |_| {
            assert_eq!(
                s.send_timeout(7, ms(1000)),
                Err(SendTimeoutError::Timeout(7))
            );
            assert_eq!(s.send_timeout(8, ms(1000)), Ok(()));
            assert_eq!(
                s.send_timeout(9, ms(1000)),
                Err(SendTimeoutError::Disconnected(9))
            );
        });
        scope.spawn(move |_| {
            thread::sleep(ms(1500));
            assert_eq!(r.recv(), Ok(8));
        });
    })
    .unwrap();
}

#[test]
fn len() {
    #[cfg(miri)]
    const COUNT: usize = 50;
    #[cfg(not(miri))]
    const COUNT: usize = 25_000;

    let (s, r) = bounded(0);

    assert_eq!(s.len(), 0);
    assert_eq!(r.len(), 0);

    scope(|scope| {
        scope.spawn(|_| {
            for i in 0..COUNT {
                assert_eq!(r.recv(), Ok(i));
                assert_eq!(r.len(), 0);
            }
        });

        scope.spawn(|_| {
            for i in 0..COUNT {
                s.send(i).unwrap();
                assert_eq!(s.len(), 0);
            }
        });
    })
    .unwrap();

    assert_eq!(s.len(), 0);
    assert_eq!(r.len(), 0);
}

#[test]
fn disconnect_wakes_sender() {
    let (s, r) = bounded(0);

    scope(|scope| {
        scope.spawn(move |_| {
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
    let (s, r) = bounded::<()>(0);

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
    const COUNT: usize = 50;
    #[cfg(not(miri))]
    const COUNT: usize = 100_000;

    let (s, r) = bounded(0);

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

    let (s, r) = bounded::<usize>(0);
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
    const COUNT: usize = 50;
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
    const COUNT: usize = 50;
    #[cfg(not(miri))]
    const COUNT: usize = 1000;

    let (request_s, request_r) = bounded(0);
    let (response_s, response_r) = bounded(0);

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
                let _ = request_s.try_send(());
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

    let (s, r) = bounded(0);

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
    const RUNS: usize = 20;
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

        DROPS.store(0, Ordering::SeqCst);
        let (s, r) = bounded::<DropCounter>(0);

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

        assert_eq!(DROPS.load(Ordering::SeqCst), steps);
        drop(s);
        drop(r);
        assert_eq!(DROPS.load(Ordering::SeqCst), steps);
    }
}

#[test]
fn fairness() {
    #[cfg(miri)]
    const COUNT: usize = 50;
    #[cfg(not(miri))]
    const COUNT: usize = 10_000;

    let (s1, r1) = bounded::<()>(0);
    let (s2, r2) = bounded::<()>(0);

    scope(|scope| {
        scope.spawn(|_| {
            let mut hits = [0usize; 2];
            for _ in 0..COUNT {
                select! {
                    recv(r1) -> _ => hits[0] += 1,
                    recv(r2) -> _ => hits[1] += 1,
                }
            }
            assert!(hits.iter().all(|x| *x >= COUNT / hits.len() / 2));
        });

        let mut hits = [0usize; 2];
        for _ in 0..COUNT {
            select! {
                send(s1, ()) -> _ => hits[0] += 1,
                send(s2, ()) -> _ => hits[1] += 1,
            }
        }
        assert!(hits.iter().all(|x| *x >= COUNT / hits.len() / 2));
    })
    .unwrap();
}

#[test]
fn fairness_duplicates() {
    #[cfg(miri)]
    const COUNT: usize = 100;
    #[cfg(not(miri))]
    const COUNT: usize = 10_000;

    let (s, r) = bounded::<()>(0);

    scope(|scope| {
        scope.spawn(|_| {
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
        });

        let mut hits = [0usize; 5];
        for _ in 0..COUNT {
            select! {
                send(s, ()) -> _ => hits[0] += 1,
                send(s, ()) -> _ => hits[1] += 1,
                send(s, ()) -> _ => hits[2] += 1,
                send(s, ()) -> _ => hits[3] += 1,
                send(s, ()) -> _ => hits[4] += 1,
            }
        }
        assert!(hits.iter().all(|x| *x >= COUNT / hits.len() / 2));
    })
    .unwrap();
}

#[test]
fn recv_in_send() {
    let (s, r) = bounded(0);

    scope(|scope| {
        scope.spawn(|_| {
            thread::sleep(ms(100));
            r.recv()
        });

        scope.spawn(|_| {
            thread::sleep(ms(500));
            s.send(()).unwrap();
        });

        select! {
            send(s, r.recv().unwrap()) -> _ => {}
        }
    })
    .unwrap();
}

#[test]
fn channel_through_channel() {
    #[cfg(miri)]
    const COUNT: usize = 50;
    #[cfg(not(miri))]
    const COUNT: usize = 1000;

    type T = Box<dyn Any + Send>;

    let (s, r) = bounded::<T>(0);

    scope(|scope| {
        scope.spawn(move |_| {
            let mut s = s;

            for _ in 0..COUNT {
                let (new_s, new_r) = bounded(0);
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
