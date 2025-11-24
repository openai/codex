use std::{
    cell::Cell,
    sync::atomic::{AtomicUsize, Ordering::SeqCst},
    thread::scope,
};

use once_cell::sync::{Lazy, OnceCell};

#[test]
fn lazy_new() {
    let called = AtomicUsize::new(0);
    let x = Lazy::new(|| {
        called.fetch_add(1, SeqCst);
        92
    });

    assert_eq!(called.load(SeqCst), 0);

    scope(|s| {
        s.spawn(|| {
            let y = *x - 30;
            assert_eq!(y, 62);
            assert_eq!(called.load(SeqCst), 1);
        });
    });

    let y = *x - 30;
    assert_eq!(y, 62);
    assert_eq!(called.load(SeqCst), 1);
}

#[test]
fn lazy_deref_mut() {
    let called = AtomicUsize::new(0);
    let mut x = Lazy::new(|| {
        called.fetch_add(1, SeqCst);
        92
    });

    assert_eq!(called.load(SeqCst), 0);

    let y = *x - 30;
    assert_eq!(y, 62);
    assert_eq!(called.load(SeqCst), 1);

    *x /= 2;
    assert_eq!(*x, 46);
    assert_eq!(called.load(SeqCst), 1);
}

#[test]
fn lazy_force_mut() {
    let called = Cell::new(0);
    let mut x = Lazy::new(|| {
        called.set(called.get() + 1);
        92
    });
    assert_eq!(called.get(), 0);
    let v = Lazy::force_mut(&mut x);
    assert_eq!(called.get(), 1);

    *v /= 2;
    assert_eq!(*x, 46);
    assert_eq!(called.get(), 1);
}

#[test]
fn lazy_get_mut() {
    let called = Cell::new(0);
    let mut x: Lazy<u32, _> = Lazy::new(|| {
        called.set(called.get() + 1);
        92
    });

    assert_eq!(called.get(), 0);
    assert_eq!(*x, 92);

    let mut_ref: &mut u32 = Lazy::get_mut(&mut x).unwrap();
    assert_eq!(called.get(), 1);

    *mut_ref /= 2;
    assert_eq!(*x, 46);
    assert_eq!(called.get(), 1);
}

#[test]
fn lazy_default() {
    static CALLED: AtomicUsize = AtomicUsize::new(0);

    struct Foo(u8);
    impl Default for Foo {
        fn default() -> Self {
            CALLED.fetch_add(1, SeqCst);
            Foo(42)
        }
    }

    let lazy: Lazy<std::sync::Mutex<Foo>> = <_>::default();

    assert_eq!(CALLED.load(SeqCst), 0);

    assert_eq!(lazy.lock().unwrap().0, 42);
    assert_eq!(CALLED.load(SeqCst), 1);

    lazy.lock().unwrap().0 = 21;

    assert_eq!(lazy.lock().unwrap().0, 21);
    assert_eq!(CALLED.load(SeqCst), 1);
}

#[test]
fn static_lazy() {
    static XS: Lazy<Vec<i32>> = Lazy::new(|| {
        let mut xs = Vec::new();
        xs.push(1);
        xs.push(2);
        xs.push(3);
        xs
    });
    scope(|s| {
        s.spawn(|| {
            assert_eq!(&*XS, &vec![1, 2, 3]);
        });
    });
    assert_eq!(&*XS, &vec![1, 2, 3]);
}

#[test]
fn static_lazy_via_fn() {
    fn xs() -> &'static Vec<i32> {
        static XS: OnceCell<Vec<i32>> = OnceCell::new();
        XS.get_or_init(|| {
            let mut xs = Vec::new();
            xs.push(1);
            xs.push(2);
            xs.push(3);
            xs
        })
    }
    assert_eq!(xs(), &vec![1, 2, 3]);
}

#[test]
fn lazy_into_value() {
    let l: Lazy<i32, _> = Lazy::new(|| panic!());
    assert!(matches!(Lazy::into_value(l), Err(_)));
    let l = Lazy::new(|| -> i32 { 92 });
    Lazy::force(&l);
    assert!(matches!(Lazy::into_value(l), Ok(92)));
}

#[test]
fn lazy_poisoning() {
    let x: Lazy<String> = Lazy::new(|| panic!("kaboom"));
    for _ in 0..2 {
        let res = std::panic::catch_unwind(|| x.len());
        assert!(res.is_err());
    }
}

#[test]
// https://github.com/rust-lang/rust/issues/34761#issuecomment-256320669
fn arrrrrrrrrrrrrrrrrrrrrr() {
    let lazy: Lazy<&String, _>;
    {
        let s = String::new();
        lazy = Lazy::new(|| &s);
        _ = *lazy;
    }
}

#[test]
fn lazy_is_sync_send() {
    fn assert_traits<T: Send + Sync>() {}
    assert_traits::<Lazy<String>>();
}
