#![feature(test)]

extern crate test;

use crossbeam_channel::{bounded, unbounded};
use crossbeam_utils::thread::scope;
use test::Bencher;

const TOTAL_STEPS: usize = 40_000;

mod unbounded {
    use super::*;

    #[bench]
    fn create(b: &mut Bencher) {
        b.iter(unbounded::<i32>);
    }

    #[bench]
    fn oneshot(b: &mut Bencher) {
        b.iter(|| {
            let (s, r) = unbounded::<i32>();
            s.send(0).unwrap();
            r.recv().unwrap();
        });
    }

    #[bench]
    fn inout(b: &mut Bencher) {
        let (s, r) = unbounded::<i32>();
        b.iter(|| {
            s.send(0).unwrap();
            r.recv().unwrap();
        });
    }

    #[bench]
    fn par_inout(b: &mut Bencher) {
        let threads = num_cpus::get();
        let steps = TOTAL_STEPS / threads;
        let (s, r) = unbounded::<i32>();

        let (s1, r1) = bounded(0);
        let (s2, r2) = bounded(0);
        scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|_| {
                    while r1.recv().is_ok() {
                        for i in 0..steps {
                            s.send(i as i32).unwrap();
                            r.recv().unwrap();
                        }
                        s2.send(()).unwrap();
                    }
                });
            }

            b.iter(|| {
                for _ in 0..threads {
                    s1.send(()).unwrap();
                }
                for _ in 0..threads {
                    r2.recv().unwrap();
                }
            });
            drop(s1);
        })
        .unwrap();
    }

    #[bench]
    fn spsc(b: &mut Bencher) {
        let steps = TOTAL_STEPS;
        let (s, r) = unbounded::<i32>();

        let (s1, r1) = bounded(0);
        let (s2, r2) = bounded(0);
        scope(|scope| {
            scope.spawn(|_| {
                while r1.recv().is_ok() {
                    for i in 0..steps {
                        s.send(i as i32).unwrap();
                    }
                    s2.send(()).unwrap();
                }
            });

            b.iter(|| {
                s1.send(()).unwrap();
                for _ in 0..steps {
                    r.recv().unwrap();
                }
                r2.recv().unwrap();
            });
            drop(s1);
        })
        .unwrap();
    }

    #[bench]
    fn spmc(b: &mut Bencher) {
        let threads = num_cpus::get() - 1;
        let steps = TOTAL_STEPS / threads;
        let (s, r) = unbounded::<i32>();

        let (s1, r1) = bounded(0);
        let (s2, r2) = bounded(0);
        scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|_| {
                    while r1.recv().is_ok() {
                        for _ in 0..steps {
                            r.recv().unwrap();
                        }
                        s2.send(()).unwrap();
                    }
                });
            }

            b.iter(|| {
                for _ in 0..threads {
                    s1.send(()).unwrap();
                }
                for i in 0..steps * threads {
                    s.send(i as i32).unwrap();
                }
                for _ in 0..threads {
                    r2.recv().unwrap();
                }
            });
            drop(s1);
        })
        .unwrap();
    }

    #[bench]
    fn mpsc(b: &mut Bencher) {
        let threads = num_cpus::get() - 1;
        let steps = TOTAL_STEPS / threads;
        let (s, r) = unbounded::<i32>();

        let (s1, r1) = bounded(0);
        let (s2, r2) = bounded(0);
        scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|_| {
                    while r1.recv().is_ok() {
                        for i in 0..steps {
                            s.send(i as i32).unwrap();
                        }
                        s2.send(()).unwrap();
                    }
                });
            }

            b.iter(|| {
                for _ in 0..threads {
                    s1.send(()).unwrap();
                }
                for _ in 0..steps * threads {
                    r.recv().unwrap();
                }
                for _ in 0..threads {
                    r2.recv().unwrap();
                }
            });
            drop(s1);
        })
        .unwrap();
    }

    #[bench]
    fn mpmc(b: &mut Bencher) {
        let threads = num_cpus::get();
        let steps = TOTAL_STEPS / threads;
        let (s, r) = unbounded::<i32>();

        let (s1, r1) = bounded(0);
        let (s2, r2) = bounded(0);
        scope(|scope| {
            for _ in 0..threads / 2 {
                scope.spawn(|_| {
                    while r1.recv().is_ok() {
                        for i in 0..steps {
                            s.send(i as i32).unwrap();
                        }
                        s2.send(()).unwrap();
                    }
                });
            }
            for _ in 0..threads / 2 {
                scope.spawn(|_| {
                    while r1.recv().is_ok() {
                        for _ in 0..steps {
                            r.recv().unwrap();
                        }
                        s2.send(()).unwrap();
                    }
                });
            }

            b.iter(|| {
                for _ in 0..threads {
                    s1.send(()).unwrap();
                }
                for _ in 0..threads {
                    r2.recv().unwrap();
                }
            });
            drop(s1);
        })
        .unwrap();
    }
}

mod bounded_n {
    use super::*;

    #[bench]
    fn spsc(b: &mut Bencher) {
        let steps = TOTAL_STEPS;
        let (s, r) = bounded::<i32>(steps);

        let (s1, r1) = bounded(0);
        let (s2, r2) = bounded(0);
        scope(|scope| {
            scope.spawn(|_| {
                while r1.recv().is_ok() {
                    for i in 0..steps {
                        s.send(i as i32).unwrap();
                    }
                    s2.send(()).unwrap();
                }
            });

            b.iter(|| {
                s1.send(()).unwrap();
                for _ in 0..steps {
                    r.recv().unwrap();
                }
                r2.recv().unwrap();
            });
            drop(s1);
        })
        .unwrap();
    }

    #[bench]
    fn spmc(b: &mut Bencher) {
        let threads = num_cpus::get() - 1;
        let steps = TOTAL_STEPS / threads;
        let (s, r) = bounded::<i32>(steps * threads);

        let (s1, r1) = bounded(0);
        let (s2, r2) = bounded(0);
        scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|_| {
                    while r1.recv().is_ok() {
                        for _ in 0..steps {
                            r.recv().unwrap();
                        }
                        s2.send(()).unwrap();
                    }
                });
            }

            b.iter(|| {
                for _ in 0..threads {
                    s1.send(()).unwrap();
                }
                for i in 0..steps * threads {
                    s.send(i as i32).unwrap();
                }
                for _ in 0..threads {
                    r2.recv().unwrap();
                }
            });
            drop(s1);
        })
        .unwrap();
    }

    #[bench]
    fn mpsc(b: &mut Bencher) {
        let threads = num_cpus::get() - 1;
        let steps = TOTAL_STEPS / threads;
        let (s, r) = bounded::<i32>(steps * threads);

        let (s1, r1) = bounded(0);
        let (s2, r2) = bounded(0);
        scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|_| {
                    while r1.recv().is_ok() {
                        for i in 0..steps {
                            s.send(i as i32).unwrap();
                        }
                        s2.send(()).unwrap();
                    }
                });
            }

            b.iter(|| {
                for _ in 0..threads {
                    s1.send(()).unwrap();
                }
                for _ in 0..steps * threads {
                    r.recv().unwrap();
                }
                for _ in 0..threads {
                    r2.recv().unwrap();
                }
            });
            drop(s1);
        })
        .unwrap();
    }

    #[bench]
    fn par_inout(b: &mut Bencher) {
        let threads = num_cpus::get();
        let steps = TOTAL_STEPS / threads;
        let (s, r) = bounded::<i32>(threads);

        let (s1, r1) = bounded(0);
        let (s2, r2) = bounded(0);
        scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|_| {
                    while r1.recv().is_ok() {
                        for i in 0..steps {
                            s.send(i as i32).unwrap();
                            r.recv().unwrap();
                        }
                        s2.send(()).unwrap();
                    }
                });
            }

            b.iter(|| {
                for _ in 0..threads {
                    s1.send(()).unwrap();
                }
                for _ in 0..threads {
                    r2.recv().unwrap();
                }
            });
            drop(s1);
        })
        .unwrap();
    }

    #[bench]
    fn mpmc(b: &mut Bencher) {
        let threads = num_cpus::get();
        assert_eq!(threads % 2, 0);
        let steps = TOTAL_STEPS / threads;
        let (s, r) = bounded::<i32>(steps * threads);

        let (s1, r1) = bounded(0);
        let (s2, r2) = bounded(0);
        scope(|scope| {
            for _ in 0..threads / 2 {
                scope.spawn(|_| {
                    while r1.recv().is_ok() {
                        for i in 0..steps {
                            s.send(i as i32).unwrap();
                        }
                        s2.send(()).unwrap();
                    }
                });
            }
            for _ in 0..threads / 2 {
                scope.spawn(|_| {
                    while r1.recv().is_ok() {
                        for _ in 0..steps {
                            r.recv().unwrap();
                        }
                        s2.send(()).unwrap();
                    }
                });
            }

            b.iter(|| {
                for _ in 0..threads {
                    s1.send(()).unwrap();
                }
                for _ in 0..threads {
                    r2.recv().unwrap();
                }
            });
            drop(s1);
        })
        .unwrap();
    }
}

mod bounded_1 {
    use super::*;

    #[bench]
    fn create(b: &mut Bencher) {
        b.iter(|| bounded::<i32>(1));
    }

    #[bench]
    fn oneshot(b: &mut Bencher) {
        b.iter(|| {
            let (s, r) = bounded::<i32>(1);
            s.send(0).unwrap();
            r.recv().unwrap();
        });
    }

    #[bench]
    fn spsc(b: &mut Bencher) {
        let steps = TOTAL_STEPS;
        let (s, r) = bounded::<i32>(1);

        let (s1, r1) = bounded(0);
        let (s2, r2) = bounded(0);
        scope(|scope| {
            scope.spawn(|_| {
                while r1.recv().is_ok() {
                    for i in 0..steps {
                        s.send(i as i32).unwrap();
                    }
                    s2.send(()).unwrap();
                }
            });

            b.iter(|| {
                s1.send(()).unwrap();
                for _ in 0..steps {
                    r.recv().unwrap();
                }
                r2.recv().unwrap();
            });
            drop(s1);
        })
        .unwrap();
    }

    #[bench]
    fn spmc(b: &mut Bencher) {
        let threads = num_cpus::get() - 1;
        let steps = TOTAL_STEPS / threads;
        let (s, r) = bounded::<i32>(1);

        let (s1, r1) = bounded(0);
        let (s2, r2) = bounded(0);
        scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|_| {
                    while r1.recv().is_ok() {
                        for _ in 0..steps {
                            r.recv().unwrap();
                        }
                        s2.send(()).unwrap();
                    }
                });
            }

            b.iter(|| {
                for _ in 0..threads {
                    s1.send(()).unwrap();
                }
                for i in 0..steps * threads {
                    s.send(i as i32).unwrap();
                }
                for _ in 0..threads {
                    r2.recv().unwrap();
                }
            });
            drop(s1);
        })
        .unwrap();
    }

    #[bench]
    fn mpsc(b: &mut Bencher) {
        let threads = num_cpus::get() - 1;
        let steps = TOTAL_STEPS / threads;
        let (s, r) = bounded::<i32>(1);

        let (s1, r1) = bounded(0);
        let (s2, r2) = bounded(0);
        scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|_| {
                    while r1.recv().is_ok() {
                        for i in 0..steps {
                            s.send(i as i32).unwrap();
                        }
                        s2.send(()).unwrap();
                    }
                });
            }

            b.iter(|| {
                for _ in 0..threads {
                    s1.send(()).unwrap();
                }
                for _ in 0..steps * threads {
                    r.recv().unwrap();
                }
                for _ in 0..threads {
                    r2.recv().unwrap();
                }
            });
            drop(s1);
        })
        .unwrap();
    }

    #[bench]
    fn mpmc(b: &mut Bencher) {
        let threads = num_cpus::get();
        let steps = TOTAL_STEPS / threads;
        let (s, r) = bounded::<i32>(1);

        let (s1, r1) = bounded(0);
        let (s2, r2) = bounded(0);
        scope(|scope| {
            for _ in 0..threads / 2 {
                scope.spawn(|_| {
                    while r1.recv().is_ok() {
                        for i in 0..steps {
                            s.send(i as i32).unwrap();
                        }
                        s2.send(()).unwrap();
                    }
                });
            }
            for _ in 0..threads / 2 {
                scope.spawn(|_| {
                    while r1.recv().is_ok() {
                        for _ in 0..steps {
                            r.recv().unwrap();
                        }
                        s2.send(()).unwrap();
                    }
                });
            }

            b.iter(|| {
                for _ in 0..threads {
                    s1.send(()).unwrap();
                }
                for _ in 0..threads {
                    r2.recv().unwrap();
                }
            });
            drop(s1);
        })
        .unwrap();
    }
}

mod bounded_0 {
    use super::*;

    #[bench]
    fn create(b: &mut Bencher) {
        b.iter(|| bounded::<i32>(0));
    }

    #[bench]
    fn spsc(b: &mut Bencher) {
        let steps = TOTAL_STEPS;
        let (s, r) = bounded::<i32>(0);

        let (s1, r1) = bounded(0);
        let (s2, r2) = bounded(0);
        scope(|scope| {
            scope.spawn(|_| {
                while r1.recv().is_ok() {
                    for i in 0..steps {
                        s.send(i as i32).unwrap();
                    }
                    s2.send(()).unwrap();
                }
            });

            b.iter(|| {
                s1.send(()).unwrap();
                for _ in 0..steps {
                    r.recv().unwrap();
                }
                r2.recv().unwrap();
            });
            drop(s1);
        })
        .unwrap();
    }

    #[bench]
    fn spmc(b: &mut Bencher) {
        let threads = num_cpus::get() - 1;
        let steps = TOTAL_STEPS / threads;
        let (s, r) = bounded::<i32>(0);

        let (s1, r1) = bounded(0);
        let (s2, r2) = bounded(0);
        scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|_| {
                    while r1.recv().is_ok() {
                        for _ in 0..steps {
                            r.recv().unwrap();
                        }
                        s2.send(()).unwrap();
                    }
                });
            }

            b.iter(|| {
                for _ in 0..threads {
                    s1.send(()).unwrap();
                }
                for i in 0..steps * threads {
                    s.send(i as i32).unwrap();
                }
                for _ in 0..threads {
                    r2.recv().unwrap();
                }
            });
            drop(s1);
        })
        .unwrap();
    }

    #[bench]
    fn mpsc(b: &mut Bencher) {
        let threads = num_cpus::get() - 1;
        let steps = TOTAL_STEPS / threads;
        let (s, r) = bounded::<i32>(0);

        let (s1, r1) = bounded(0);
        let (s2, r2) = bounded(0);
        scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|_| {
                    while r1.recv().is_ok() {
                        for i in 0..steps {
                            s.send(i as i32).unwrap();
                        }
                        s2.send(()).unwrap();
                    }
                });
            }

            b.iter(|| {
                for _ in 0..threads {
                    s1.send(()).unwrap();
                }
                for _ in 0..steps * threads {
                    r.recv().unwrap();
                }
                for _ in 0..threads {
                    r2.recv().unwrap();
                }
            });
            drop(s1);
        })
        .unwrap();
    }

    #[bench]
    fn mpmc(b: &mut Bencher) {
        let threads = num_cpus::get();
        let steps = TOTAL_STEPS / threads;
        let (s, r) = bounded::<i32>(0);

        let (s1, r1) = bounded(0);
        let (s2, r2) = bounded(0);
        scope(|scope| {
            for _ in 0..threads / 2 {
                scope.spawn(|_| {
                    while r1.recv().is_ok() {
                        for i in 0..steps {
                            s.send(i as i32).unwrap();
                        }
                        s2.send(()).unwrap();
                    }
                });
            }
            for _ in 0..threads / 2 {
                scope.spawn(|_| {
                    while r1.recv().is_ok() {
                        for _ in 0..steps {
                            r.recv().unwrap();
                        }
                        s2.send(()).unwrap();
                    }
                });
            }

            b.iter(|| {
                for _ in 0..threads {
                    s1.send(()).unwrap();
                }
                for _ in 0..threads {
                    r2.recv().unwrap();
                }
            });
            drop(s1);
        })
        .unwrap();
    }
}
