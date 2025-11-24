use crate::runtime::scheduler::multi_thread::{queue, Stats};
use crate::runtime::tests::{unowned, NoopSchedule};

use loom::thread;
use std::cell::RefCell;

fn new_stats() -> Stats {
    Stats::new(&crate::runtime::WorkerMetrics::new())
}

#[test]
fn basic() {
    loom::model(|| {
        let (steal, mut local) = queue::local();
        let inject = RefCell::new(vec![]);
        let mut stats = new_stats();

        let th = thread::spawn(move || {
            let mut stats = new_stats();
            let (_, mut local) = queue::local();
            let mut n = 0;

            for _ in 0..3 {
                if steal.steal_into(&mut local, &mut stats).is_some() {
                    n += 1;
                }

                while local.pop().is_some() {
                    n += 1;
                }
            }

            n
        });

        let mut n = 0;

        for _ in 0..2 {
            for _ in 0..2 {
                let (task, _) = unowned(async {});
                local.push_back_or_overflow(task, &inject, &mut stats);
            }

            if local.pop().is_some() {
                n += 1;
            }

            // Push another task
            let (task, _) = unowned(async {});
            local.push_back_or_overflow(task, &inject, &mut stats);

            while local.pop().is_some() {
                n += 1;
            }
        }

        n += inject.borrow_mut().drain(..).count();

        n += th.join().unwrap();

        assert_eq!(6, n);
    });
}

#[test]
fn steal_overflow() {
    loom::model(|| {
        let (steal, mut local) = queue::local();
        let inject = RefCell::new(vec![]);
        let mut stats = new_stats();

        let th = thread::spawn(move || {
            let mut stats = new_stats();
            let (_, mut local) = queue::local();
            let mut n = 0;

            if steal.steal_into(&mut local, &mut stats).is_some() {
                n += 1;
            }

            while local.pop().is_some() {
                n += 1;
            }

            n
        });

        let mut n = 0;

        // push a task, pop a task
        let (task, _) = unowned(async {});
        local.push_back_or_overflow(task, &inject, &mut stats);

        if local.pop().is_some() {
            n += 1;
        }

        for _ in 0..6 {
            let (task, _) = unowned(async {});
            local.push_back_or_overflow(task, &inject, &mut stats);
        }

        n += th.join().unwrap();

        while local.pop().is_some() {
            n += 1;
        }

        n += inject.borrow_mut().drain(..).count();

        assert_eq!(7, n);
    });
}

#[test]
fn multi_stealer() {
    const NUM_TASKS: usize = 5;

    fn steal_tasks(steal: queue::Steal<NoopSchedule>) -> usize {
        let mut stats = new_stats();
        let (_, mut local) = queue::local();

        if steal.steal_into(&mut local, &mut stats).is_none() {
            return 0;
        }

        let mut n = 1;

        while local.pop().is_some() {
            n += 1;
        }

        n
    }

    loom::model(|| {
        let (steal, mut local) = queue::local();
        let inject = RefCell::new(vec![]);
        let mut stats = new_stats();

        // Push work
        for _ in 0..NUM_TASKS {
            let (task, _) = unowned(async {});
            local.push_back_or_overflow(task, &inject, &mut stats);
        }

        let th1 = {
            let steal = steal.clone();
            thread::spawn(move || steal_tasks(steal))
        };

        let th2 = thread::spawn(move || steal_tasks(steal));

        let mut n = 0;

        while local.pop().is_some() {
            n += 1;
        }

        n += inject.borrow_mut().drain(..).count();

        n += th1.join().unwrap();
        n += th2.join().unwrap();

        assert_eq!(n, NUM_TASKS);
    });
}

#[test]
fn chained_steal() {
    loom::model(|| {
        let mut stats = new_stats();
        let (s1, mut l1) = queue::local();
        let (s2, mut l2) = queue::local();
        let inject = RefCell::new(vec![]);

        // Load up some tasks
        for _ in 0..4 {
            let (task, _) = unowned(async {});
            l1.push_back_or_overflow(task, &inject, &mut stats);

            let (task, _) = unowned(async {});
            l2.push_back_or_overflow(task, &inject, &mut stats);
        }

        // Spawn a task to steal from **our** queue
        let th = thread::spawn(move || {
            let mut stats = new_stats();
            let (_, mut local) = queue::local();
            s1.steal_into(&mut local, &mut stats);

            while local.pop().is_some() {}
        });

        // Drain our tasks, then attempt to steal
        while l1.pop().is_some() {}

        s2.steal_into(&mut l1, &mut stats);

        th.join().unwrap();

        while l1.pop().is_some() {}
        while l2.pop().is_some() {}
    });
}
