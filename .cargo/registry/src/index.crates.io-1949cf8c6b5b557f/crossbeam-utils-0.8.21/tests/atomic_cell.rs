use std::mem;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;

use crossbeam_utils::atomic::AtomicCell;

#[test]
fn is_lock_free() {
    struct UsizeWrap(#[allow(dead_code)] usize);
    struct U8Wrap(#[allow(dead_code)] bool);
    struct I16Wrap(#[allow(dead_code)] i16);
    #[repr(align(8))]
    struct U64Align8(#[allow(dead_code)] u64);

    assert!(AtomicCell::<usize>::is_lock_free());
    assert!(AtomicCell::<isize>::is_lock_free());
    assert!(AtomicCell::<UsizeWrap>::is_lock_free());

    assert!(AtomicCell::<()>::is_lock_free());

    assert!(AtomicCell::<u8>::is_lock_free());
    assert!(AtomicCell::<i8>::is_lock_free());
    assert!(AtomicCell::<bool>::is_lock_free());
    assert!(AtomicCell::<U8Wrap>::is_lock_free());

    assert!(AtomicCell::<u16>::is_lock_free());
    assert!(AtomicCell::<i16>::is_lock_free());
    assert!(AtomicCell::<I16Wrap>::is_lock_free());

    assert!(AtomicCell::<u32>::is_lock_free());
    assert!(AtomicCell::<i32>::is_lock_free());

    // Sizes of both types must be equal, and the alignment of `u64` must be greater or equal than
    // that of `AtomicU64`. In i686-unknown-linux-gnu, the alignment of `u64` is `4` and alignment
    // of `AtomicU64` is `8`, so `AtomicCell<u64>` is not lock-free.
    assert_eq!(
        AtomicCell::<u64>::is_lock_free(),
        cfg!(target_has_atomic = "64") && std::mem::align_of::<u64>() == 8
    );
    assert_eq!(mem::size_of::<U64Align8>(), 8);
    assert_eq!(mem::align_of::<U64Align8>(), 8);
    assert_eq!(
        AtomicCell::<U64Align8>::is_lock_free(),
        cfg!(target_has_atomic = "64")
    );

    // AtomicU128 is unstable
    assert!(!AtomicCell::<u128>::is_lock_free());
}

#[test]
fn const_is_lock_free() {
    const _U: bool = AtomicCell::<usize>::is_lock_free();
    const _I: bool = AtomicCell::<isize>::is_lock_free();
}

#[test]
fn drops_unit() {
    static CNT: AtomicUsize = AtomicUsize::new(0);
    CNT.store(0, SeqCst);

    #[derive(Debug, PartialEq, Eq)]
    struct Foo();

    impl Foo {
        fn new() -> Foo {
            CNT.fetch_add(1, SeqCst);
            Foo()
        }
    }

    impl Drop for Foo {
        fn drop(&mut self) {
            CNT.fetch_sub(1, SeqCst);
        }
    }

    impl Default for Foo {
        fn default() -> Foo {
            Foo::new()
        }
    }

    let a = AtomicCell::new(Foo::new());

    assert_eq!(a.swap(Foo::new()), Foo::new());
    assert_eq!(CNT.load(SeqCst), 1);

    a.store(Foo::new());
    assert_eq!(CNT.load(SeqCst), 1);

    assert_eq!(a.swap(Foo::default()), Foo::new());
    assert_eq!(CNT.load(SeqCst), 1);

    drop(a);
    assert_eq!(CNT.load(SeqCst), 0);
}

#[test]
fn drops_u8() {
    static CNT: AtomicUsize = AtomicUsize::new(0);
    CNT.store(0, SeqCst);

    #[derive(Debug, PartialEq, Eq)]
    struct Foo(u8);

    impl Foo {
        fn new(val: u8) -> Foo {
            CNT.fetch_add(1, SeqCst);
            Foo(val)
        }
    }

    impl Drop for Foo {
        fn drop(&mut self) {
            CNT.fetch_sub(1, SeqCst);
        }
    }

    impl Default for Foo {
        fn default() -> Foo {
            Foo::new(0)
        }
    }

    let a = AtomicCell::new(Foo::new(5));

    assert_eq!(a.swap(Foo::new(6)), Foo::new(5));
    assert_eq!(a.swap(Foo::new(1)), Foo::new(6));
    assert_eq!(CNT.load(SeqCst), 1);

    a.store(Foo::new(2));
    assert_eq!(CNT.load(SeqCst), 1);

    assert_eq!(a.swap(Foo::default()), Foo::new(2));
    assert_eq!(CNT.load(SeqCst), 1);

    assert_eq!(a.swap(Foo::default()), Foo::new(0));
    assert_eq!(CNT.load(SeqCst), 1);

    drop(a);
    assert_eq!(CNT.load(SeqCst), 0);
}

#[test]
fn drops_usize() {
    static CNT: AtomicUsize = AtomicUsize::new(0);
    CNT.store(0, SeqCst);

    #[derive(Debug, PartialEq, Eq)]
    struct Foo(usize);

    impl Foo {
        fn new(val: usize) -> Foo {
            CNT.fetch_add(1, SeqCst);
            Foo(val)
        }
    }

    impl Drop for Foo {
        fn drop(&mut self) {
            CNT.fetch_sub(1, SeqCst);
        }
    }

    impl Default for Foo {
        fn default() -> Foo {
            Foo::new(0)
        }
    }

    let a = AtomicCell::new(Foo::new(5));

    assert_eq!(a.swap(Foo::new(6)), Foo::new(5));
    assert_eq!(a.swap(Foo::new(1)), Foo::new(6));
    assert_eq!(CNT.load(SeqCst), 1);

    a.store(Foo::new(2));
    assert_eq!(CNT.load(SeqCst), 1);

    assert_eq!(a.swap(Foo::default()), Foo::new(2));
    assert_eq!(CNT.load(SeqCst), 1);

    assert_eq!(a.swap(Foo::default()), Foo::new(0));
    assert_eq!(CNT.load(SeqCst), 1);

    drop(a);
    assert_eq!(CNT.load(SeqCst), 0);
}

#[test]
fn modular_u8() {
    #[derive(Clone, Copy, Eq, Debug, Default)]
    struct Foo(u8);

    impl PartialEq for Foo {
        fn eq(&self, other: &Foo) -> bool {
            self.0 % 5 == other.0 % 5
        }
    }

    let a = AtomicCell::new(Foo(1));

    assert_eq!(a.load(), Foo(1));
    assert_eq!(a.swap(Foo(2)), Foo(11));
    assert_eq!(a.load(), Foo(52));

    a.store(Foo(0));
    assert_eq!(a.compare_exchange(Foo(0), Foo(5)), Ok(Foo(100)));
    assert_eq!(a.load().0, 5);
    assert_eq!(a.compare_exchange(Foo(10), Foo(15)), Ok(Foo(100)));
    assert_eq!(a.load().0, 15);
}

#[test]
fn modular_usize() {
    #[derive(Clone, Copy, Eq, Debug, Default)]
    struct Foo(usize);

    impl PartialEq for Foo {
        fn eq(&self, other: &Foo) -> bool {
            self.0 % 5 == other.0 % 5
        }
    }

    let a = AtomicCell::new(Foo(1));

    assert_eq!(a.load(), Foo(1));
    assert_eq!(a.swap(Foo(2)), Foo(11));
    assert_eq!(a.load(), Foo(52));

    a.store(Foo(0));
    assert_eq!(a.compare_exchange(Foo(0), Foo(5)), Ok(Foo(100)));
    assert_eq!(a.load().0, 5);
    assert_eq!(a.compare_exchange(Foo(10), Foo(15)), Ok(Foo(100)));
    assert_eq!(a.load().0, 15);
}

#[test]
fn garbage_padding() {
    #[derive(Copy, Clone, Eq, PartialEq)]
    struct Object {
        a: i64,
        b: i32,
    }

    let cell = AtomicCell::new(Object { a: 0, b: 0 });
    let _garbage = [0xfe, 0xfe, 0xfe, 0xfe, 0xfe]; // Needed
    let next = Object { a: 0, b: 0 };

    let prev = cell.load();
    assert!(cell.compare_exchange(prev, next).is_ok());
    println!();
}

#[test]
fn const_atomic_cell_new() {
    static CELL: AtomicCell<usize> = AtomicCell::new(0);

    CELL.store(1);
    assert_eq!(CELL.load(), 1);
}

// https://github.com/crossbeam-rs/crossbeam/pull/767
macro_rules! test_arithmetic {
    ($test_name:ident, $ty:ident) => {
        #[test]
        fn $test_name() {
            let a: AtomicCell<$ty> = AtomicCell::new(7);

            assert_eq!(a.fetch_add(3), 7);
            assert_eq!(a.load(), 10);

            assert_eq!(a.fetch_sub(3), 10);
            assert_eq!(a.load(), 7);

            assert_eq!(a.fetch_and(3), 7);
            assert_eq!(a.load(), 3);

            assert_eq!(a.fetch_or(16), 3);
            assert_eq!(a.load(), 19);

            assert_eq!(a.fetch_xor(2), 19);
            assert_eq!(a.load(), 17);

            assert_eq!(a.fetch_max(18), 17);
            assert_eq!(a.load(), 18);

            assert_eq!(a.fetch_min(17), 18);
            assert_eq!(a.load(), 17);

            assert_eq!(a.fetch_nand(7), 17);
            assert_eq!(a.load(), !(17 & 7));
        }
    };
}
test_arithmetic!(arithmetic_u8, u8);
test_arithmetic!(arithmetic_i8, i8);
test_arithmetic!(arithmetic_u16, u16);
test_arithmetic!(arithmetic_i16, i16);
test_arithmetic!(arithmetic_u32, u32);
test_arithmetic!(arithmetic_i32, i32);
test_arithmetic!(arithmetic_u64, u64);
test_arithmetic!(arithmetic_i64, i64);
test_arithmetic!(arithmetic_u128, u128);
test_arithmetic!(arithmetic_i128, i128);

// https://github.com/crossbeam-rs/crossbeam/issues/748
#[cfg_attr(miri, ignore)] // TODO
#[test]
fn issue_748() {
    #[allow(dead_code)]
    #[repr(align(8))]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Test {
        Field(u32),
        FieldLess,
    }

    assert_eq!(mem::size_of::<Test>(), 8);
    assert_eq!(
        AtomicCell::<Test>::is_lock_free(),
        cfg!(target_has_atomic = "64")
    );
    let x = AtomicCell::new(Test::FieldLess);
    assert_eq!(x.load(), Test::FieldLess);
}

// https://github.com/crossbeam-rs/crossbeam/issues/833
#[test]
fn issue_833() {
    use std::num::NonZeroU128;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;

    #[cfg(miri)]
    const N: usize = 10_000;
    #[cfg(not(miri))]
    const N: usize = 1_000_000;

    #[allow(dead_code)]
    enum Enum {
        NeverConstructed,
        Cell(AtomicCell<NonZeroU128>),
    }

    static STATIC: Enum = Enum::Cell(AtomicCell::new(match NonZeroU128::new(1) {
        Some(nonzero) => nonzero,
        None => unreachable!(),
    }));
    static FINISHED: AtomicBool = AtomicBool::new(false);

    let handle = thread::spawn(|| {
        let cell = match &STATIC {
            Enum::NeverConstructed => unreachable!(),
            Enum::Cell(cell) => cell,
        };
        let x = NonZeroU128::new(0xFFFF_FFFF_FFFF_FFFF_0000_0000_0000_0000).unwrap();
        let y = NonZeroU128::new(0x0000_0000_0000_0000_FFFF_FFFF_FFFF_FFFF).unwrap();
        while !FINISHED.load(Ordering::Relaxed) {
            cell.store(x);
            cell.store(y);
        }
    });

    for _ in 0..N {
        if let Enum::NeverConstructed = STATIC {
            unreachable!(":(");
        }
    }

    FINISHED.store(true, Ordering::Relaxed);
    handle.join().unwrap();
}
