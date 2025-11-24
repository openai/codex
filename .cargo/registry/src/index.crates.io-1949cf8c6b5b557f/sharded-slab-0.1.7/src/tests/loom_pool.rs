use super::util::*;
use crate::{clear::Clear, sync::alloc, Pack, Pool};
use loom::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Condvar, Mutex,
    },
    thread,
};
use std::sync::Arc;

#[derive(Default, Debug)]
struct State {
    is_dropped: AtomicBool,
    is_cleared: AtomicBool,
    id: usize,
}

impl State {
    fn assert_clear(&self) {
        assert!(!self.is_dropped.load(Ordering::SeqCst));
        assert!(self.is_cleared.load(Ordering::SeqCst));
    }

    fn assert_not_clear(&self) {
        assert!(!self.is_dropped.load(Ordering::SeqCst));
        assert!(!self.is_cleared.load(Ordering::SeqCst));
    }
}

impl PartialEq for State {
    fn eq(&self, other: &State) -> bool {
        self.id.eq(&other.id)
    }
}

#[derive(Default, Debug)]
struct DontDropMe(Arc<State>);

impl PartialEq for DontDropMe {
    fn eq(&self, other: &DontDropMe) -> bool {
        self.0.eq(&other.0)
    }
}

impl DontDropMe {
    fn new(id: usize) -> (Arc<State>, Self) {
        let state = Arc::new(State {
            is_dropped: AtomicBool::new(false),
            is_cleared: AtomicBool::new(false),
            id,
        });
        (state.clone(), Self(state))
    }
}

impl Drop for DontDropMe {
    fn drop(&mut self) {
        test_println!("-> DontDropMe drop: dropping data {:?}", self.0.id);
        self.0.is_dropped.store(true, Ordering::SeqCst)
    }
}

impl Clear for DontDropMe {
    fn clear(&mut self) {
        test_println!("-> DontDropMe clear: clearing data {:?}", self.0.id);
        self.0.is_cleared.store(true, Ordering::SeqCst);
    }
}

#[test]
fn dont_drop() {
    run_model("dont_drop", || {
        let pool: Pool<DontDropMe> = Pool::new();
        let (item1, value) = DontDropMe::new(1);
        test_println!("-> dont_drop: Inserting into pool {}", item1.id);
        let idx = pool
            .create_with(move |item| *item = value)
            .expect("create_with");

        item1.assert_not_clear();

        test_println!("-> dont_drop: clearing idx: {}", idx);
        pool.clear(idx);

        item1.assert_clear();
    });
}

#[test]
fn concurrent_create_with_clear() {
    run_model("concurrent_create_with_clear", || {
        let pool: Arc<Pool<DontDropMe>> = Arc::new(Pool::new());
        let pair = Arc::new((Mutex::new(None), Condvar::new()));

        let (item1, value) = DontDropMe::new(1);
        let idx1 = pool
            .create_with(move |item| *item = value)
            .expect("create_with");
        let p = pool.clone();
        let pair2 = pair.clone();
        let test_value = item1.clone();
        let t1 = thread::spawn(move || {
            let (lock, cvar) = &*pair2;
            test_println!("-> making get request");
            assert_eq!(p.get(idx1).unwrap().0.id, test_value.id);
            let mut next = lock.lock().unwrap();
            *next = Some(());
            cvar.notify_one();
        });

        test_println!("-> making get request");
        let guard = pool.get(idx1);

        let (lock, cvar) = &*pair;
        let mut next = lock.lock().unwrap();
        // wait until we have a guard on the other thread.
        while next.is_none() {
            next = cvar.wait(next).unwrap();
        }
        // the item should be marked (clear returns true)...
        assert!(pool.clear(idx1));
        // ...but the value shouldn't be removed yet.
        item1.assert_not_clear();

        t1.join().expect("thread 1 unable to join");

        drop(guard);
        item1.assert_clear();
    })
}

#[test]
fn racy_clear() {
    run_model("racy_clear", || {
        let pool = Arc::new(Pool::new());
        let (item, value) = DontDropMe::new(1);

        let idx = pool
            .create_with(move |item| *item = value)
            .expect("create_with");
        assert_eq!(pool.get(idx).unwrap().0.id, item.id);

        let p = pool.clone();
        let t2 = thread::spawn(move || p.clear(idx));
        let r1 = pool.clear(idx);
        let r2 = t2.join().expect("thread 2 should not panic");

        test_println!("r1: {}, r2: {}", r1, r2);

        assert!(
            !(r1 && r2),
            "Both threads should not have cleared the value"
        );
        assert!(r1 || r2, "One thread should have removed the value");
        assert!(pool.get(idx).is_none());
        item.assert_clear();
    })
}

#[test]
fn clear_local_and_reuse() {
    run_model("take_remote_and_reuse", || {
        let pool = Arc::new(Pool::new_with_config::<TinyConfig>());

        let idx1 = pool
            .create_with(|item: &mut String| {
                item.push_str("hello world");
            })
            .expect("create_with");
        let idx2 = pool
            .create_with(|item| item.push_str("foo"))
            .expect("create_with");
        let idx3 = pool
            .create_with(|item| item.push_str("bar"))
            .expect("create_with");

        assert_eq!(pool.get(idx1).unwrap(), String::from("hello world"));
        assert_eq!(pool.get(idx2).unwrap(), String::from("foo"));
        assert_eq!(pool.get(idx3).unwrap(), String::from("bar"));

        let first = idx1 & (!crate::page::slot::Generation::<TinyConfig>::MASK);
        assert!(pool.clear(idx1));

        let idx1 = pool
            .create_with(move |item| item.push_str("h"))
            .expect("create_with");

        let second = idx1 & (!crate::page::slot::Generation::<TinyConfig>::MASK);
        assert_eq!(first, second);
        assert!(pool.get(idx1).unwrap().capacity() >= 11);
    })
}

#[test]
fn create_mut_guard_prevents_access() {
    run_model("create_mut_guard_prevents_access", || {
        let pool = Arc::new(Pool::<String>::new());
        let guard = pool.create().unwrap();
        let key: usize = guard.key();

        let pool2 = pool.clone();
        thread::spawn(move || {
            assert!(pool2.get(key).is_none());
        })
        .join()
        .unwrap();
    });
}

#[test]
fn create_mut_guard() {
    run_model("create_mut_guard", || {
        let pool = Arc::new(Pool::<String>::new());
        let mut guard = pool.create().unwrap();
        let key: usize = guard.key();

        let pool2 = pool.clone();
        let t1 = thread::spawn(move || {
            test_dbg!(pool2.get(key));
        });

        guard.push_str("Hello world");
        drop(guard);

        t1.join().unwrap();
    });
}

#[test]
fn create_mut_guard_2() {
    run_model("create_mut_guard_2", || {
        let pool = Arc::new(Pool::<String>::new());
        let mut guard = pool.create().unwrap();
        let key: usize = guard.key();

        let pool2 = pool.clone();
        let pool3 = pool.clone();
        let t1 = thread::spawn(move || {
            test_dbg!(pool2.get(key));
        });

        guard.push_str("Hello world");
        let t2 = thread::spawn(move || {
            test_dbg!(pool3.get(key));
        });
        drop(guard);

        t1.join().unwrap();
        t2.join().unwrap();
    });
}

#[test]
fn create_mut_guard_downgrade() {
    run_model("create_mut_guard_downgrade", || {
        let pool = Arc::new(Pool::<String>::new());
        let mut guard = pool.create().unwrap();
        let key: usize = guard.key();

        let pool2 = pool.clone();
        let pool3 = pool.clone();
        let t1 = thread::spawn(move || {
            test_dbg!(pool2.get(key));
        });

        guard.push_str("Hello world");
        let guard = guard.downgrade();
        let t2 = thread::spawn(move || {
            test_dbg!(pool3.get(key));
        });

        t1.join().unwrap();
        t2.join().unwrap();
        assert_eq!(guard, "Hello world".to_owned());
    });
}

#[test]
fn create_mut_guard_downgrade_clear() {
    run_model("create_mut_guard_downgrade_clear", || {
        let pool = Arc::new(Pool::<String>::new());
        let mut guard = pool.create().unwrap();
        let key: usize = guard.key();

        let pool2 = pool.clone();

        guard.push_str("Hello world");
        let guard = guard.downgrade();
        let pool3 = pool.clone();
        let t1 = thread::spawn(move || {
            test_dbg!(pool2.get(key));
        });
        let t2 = thread::spawn(move || {
            test_dbg!(pool3.clear(key));
        });

        assert_eq!(guard, "Hello world".to_owned());
        drop(guard);

        t1.join().unwrap();
        t2.join().unwrap();

        assert!(pool.get(key).is_none());
    });
}

#[test]
fn create_mut_downgrade_during_clear() {
    run_model("create_mut_downgrade_during_clear", || {
        let pool = Arc::new(Pool::<String>::new());
        let mut guard = pool.create().unwrap();
        let key: usize = guard.key();
        guard.push_str("Hello world");

        let pool2 = pool.clone();
        let guard = guard.downgrade();
        let t1 = thread::spawn(move || {
            test_dbg!(pool2.clear(key));
        });

        t1.join().unwrap();

        assert_eq!(guard, "Hello world".to_owned());
        drop(guard);

        assert!(pool.get(key).is_none());
    });
}

#[test]
fn ownedref_send_out_of_local() {
    run_model("ownedref_send_out_of_local", || {
        let pool = Arc::new(Pool::<alloc::Track<String>>::new());
        let key1 = pool
            .create_with(|item| item.get_mut().push_str("hello"))
            .expect("create item 1");
        let key2 = pool
            .create_with(|item| item.get_mut().push_str("goodbye"))
            .expect("create item 2");

        let item1 = pool.clone().get_owned(key1).expect("get key1");
        let item2 = pool.clone().get_owned(key2).expect("get key2");
        let pool2 = pool.clone();

        test_dbg!(pool.clear(key1));

        let t1 = thread::spawn(move || {
            assert_eq!(item1.get_ref(), &String::from("hello"));
            drop(item1);
        });
        let t2 = thread::spawn(move || {
            assert_eq!(item2.get_ref(), &String::from("goodbye"));
            test_dbg!(pool2.clear(key2));
            drop(item2);
        });

        t1.join().unwrap();
        t2.join().unwrap();

        assert!(pool.get(key1).is_none());
        assert!(pool.get(key2).is_none());
    });
}

#[test]
fn ownedrefs_outlive_pool() {
    run_model("ownedrefs_outlive_pool", || {
        let pool = Arc::new(Pool::<alloc::Track<String>>::new());
        let key1 = pool
            .create_with(|item| item.get_mut().push_str("hello"))
            .expect("create item 1");
        let key2 = pool
            .create_with(|item| item.get_mut().push_str("goodbye"))
            .expect("create item 2");

        let item1_1 = pool.clone().get_owned(key1).expect("get key1");
        let item1_2 = pool.clone().get_owned(key1).expect("get key1 again");
        let item2 = pool.clone().get_owned(key2).expect("get key2");
        drop(pool);

        let t1 = thread::spawn(move || {
            assert_eq!(item1_1.get_ref(), &String::from("hello"));
            drop(item1_1);
        });

        let t2 = thread::spawn(move || {
            assert_eq!(item2.get_ref(), &String::from("goodbye"));
            drop(item2);
        });

        t1.join().unwrap();
        t2.join().unwrap();

        assert_eq!(item1_2.get_ref(), &String::from("hello"));
    });
}

#[test]
fn ownedref_ping_pong() {
    run_model("ownedref_ping_pong", || {
        let pool = Arc::new(Pool::<alloc::Track<String>>::new());
        let key1 = pool
            .create_with(|item| item.get_mut().push_str("hello"))
            .expect("create item 1");
        let key2 = pool
            .create_with(|item| item.get_mut().push_str("world"))
            .expect("create item 2");

        let item1 = pool.clone().get_owned(key1).expect("get key1");
        let pool2 = pool.clone();
        let pool3 = pool.clone();

        let t1 = thread::spawn(move || {
            assert_eq!(item1.get_ref(), &String::from("hello"));
            pool2.clear(key1);
            item1
        });

        let t2 = thread::spawn(move || {
            let item2 = pool3.clone().get_owned(key2).unwrap();
            assert_eq!(item2.get_ref(), &String::from("world"));
            pool3.clear(key1);
            item2
        });

        let item1 = t1.join().unwrap();
        let item2 = t2.join().unwrap();

        assert_eq!(item1.get_ref(), &String::from("hello"));
        assert_eq!(item2.get_ref(), &String::from("world"));
    });
}

#[test]
fn ownedref_drop_from_other_threads() {
    run_model("ownedref_drop_from_other_threads", || {
        let pool = Arc::new(Pool::<alloc::Track<String>>::new());
        let key1 = pool
            .create_with(|item| item.get_mut().push_str("hello"))
            .expect("create item 1");
        let item1 = pool.clone().get_owned(key1).expect("get key1");

        let pool2 = pool.clone();

        let t1 = thread::spawn(move || {
            let pool = pool2.clone();
            let key2 = pool
                .create_with(|item| item.get_mut().push_str("goodbye"))
                .expect("create item 1");
            let item2 = pool.clone().get_owned(key2).expect("get key1");
            let t2 = thread::spawn(move || {
                assert_eq!(item2.get_ref(), &String::from("goodbye"));
                test_dbg!(pool2.clear(key1));
                drop(item2)
            });
            assert_eq!(item1.get_ref(), &String::from("hello"));
            test_dbg!(pool.clear(key2));
            drop(item1);
            (t2, key2)
        });

        let (t2, key2) = t1.join().unwrap();
        test_dbg!(pool.get(key1));
        test_dbg!(pool.get(key2));

        t2.join().unwrap();

        assert!(pool.get(key1).is_none());
        assert!(pool.get(key2).is_none());
    });
}

#[test]
fn create_owned_mut_guard() {
    run_model("create_owned_mut_guard", || {
        let pool = Arc::new(Pool::<String>::new());
        let mut guard = pool.clone().create_owned().unwrap();
        let key: usize = guard.key();

        let pool2 = pool.clone();
        let t1 = thread::spawn(move || {
            test_dbg!(pool2.get(key));
        });

        guard.push_str("Hello world");
        drop(guard);

        t1.join().unwrap();
    });
}

#[test]
fn create_owned_mut_guard_send() {
    run_model("create_owned_mut_guard", || {
        let pool = Arc::new(Pool::<String>::new());
        let mut guard = pool.clone().create_owned().unwrap();
        let key: usize = guard.key();

        let pool2 = pool.clone();
        let t1 = thread::spawn(move || {
            test_dbg!(pool2.get(key));
        });

        let t2 = thread::spawn(move || {
            guard.push_str("Hello world");
            drop(guard);
        });

        t1.join().unwrap();
        t2.join().unwrap();
    });
}

#[test]
fn create_owned_mut_guard_2() {
    run_model("create_owned_mut_guard_2", || {
        let pool = Arc::new(Pool::<String>::new());
        let mut guard = pool.clone().create_owned().unwrap();
        let key: usize = guard.key();

        let pool2 = pool.clone();
        let pool3 = pool.clone();
        let t1 = thread::spawn(move || {
            test_dbg!(pool2.get(key));
        });

        guard.push_str("Hello world");
        let t2 = thread::spawn(move || {
            test_dbg!(pool3.get(key));
        });
        drop(guard);

        t1.join().unwrap();
        t2.join().unwrap();
    });
}

#[test]
fn create_owned_mut_guard_downgrade() {
    run_model("create_owned_mut_guard_downgrade", || {
        let pool = Arc::new(Pool::<String>::new());
        let mut guard = pool.clone().create_owned().unwrap();
        guard.push_str("Hello world");

        let key: usize = guard.key();

        let pool2 = pool.clone();
        let pool3 = pool.clone();
        let t1 = thread::spawn(move || {
            test_dbg!(pool2.get(key));
        });

        let guard = guard.downgrade();
        let t2 = thread::spawn(move || {
            assert_eq!(pool3.get(key).unwrap(), "Hello world".to_owned());
        });

        t1.join().unwrap();
        t2.join().unwrap();
        assert_eq!(guard, "Hello world".to_owned());
    });
}

#[test]
fn create_owned_mut_guard_downgrade_then_clear() {
    run_model("create_owned_mut_guard_downgrade_then_clear", || {
        let pool = Arc::new(Pool::<String>::new());
        let mut guard = pool.clone().create_owned().unwrap();
        let key: usize = guard.key();

        let pool2 = pool.clone();

        guard.push_str("Hello world");
        let guard = guard.downgrade();
        let pool3 = pool.clone();
        let t1 = thread::spawn(move || {
            test_dbg!(pool2.get(key));
        });
        let t2 = thread::spawn(move || {
            test_dbg!(pool3.clear(key));
        });

        assert_eq!(guard, "Hello world".to_owned());
        drop(guard);

        t1.join().unwrap();
        t2.join().unwrap();

        assert!(pool.get(key).is_none());
    });
}

#[test]
fn create_owned_mut_downgrade_during_clear() {
    run_model("create_owned_mut_downgrade_during_clear", || {
        let pool = Arc::new(Pool::<String>::new());
        let mut guard = pool.clone().create_owned().unwrap();
        let key: usize = guard.key();
        guard.push_str("Hello world");

        let pool2 = pool.clone();
        let guard = guard.downgrade();
        let t1 = thread::spawn(move || {
            test_dbg!(pool2.clear(key));
        });

        t1.join().unwrap();

        assert_eq!(guard, "Hello world".to_owned());
        drop(guard);

        assert!(pool.get(key).is_none());
    });
}

#[test]
fn create_mut_downgrade_during_clear_by_other_thead() {
    run_model("create_mut_downgrade_during_clear_by_other_thread", || {
        let pool = Arc::new(Pool::<String>::new());
        let mut guard = pool.clone().create_owned().unwrap();
        let key: usize = guard.key();
        guard.push_str("Hello world");

        let pool2 = pool.clone();
        let t1 = thread::spawn(move || {
            let guard = guard.downgrade();
            assert_eq!(guard, "Hello world".to_owned());
            drop(guard);
        });

        let t2 = thread::spawn(move || {
            test_dbg!(pool2.clear(key));
        });

        test_dbg!(pool.get(key));

        t1.join().unwrap();
        t2.join().unwrap();
    });
}
