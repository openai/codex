#[cfg(feature = "std")]
use std::sync::Barrier;
use std::{
    num::NonZeroUsize,
    sync::atomic::{AtomicUsize, Ordering::SeqCst},
    thread::scope,
};

use once_cell::race::{OnceBool, OnceNonZeroUsize};

#[test]
fn once_non_zero_usize_smoke_test() {
    let cnt = AtomicUsize::new(0);
    let cell = OnceNonZeroUsize::new();
    let val = NonZeroUsize::new(92).unwrap();
    scope(|s| {
        s.spawn(|| {
            assert_eq!(
                cell.get_or_init(|| {
                    cnt.fetch_add(1, SeqCst);
                    val
                }),
                val
            );
            assert_eq!(cnt.load(SeqCst), 1);

            assert_eq!(
                cell.get_or_init(|| {
                    cnt.fetch_add(1, SeqCst);
                    val
                }),
                val
            );
            assert_eq!(cnt.load(SeqCst), 1);
        });
    });
    assert_eq!(cell.get(), Some(val));
    assert_eq!(cnt.load(SeqCst), 1);
}

#[test]
fn once_non_zero_usize_set() {
    let val1 = NonZeroUsize::new(92).unwrap();
    let val2 = NonZeroUsize::new(62).unwrap();

    let cell = OnceNonZeroUsize::new();

    assert!(cell.set(val1).is_ok());
    assert_eq!(cell.get(), Some(val1));

    assert!(cell.set(val2).is_err());
    assert_eq!(cell.get(), Some(val1));
}

#[cfg(feature = "std")]
#[test]
fn once_non_zero_usize_first_wins() {
    let val1 = NonZeroUsize::new(92).unwrap();
    let val2 = NonZeroUsize::new(62).unwrap();

    let cell = OnceNonZeroUsize::new();

    let b1 = Barrier::new(2);
    let b2 = Barrier::new(2);
    let b3 = Barrier::new(2);
    scope(|s| {
        s.spawn(|| {
            let r1 = cell.get_or_init(|| {
                b1.wait();
                b2.wait();
                val1
            });
            assert_eq!(r1, val1);
            b3.wait();
        });
        b1.wait();
        s.spawn(|| {
            let r2 = cell.get_or_init(|| {
                b2.wait();
                b3.wait();
                val2
            });
            assert_eq!(r2, val1);
        });
    });

    assert_eq!(cell.get(), Some(val1));
}

#[test]
fn once_bool_smoke_test() {
    let cnt = AtomicUsize::new(0);
    let cell = OnceBool::new();
    scope(|s| {
        s.spawn(|| {
            assert_eq!(
                cell.get_or_init(|| {
                    cnt.fetch_add(1, SeqCst);
                    false
                }),
                false
            );
            assert_eq!(cnt.load(SeqCst), 1);

            assert_eq!(
                cell.get_or_init(|| {
                    cnt.fetch_add(1, SeqCst);
                    false
                }),
                false
            );
            assert_eq!(cnt.load(SeqCst), 1);
        });
    });
    assert_eq!(cell.get(), Some(false));
    assert_eq!(cnt.load(SeqCst), 1);
}

#[test]
fn once_bool_set() {
    let cell = OnceBool::new();

    assert!(cell.set(false).is_ok());
    assert_eq!(cell.get(), Some(false));

    assert!(cell.set(true).is_err());
    assert_eq!(cell.get(), Some(false));
}

#[test]
fn get_unchecked() {
    let cell = OnceNonZeroUsize::new();
    cell.set(NonZeroUsize::new(92).unwrap()).unwrap();
    let value = unsafe { cell.get_unchecked() };
    assert_eq!(value, NonZeroUsize::new(92).unwrap());
}
