use std::cell::Cell;
use std::mem;

use crossbeam_utils::CachePadded;

#[test]
fn default() {
    let x: CachePadded<u64> = Default::default();
    assert_eq!(*x, 0);
}

#[test]
fn store_u64() {
    let x: CachePadded<u64> = CachePadded::new(17);
    assert_eq!(*x, 17);
}

#[test]
fn store_pair() {
    let x: CachePadded<(u64, u64)> = CachePadded::new((17, 37));
    assert_eq!(x.0, 17);
    assert_eq!(x.1, 37);
}

#[test]
fn distance() {
    let arr = [CachePadded::new(17u8), CachePadded::new(37u8)];
    let a = &*arr[0] as *const u8;
    let b = &*arr[1] as *const u8;
    let align = mem::align_of::<CachePadded<()>>();
    assert!(align >= 32);
    assert_eq!(unsafe { a.add(align) }, b);
}

#[test]
fn different_sizes() {
    CachePadded::new(17u8);
    CachePadded::new(17u16);
    CachePadded::new(17u32);
    CachePadded::new([17u64; 0]);
    CachePadded::new([17u64; 1]);
    CachePadded::new([17u64; 2]);
    CachePadded::new([17u64; 3]);
    CachePadded::new([17u64; 4]);
    CachePadded::new([17u64; 5]);
    CachePadded::new([17u64; 6]);
    CachePadded::new([17u64; 7]);
    CachePadded::new([17u64; 8]);
}

#[test]
fn large() {
    let a = [17u64; 9];
    let b = CachePadded::new(a);
    assert!(mem::size_of_val(&a) <= mem::size_of_val(&b));
}

#[test]
fn debug() {
    assert_eq!(
        format!("{:?}", CachePadded::new(17u64)),
        "CachePadded { value: 17 }"
    );
}

#[test]
fn drops() {
    let count = Cell::new(0);

    struct Foo<'a>(&'a Cell<usize>);

    impl<'a> Drop for Foo<'a> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    let a = CachePadded::new(Foo(&count));
    let b = CachePadded::new(Foo(&count));

    assert_eq!(count.get(), 0);
    drop(a);
    assert_eq!(count.get(), 1);
    drop(b);
    assert_eq!(count.get(), 2);
}

#[allow(clippy::clone_on_copy)] // This is intentional.
#[test]
fn clone() {
    let a = CachePadded::new(17);
    let b = a.clone();
    assert_eq!(*a, *b);
}

#[test]
fn runs_custom_clone() {
    let count = Cell::new(0);

    struct Foo<'a>(&'a Cell<usize>);

    impl<'a> Clone for Foo<'a> {
        fn clone(&self) -> Foo<'a> {
            self.0.set(self.0.get() + 1);
            Foo::<'a>(self.0)
        }
    }

    let a = CachePadded::new(Foo(&count));
    let _ = a.clone();

    assert_eq!(count.get(), 1);
}
