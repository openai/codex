//! Tests for channel selection using the `Select` struct.

use std::any::Any;
use std::cell::Cell;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{after, bounded, tick, unbounded, Receiver, Select, TryRecvError};
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
    let oper1 = sel.recv(&r1);
    let oper2 = sel.recv(&r2);
    let oper = sel.select();
    match oper.index() {
        i if i == oper1 => assert_eq!(oper.recv(&r1), Ok(1)),
        i if i == oper2 => panic!(),
        _ => unreachable!(),
    }

    s2.send(2).unwrap();

    let mut sel = Select::new();
    let oper1 = sel.recv(&r1);
    let oper2 = sel.recv(&r2);
    let oper = sel.select();
    match oper.index() {
        i if i == oper1 => panic!(),
        i if i == oper2 => assert_eq!(oper.recv(&r2), Ok(2)),
        _ => unreachable!(),
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

    let mut sel = Select::new();
    let oper1 = sel.recv(&r1);
    let oper2 = sel.recv(&r2);
    let oper3 = sel.recv(&r3);
    let oper4 = sel.recv(&r4);
    let oper5 = sel.recv(&r5);
    let oper = sel.select();
    match oper.index() {
        i if i == oper1 => panic!(),
        i if i == oper2 => panic!(),
        i if i == oper3 => panic!(),
        i if i == oper4 => panic!(),
        i if i == oper5 => assert_eq!(oper.recv(&r5), Ok(5)),
        _ => unreachable!(),
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

        let mut sel = Select::new();
        let oper1 = sel.recv(&r1);
        let oper2 = sel.recv(&r2);
        let oper = sel.select_timeout(ms(1000));
        match oper {
            Err(_) => panic!(),
            Ok(oper) => match oper.index() {
                i if i == oper1 => assert!(oper.recv(&r1).is_err()),
                i if i == oper2 => panic!(),
                _ => unreachable!(),
            },
        }

        r2.recv().unwrap();
    })
    .unwrap();

    let mut sel = Select::new();
    let oper1 = sel.recv(&r1);
    let oper2 = sel.recv(&r2);
    let oper = sel.select_timeout(ms(1000));
    match oper {
        Err(_) => panic!(),
        Ok(oper) => match oper.index() {
            i if i == oper1 => assert!(oper.recv(&r1).is_err()),
            i if i == oper2 => panic!(),
            _ => unreachable!(),
        },
    }

    scope(|scope| {
        scope.spawn(|_| {
            thread::sleep(ms(500));
            drop(s2);
        });

        let mut sel = Select::new();
        let oper1 = sel.recv(&r2);
        let oper = sel.select_timeout(ms(1000));
        match oper {
            Err(_) => panic!(),
            Ok(oper) => match oper.index() {
                i if i == oper1 => assert!(oper.recv(&r2).is_err()),
                _ => unreachable!(),
            },
        }
    })
    .unwrap();
}

#[test]
fn default() {
    let (s1, r1) = unbounded::<i32>();
    let (s2, r2) = unbounded::<i32>();

    let mut sel = Select::new();
    let _oper1 = sel.recv(&r1);
    let _oper2 = sel.recv(&r2);
    let oper = sel.try_select();
    match oper {
        Err(_) => {}
        Ok(_) => panic!(),
    }

    drop(s1);

    let mut sel = Select::new();
    let oper1 = sel.recv(&r1);
    let oper2 = sel.recv(&r2);
    let oper = sel.try_select();
    match oper {
        Err(_) => panic!(),
        Ok(oper) => match oper.index() {
            i if i == oper1 => assert!(oper.recv(&r1).is_err()),
            i if i == oper2 => panic!(),
            _ => unreachable!(),
        },
    }

    s2.send(2).unwrap();

    let mut sel = Select::new();
    let oper1 = sel.recv(&r2);
    let oper = sel.try_select();
    match oper {
        Err(_) => panic!(),
        Ok(oper) => match oper.index() {
            i if i == oper1 => assert_eq!(oper.recv(&r2), Ok(2)),
            _ => unreachable!(),
        },
    }

    let mut sel = Select::new();
    let _oper1 = sel.recv(&r2);
    let oper = sel.try_select();
    match oper {
        Err(_) => {}
        Ok(_) => panic!(),
    }

    let mut sel = Select::new();
    let oper = sel.try_select();
    match oper {
        Err(_) => {}
        Ok(_) => panic!(),
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

        let mut sel = Select::new();
        let oper1 = sel.recv(&r1);
        let oper2 = sel.recv(&r2);
        let oper = sel.select_timeout(ms(1000));
        match oper {
            Err(_) => {}
            Ok(oper) => match oper.index() {
                i if i == oper1 => panic!(),
                i if i == oper2 => panic!(),
                _ => unreachable!(),
            },
        }

        let mut sel = Select::new();
        let oper1 = sel.recv(&r1);
        let oper2 = sel.recv(&r2);
        let oper = sel.select_timeout(ms(1000));
        match oper {
            Err(_) => panic!(),
            Ok(oper) => match oper.index() {
                i if i == oper1 => panic!(),
                i if i == oper2 => assert_eq!(oper.recv(&r2), Ok(2)),
                _ => unreachable!(),
            },
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
        let oper = sel.select_timeout(ms(1000));
        match oper {
            Err(_) => {
                let mut sel = Select::new();
                let oper1 = sel.recv(&r);
                let oper = sel.try_select();
                match oper {
                    Err(_) => panic!(),
                    Ok(oper) => match oper.index() {
                        i if i == oper1 => assert!(oper.recv(&r).is_err()),
                        _ => unreachable!(),
                    },
                }
            }
            Ok(_) => unreachable!(),
        }
    })
    .unwrap();
}

#[test]
fn default_when_disconnected() {
    let (_, r) = unbounded::<i32>();

    let mut sel = Select::new();
    let oper1 = sel.recv(&r);
    let oper = sel.try_select();
    match oper {
        Err(_) => panic!(),
        Ok(oper) => match oper.index() {
            i if i == oper1 => assert!(oper.recv(&r).is_err()),
            _ => unreachable!(),
        },
    }

    let (_, r) = unbounded::<i32>();

    let mut sel = Select::new();
    let oper1 = sel.recv(&r);
    let oper = sel.select_timeout(ms(1000));
    match oper {
        Err(_) => panic!(),
        Ok(oper) => match oper.index() {
            i if i == oper1 => assert!(oper.recv(&r).is_err()),
            _ => unreachable!(),
        },
    }

    let (s, _) = bounded::<i32>(0);

    let mut sel = Select::new();
    let oper1 = sel.send(&s);
    let oper = sel.try_select();
    match oper {
        Err(_) => panic!(),
        Ok(oper) => match oper.index() {
            i if i == oper1 => assert!(oper.send(&s, 0).is_err()),
            _ => unreachable!(),
        },
    }

    let (s, _) = bounded::<i32>(0);

    let mut sel = Select::new();
    let oper1 = sel.send(&s);
    let oper = sel.select_timeout(ms(1000));
    match oper {
        Err(_) => panic!(),
        Ok(oper) => match oper.index() {
            i if i == oper1 => assert!(oper.send(&s, 0).is_err()),
            _ => unreachable!(),
        },
    }
}

#[test]
fn default_only() {
    let start = Instant::now();

    let mut sel = Select::new();
    let oper = sel.try_select();
    assert!(oper.is_err());
    let now = Instant::now();
    assert!(now - start <= ms(50));

    let start = Instant::now();
    let mut sel = Select::new();
    let oper = sel.select_timeout(ms(500));
    assert!(oper.is_err());
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
        let oper1 = sel.recv(&r1);
        let oper2 = sel.recv(&r2);
        let oper = sel.select_timeout(ms(1000));
        match oper {
            Err(_) => panic!(),
            Ok(oper) => match oper.index() {
                i if i == oper1 => panic!(),
                i if i == oper2 => assert_eq!(oper.recv(&r2), Ok(2)),
                _ => unreachable!(),
            },
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
            let oper1 = sel.recv(&r1);
            let oper2 = sel.send(&s2);
            let oper = sel.select();
            match oper.index() {
                i if i == oper1 => assert_eq!(oper.recv(&r1), Ok(1)),
                i if i == oper2 => oper.send(&s2, 2).unwrap(),
                _ => unreachable!(),
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
                let mut done = false;

                let mut sel = Select::new();
                let oper1 = sel.send(&s1);
                let oper = sel.try_select();
                match oper {
                    Err(_) => {}
                    Ok(oper) => match oper.index() {
                        i if i == oper1 => {
                            let _ = oper.send(&s1, 1);
                            done = true;
                        }
                        _ => unreachable!(),
                    },
                }
                if done {
                    break;
                }

                let mut sel = Select::new();
                let oper1 = sel.recv(&r_end);
                let oper = sel.try_select();
                match oper {
                    Err(_) => {}
                    Ok(oper) => match oper.index() {
                        i if i == oper1 => {
                            let _ = oper.recv(&r_end);
                            done = true;
                        }
                        _ => unreachable!(),
                    },
                }
                if done {
                    break;
                }
            });

            scope.spawn(|_| loop {
                if let Ok(x) = r2.try_recv() {
                    assert_eq!(x, 2);
                    break;
                }

                let mut done = false;
                let mut sel = Select::new();
                let oper1 = sel.recv(&r_end);
                let oper = sel.try_select();
                match oper {
                    Err(_) => {}
                    Ok(oper) => match oper.index() {
                        i if i == oper1 => {
                            let _ = oper.recv(&r_end);
                            done = true;
                        }
                        _ => unreachable!(),
                    },
                }
                if done {
                    break;
                }
            });

            scope.spawn(|_| {
                thread::sleep(ms(500));

                let mut sel = Select::new();
                let oper1 = sel.recv(&r1);
                let oper2 = sel.send(&s2);
                let oper = sel.select_timeout(ms(1000));
                match oper {
                    Err(_) => {}
                    Ok(oper) => match oper.index() {
                        i if i == oper1 => assert_eq!(oper.recv(&r1), Ok(1)),
                        i if i == oper2 => assert!(oper.send(&s2, 2).is_ok()),
                        _ => unreachable!(),
                    },
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
            assert!(r3.try_recv().is_err());
            s1.send(1).unwrap();
            r3.recv().unwrap();
        });

        s3.send(()).unwrap();

        let mut sel = Select::new();
        let oper1 = sel.recv(&r1);
        let oper2 = sel.recv(&r2);
        let oper = sel.select();
        match oper.index() {
            i if i == oper1 => drop(oper.recv(&r1)),
            i if i == oper2 => drop(oper.recv(&r2)),
            _ => unreachable!(),
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
            let oper1 = sel.recv(&r1);
            let oper2 = sel.recv(&r2);
            let oper = sel.select();
            match oper.index() {
                i if i == oper1 => panic!(),
                i if i == oper2 => drop(oper.recv(&r2)),
                _ => unreachable!(),
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
    let oper1 = sel.recv(&r);
    let oper = sel.select();
    match oper.index() {
        i if i == oper1 => drop(oper.recv(&r)),
        _ => unreachable!(),
    }
}

#[test]
fn preflight2() {
    let (s, r) = unbounded();
    drop(s.clone());
    s.send(()).unwrap();
    drop(s);

    let mut sel = Select::new();
    let oper1 = sel.recv(&r);
    let oper = sel.select();
    match oper.index() {
        i if i == oper1 => assert_eq!(oper.recv(&r), Ok(())),
        _ => unreachable!(),
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
    let oper1 = sel.recv(&r);
    let oper = sel.select();
    match oper.index() {
        i if i == oper1 => assert!(oper.recv(&r).is_err()),
        _ => unreachable!(),
    }
}

#[test]
fn duplicate_operations() {
    let (s, r) = unbounded::<i32>();
    let hit = vec![Cell::new(false); 4];

    while hit.iter().map(|h| h.get()).any(|hit| !hit) {
        let mut sel = Select::new();
        let oper0 = sel.recv(&r);
        let oper1 = sel.recv(&r);
        let oper2 = sel.send(&s);
        let oper3 = sel.send(&s);
        let oper = sel.select();
        match oper.index() {
            i if i == oper0 => {
                assert!(oper.recv(&r).is_ok());
                hit[0].set(true);
            }
            i if i == oper1 => {
                assert!(oper.recv(&r).is_ok());
                hit[1].set(true);
            }
            i if i == oper2 => {
                assert!(oper.send(&s, 0).is_ok());
                hit[2].set(true);
            }
            i if i == oper3 => {
                assert!(oper.send(&s, 0).is_ok());
                hit[3].set(true);
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn nesting() {
    let (s, r) = unbounded::<i32>();

    let mut sel = Select::new();
    let oper1 = sel.send(&s);
    let oper = sel.select();
    match oper.index() {
        i if i == oper1 => {
            assert!(oper.send(&s, 0).is_ok());

            let mut sel = Select::new();
            let oper1 = sel.recv(&r);
            let oper = sel.select();
            match oper.index() {
                i if i == oper1 => {
                    assert_eq!(oper.recv(&r), Ok(0));

                    let mut sel = Select::new();
                    let oper1 = sel.send(&s);
                    let oper = sel.select();
                    match oper.index() {
                        i if i == oper1 => {
                            assert!(oper.send(&s, 1).is_ok());

                            let mut sel = Select::new();
                            let oper1 = sel.recv(&r);
                            let oper = sel.select();
                            match oper.index() {
                                i if i == oper1 => {
                                    assert_eq!(oper.recv(&r), Ok(1));
                                }
                                _ => unreachable!(),
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                _ => unreachable!(),
            }
        }
        _ => unreachable!(),
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
                let mut sel = Select::new();
                let oper1 = sel.recv(&r1);
                let oper2 = sel.recv(&r2);
                let oper = sel.select();
                match oper.index() {
                    ix if ix == oper1 => assert_eq!(oper.recv(&r1), Ok(i)),
                    ix if ix == oper2 => assert_eq!(oper.recv(&r2), Ok(i)),
                    _ => unreachable!(),
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
    const COUNT: usize = 50;
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
                let oper1 = sel.send(&s1);
                let oper2 = sel.send(&s2);
                let oper = sel.select();
                match oper.index() {
                    ix if ix == oper1 => assert!(oper.send(&s1, i).is_ok()),
                    ix if ix == oper2 => assert!(oper.send(&s2, i).is_ok()),
                    _ => unreachable!(),
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
                let oper1 = sel.recv(&r1);
                let oper2 = sel.send(&s2);
                let oper = sel.select();
                match oper.index() {
                    ix if ix == oper1 => assert_eq!(oper.recv(&r1), Ok(i)),
                    ix if ix == oper2 => assert!(oper.send(&s2, i).is_ok()),
                    _ => unreachable!(),
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
                    let oper1 = sel.send(&s);
                    let oper = sel.select_timeout(ms(100));
                    match oper {
                        Err(_) => {}
                        Ok(oper) => match oper.index() {
                            ix if ix == oper1 => {
                                assert!(oper.send(&s, i).is_ok());
                                break;
                            }
                            _ => unreachable!(),
                        },
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
                    let oper1 = sel.recv(&r);
                    let oper = sel.select_timeout(ms(100));
                    match oper {
                        Err(_) => {}
                        Ok(oper) => match oper.index() {
                            ix if ix == oper1 => {
                                assert_eq!(oper.recv(&r), Ok(i));
                                break;
                            }
                            _ => unreachable!(),
                        },
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
    let oper1 = sel.send(&s);
    let oper2 = sel.recv(&r);
    let oper = sel.select_timeout(ms(100));
    match oper {
        Err(_) => {}
        Ok(oper) => match oper.index() {
            ix if ix == oper1 => panic!(),
            ix if ix == oper2 => panic!(),
            _ => unreachable!(),
        },
    }

    let (s, r) = unbounded::<i32>();
    let mut sel = Select::new();
    let oper1 = sel.send(&s);
    let oper2 = sel.recv(&r);
    let oper = sel.select_timeout(ms(100));
    match oper {
        Err(_) => panic!(),
        Ok(oper) => match oper.index() {
            ix if ix == oper1 => assert!(oper.send(&s, 0).is_ok()),
            ix if ix == oper2 => panic!(),
            _ => unreachable!(),
        },
    }
}

#[test]
fn matching() {
    const THREADS: usize = 44;

    let (s, r) = &bounded::<usize>(0);

    scope(|scope| {
        for i in 0..THREADS {
            scope.spawn(move |_| {
                let mut sel = Select::new();
                let oper1 = sel.recv(r);
                let oper2 = sel.send(s);
                let oper = sel.select();
                match oper.index() {
                    ix if ix == oper1 => assert_ne!(oper.recv(r), Ok(i)),
                    ix if ix == oper2 => assert!(oper.send(s, i).is_ok()),
                    _ => unreachable!(),
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
                let mut sel = Select::new();
                let oper1 = sel.recv(r);
                let oper2 = sel.send(s);
                let oper = sel.select();
                match oper.index() {
                    ix if ix == oper1 => assert_ne!(oper.recv(r), Ok(i)),
                    ix if ix == oper2 => assert!(oper.send(s, i).is_ok()),
                    _ => unreachable!(),
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
    const COUNT: usize = 50;
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

                    {
                        let mut sel = Select::new();
                        let oper1 = sel.send(&s);
                        let oper = sel.select();
                        match oper.index() {
                            ix if ix == oper1 => assert!(oper.send(&s, new_r).is_ok()),
                            _ => unreachable!(),
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
                        let oper1 = sel.recv(&r);
                        let oper = sel.select();
                        match oper.index() {
                            ix if ix == oper1 => oper
                                .recv(&r)
                                .unwrap()
                                .downcast_mut::<Option<Receiver<T>>>()
                                .unwrap()
                                .take()
                                .unwrap(),
                            _ => unreachable!(),
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
fn linearizable_try() {
    #[cfg(miri)]
    const COUNT: usize = 50;
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

                    let mut sel = Select::new();
                    let oper1 = sel.recv(&r1);
                    let oper2 = sel.recv(&r2);
                    let oper = sel.try_select();
                    match oper {
                        Err(_) => unreachable!(),
                        Ok(oper) => match oper.index() {
                            ix if ix == oper1 => assert!(oper.recv(&r1).is_ok()),
                            ix if ix == oper2 => assert!(oper.recv(&r2).is_ok()),
                            _ => unreachable!(),
                        },
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
    const COUNT: usize = 50;
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

                    let mut sel = Select::new();
                    let oper1 = sel.recv(&r1);
                    let oper2 = sel.recv(&r2);
                    let oper = sel.select_timeout(ms(0));
                    match oper {
                        Err(_) => unreachable!(),
                        Ok(oper) => match oper.index() {
                            ix if ix == oper1 => assert!(oper.recv(&r1).is_ok()),
                            ix if ix == oper2 => assert!(oper.recv(&r2).is_ok()),
                            _ => unreachable!(),
                        },
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
    const COUNT: usize = 50;
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
        let oper1 = sel.recv(&r1);
        let oper2 = sel.recv(&r2);
        let oper3 = sel.recv(&after);
        let oper4 = sel.recv(&tick);
        let oper = sel.select();
        match oper.index() {
            i if i == oper1 => {
                oper.recv(&r1).unwrap();
                hits[0].set(hits[0].get() + 1);
            }
            i if i == oper2 => {
                oper.recv(&r2).unwrap();
                hits[1].set(hits[1].get() + 1);
            }
            i if i == oper3 => {
                oper.recv(&after).unwrap();
                hits[2].set(hits[2].get() + 1);
            }
            i if i == oper4 => {
                oper.recv(&tick).unwrap();
                hits[3].set(hits[3].get() + 1);
            }
            _ => unreachable!(),
        }
    }
    assert!(hits.iter().all(|x| x.get() >= COUNT / hits.len() / 2));
}

#[test]
fn fairness2() {
    #[cfg(miri)]
    const COUNT: usize = 50;
    #[cfg(not(miri))]
    const COUNT: usize = 10_000;

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
            let oper1 = sel.recv(&r1);
            let oper2 = sel.recv(&r2);
            let oper3 = sel.recv(&r3);
            let oper = sel.select();
            match oper.index() {
                i if i == oper1 => {
                    oper.recv(&r1).unwrap();
                    hits[0].set(hits[0].get() + 1);
                }
                i if i == oper2 => {
                    oper.recv(&r2).unwrap();
                    hits[1].set(hits[1].get() + 1);
                }
                i if i == oper3 => {
                    oper.recv(&r3).unwrap();
                    hits[2].set(hits[2].get() + 1);
                }
                _ => unreachable!(),
            }
        }
        assert!(hits.iter().all(|x| x.get() >= COUNT / hits.len() / 50));
    })
    .unwrap();
}

#[test]
fn sync_and_clone() {
    const THREADS: usize = 20;

    let (s, r) = &bounded::<usize>(0);

    let mut sel = Select::new();
    let oper1 = sel.recv(r);
    let oper2 = sel.send(s);
    let sel = &sel;

    scope(|scope| {
        for i in 0..THREADS {
            scope.spawn(move |_| {
                let mut sel = sel.clone();
                let oper = sel.select();
                match oper.index() {
                    ix if ix == oper1 => assert_ne!(oper.recv(r), Ok(i)),
                    ix if ix == oper2 => assert!(oper.send(s, i).is_ok()),
                    _ => unreachable!(),
                }
            });
        }
    })
    .unwrap();

    assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
}

#[test]
fn send_and_clone() {
    const THREADS: usize = 20;

    let (s, r) = &bounded::<usize>(0);

    let mut sel = Select::new();
    let oper1 = sel.recv(r);
    let oper2 = sel.send(s);

    scope(|scope| {
        for i in 0..THREADS {
            let mut sel = sel.clone();
            scope.spawn(move |_| {
                let oper = sel.select();
                match oper.index() {
                    ix if ix == oper1 => assert_ne!(oper.recv(r), Ok(i)),
                    ix if ix == oper2 => assert!(oper.send(s, i).is_ok()),
                    _ => unreachable!(),
                }
            });
        }
    })
    .unwrap();

    assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
}

#[test]
fn reuse() {
    #[cfg(miri)]
    const COUNT: usize = 50;
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

        let mut sel = Select::new();
        let oper1 = sel.recv(&r1);
        let oper2 = sel.send(&s2);

        for i in 0..COUNT {
            for _ in 0..2 {
                let oper = sel.select();
                match oper.index() {
                    ix if ix == oper1 => assert_eq!(oper.recv(&r1), Ok(i)),
                    ix if ix == oper2 => assert!(oper.send(&s2, i).is_ok()),
                    _ => unreachable!(),
                }
            }
            s3.send(()).unwrap();
        }
    })
    .unwrap();
}
