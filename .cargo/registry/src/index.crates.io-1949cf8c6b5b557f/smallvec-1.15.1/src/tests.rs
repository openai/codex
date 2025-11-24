use crate::{smallvec, SmallVec};

use std::iter::FromIterator;

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::{vec, vec::Vec};

#[test]
pub fn test_zero() {
    let mut v = SmallVec::<[_; 0]>::new();
    assert!(!v.spilled());
    v.push(0usize);
    assert!(v.spilled());
    assert_eq!(&*v, &[0]);
}

// We heap allocate all these strings so that double frees will show up under valgrind.

#[test]
pub fn test_inline() {
    let mut v = SmallVec::<[_; 16]>::new();
    v.push("hello".to_owned());
    v.push("there".to_owned());
    assert_eq!(&*v, &["hello".to_owned(), "there".to_owned(),][..]);
}

#[test]
pub fn test_spill() {
    let mut v = SmallVec::<[_; 2]>::new();
    v.push("hello".to_owned());
    assert_eq!(v[0], "hello");
    v.push("there".to_owned());
    v.push("burma".to_owned());
    assert_eq!(v[0], "hello");
    v.push("shave".to_owned());
    assert_eq!(
        &*v,
        &[
            "hello".to_owned(),
            "there".to_owned(),
            "burma".to_owned(),
            "shave".to_owned(),
        ][..]
    );
}

#[test]
pub fn test_double_spill() {
    let mut v = SmallVec::<[_; 2]>::new();
    v.push("hello".to_owned());
    v.push("there".to_owned());
    v.push("burma".to_owned());
    v.push("shave".to_owned());
    v.push("hello".to_owned());
    v.push("there".to_owned());
    v.push("burma".to_owned());
    v.push("shave".to_owned());
    assert_eq!(
        &*v,
        &[
            "hello".to_owned(),
            "there".to_owned(),
            "burma".to_owned(),
            "shave".to_owned(),
            "hello".to_owned(),
            "there".to_owned(),
            "burma".to_owned(),
            "shave".to_owned(),
        ][..]
    );
}

// https://github.com/servo/rust-smallvec/issues/4
#[test]
fn issue_4() {
    SmallVec::<[Box<u32>; 2]>::new();
}

// https://github.com/servo/rust-smallvec/issues/5
#[test]
fn issue_5() {
    assert!(Some(SmallVec::<[&u32; 2]>::new()).is_some());
}

#[test]
fn test_with_capacity() {
    let v: SmallVec<[u8; 3]> = SmallVec::with_capacity(1);
    assert!(v.is_empty());
    assert!(!v.spilled());
    assert_eq!(v.capacity(), 3);

    let v: SmallVec<[u8; 3]> = SmallVec::with_capacity(10);
    assert!(v.is_empty());
    assert!(v.spilled());
    assert_eq!(v.capacity(), 10);
}

#[test]
fn drain() {
    let mut v: SmallVec<[u8; 2]> = SmallVec::new();
    v.push(3);
    assert_eq!(v.drain(..).collect::<Vec<_>>(), &[3]);

    // spilling the vec
    v.push(3);
    v.push(4);
    v.push(5);
    let old_capacity = v.capacity();
    assert_eq!(v.drain(1..).collect::<Vec<_>>(), &[4, 5]);
    // drain should not change the capacity
    assert_eq!(v.capacity(), old_capacity);

    // Exercise the tail-shifting code when in the inline state
    // This has the potential to produce UB due to aliasing
    let mut v: SmallVec<[u8; 2]> = SmallVec::new();
    v.push(1);
    v.push(2);
    assert_eq!(v.drain(..1).collect::<Vec<_>>(), &[1]);
}

#[test]
fn drain_rev() {
    let mut v: SmallVec<[u8; 2]> = SmallVec::new();
    v.push(3);
    assert_eq!(v.drain(..).rev().collect::<Vec<_>>(), &[3]);

    // spilling the vec
    v.push(3);
    v.push(4);
    v.push(5);
    assert_eq!(v.drain(..).rev().collect::<Vec<_>>(), &[5, 4, 3]);
}

#[test]
fn drain_forget() {
    let mut v: SmallVec<[u8; 1]> = smallvec![0, 1, 2, 3, 4, 5, 6, 7];
    std::mem::forget(v.drain(2..5));
    assert_eq!(v.len(), 2);
}

#[test]
fn into_iter() {
    let mut v: SmallVec<[u8; 2]> = SmallVec::new();
    v.push(3);
    assert_eq!(v.into_iter().collect::<Vec<_>>(), &[3]);

    // spilling the vec
    let mut v: SmallVec<[u8; 2]> = SmallVec::new();
    v.push(3);
    v.push(4);
    v.push(5);
    assert_eq!(v.into_iter().collect::<Vec<_>>(), &[3, 4, 5]);
}

#[test]
fn into_iter_rev() {
    let mut v: SmallVec<[u8; 2]> = SmallVec::new();
    v.push(3);
    assert_eq!(v.into_iter().rev().collect::<Vec<_>>(), &[3]);

    // spilling the vec
    let mut v: SmallVec<[u8; 2]> = SmallVec::new();
    v.push(3);
    v.push(4);
    v.push(5);
    assert_eq!(v.into_iter().rev().collect::<Vec<_>>(), &[5, 4, 3]);
}

#[test]
fn into_iter_drop() {
    use std::cell::Cell;

    struct DropCounter<'a>(&'a Cell<i32>);

    impl<'a> Drop for DropCounter<'a> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    {
        let cell = Cell::new(0);
        let mut v: SmallVec<[DropCounter<'_>; 2]> = SmallVec::new();
        v.push(DropCounter(&cell));
        v.into_iter();
        assert_eq!(cell.get(), 1);
    }

    {
        let cell = Cell::new(0);
        let mut v: SmallVec<[DropCounter<'_>; 2]> = SmallVec::new();
        v.push(DropCounter(&cell));
        v.push(DropCounter(&cell));
        assert!(v.into_iter().next().is_some());
        assert_eq!(cell.get(), 2);
    }

    {
        let cell = Cell::new(0);
        let mut v: SmallVec<[DropCounter<'_>; 2]> = SmallVec::new();
        v.push(DropCounter(&cell));
        v.push(DropCounter(&cell));
        v.push(DropCounter(&cell));
        assert!(v.into_iter().next().is_some());
        assert_eq!(cell.get(), 3);
    }
    {
        let cell = Cell::new(0);
        let mut v: SmallVec<[DropCounter<'_>; 2]> = SmallVec::new();
        v.push(DropCounter(&cell));
        v.push(DropCounter(&cell));
        v.push(DropCounter(&cell));
        {
            let mut it = v.into_iter();
            assert!(it.next().is_some());
            assert!(it.next_back().is_some());
        }
        assert_eq!(cell.get(), 3);
    }
}

#[test]
fn test_capacity() {
    let mut v: SmallVec<[u8; 2]> = SmallVec::new();
    v.reserve(1);
    assert_eq!(v.capacity(), 2);
    assert!(!v.spilled());

    v.reserve_exact(0x100);
    assert!(v.capacity() >= 0x100);

    v.push(0);
    v.push(1);
    v.push(2);
    v.push(3);

    v.shrink_to_fit();
    assert!(v.capacity() < 0x100);
}

#[test]
fn test_truncate() {
    let mut v: SmallVec<[Box<u8>; 8]> = SmallVec::new();

    for x in 0..8 {
        v.push(Box::new(x));
    }
    v.truncate(4);

    assert_eq!(v.len(), 4);
    assert!(!v.spilled());

    assert_eq!(*v.swap_remove(1), 1);
    assert_eq!(*v.remove(1), 3);
    v.insert(1, Box::new(3));

    assert_eq!(&v.iter().map(|v| **v).collect::<Vec<_>>(), &[0, 3, 2]);
}

#[test]
fn test_insert_many() {
    let mut v: SmallVec<[u8; 8]> = SmallVec::new();
    for x in 0..4 {
        v.push(x);
    }
    assert_eq!(v.len(), 4);
    v.insert_many(1, [5, 6].iter().cloned());
    assert_eq!(
        &v.iter().map(|v| *v).collect::<Vec<_>>(),
        &[0, 5, 6, 1, 2, 3]
    );
}

struct MockHintIter<T: Iterator> {
    x: T,
    hint: usize,
}
impl<T: Iterator> Iterator for MockHintIter<T> {
    type Item = T::Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.x.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.hint, None)
    }
}

#[test]
fn test_insert_many_short_hint() {
    let mut v: SmallVec<[u8; 8]> = SmallVec::new();
    for x in 0..4 {
        v.push(x);
    }
    assert_eq!(v.len(), 4);
    v.insert_many(
        1,
        MockHintIter {
            x: [5, 6].iter().cloned(),
            hint: 5,
        },
    );
    assert_eq!(
        &v.iter().map(|v| *v).collect::<Vec<_>>(),
        &[0, 5, 6, 1, 2, 3]
    );
}

#[test]
fn test_insert_many_long_hint() {
    let mut v: SmallVec<[u8; 8]> = SmallVec::new();
    for x in 0..4 {
        v.push(x);
    }
    assert_eq!(v.len(), 4);
    v.insert_many(
        1,
        MockHintIter {
            x: [5, 6].iter().cloned(),
            hint: 1,
        },
    );
    assert_eq!(
        &v.iter().map(|v| *v).collect::<Vec<_>>(),
        &[0, 5, 6, 1, 2, 3]
    );
}

// https://github.com/servo/rust-smallvec/issues/96
mod insert_many_panic {
    use crate::{smallvec, SmallVec};
    use alloc::boxed::Box;

    struct PanicOnDoubleDrop {
        dropped: Box<bool>,
    }

    impl PanicOnDoubleDrop {
        fn new() -> Self {
            Self {
                dropped: Box::new(false),
            }
        }
    }

    impl Drop for PanicOnDoubleDrop {
        fn drop(&mut self) {
            assert!(!*self.dropped, "already dropped");
            *self.dropped = true;
        }
    }

    /// Claims to yield `hint` items, but actually yields `count`, then panics.
    struct BadIter {
        hint: usize,
        count: usize,
    }

    impl Iterator for BadIter {
        type Item = PanicOnDoubleDrop;
        fn size_hint(&self) -> (usize, Option<usize>) {
            (self.hint, None)
        }
        fn next(&mut self) -> Option<Self::Item> {
            if self.count == 0 {
                panic!()
            }
            self.count -= 1;
            Some(PanicOnDoubleDrop::new())
        }
    }

    #[test]
    fn panic_early_at_start() {
        let mut vec: SmallVec<[PanicOnDoubleDrop; 0]> =
            smallvec![PanicOnDoubleDrop::new(), PanicOnDoubleDrop::new(),];
        let result = ::std::panic::catch_unwind(move || {
            vec.insert_many(0, BadIter { hint: 1, count: 0 });
        });
        assert!(result.is_err());
    }

    #[test]
    fn panic_early_in_middle() {
        let mut vec: SmallVec<[PanicOnDoubleDrop; 0]> =
            smallvec![PanicOnDoubleDrop::new(), PanicOnDoubleDrop::new(),];
        let result = ::std::panic::catch_unwind(move || {
            vec.insert_many(1, BadIter { hint: 4, count: 2 });
        });
        assert!(result.is_err());
    }

    #[test]
    fn panic_early_at_end() {
        let mut vec: SmallVec<[PanicOnDoubleDrop; 0]> =
            smallvec![PanicOnDoubleDrop::new(), PanicOnDoubleDrop::new(),];
        let result = ::std::panic::catch_unwind(move || {
            vec.insert_many(2, BadIter { hint: 3, count: 1 });
        });
        assert!(result.is_err());
    }

    #[test]
    fn panic_late_at_start() {
        let mut vec: SmallVec<[PanicOnDoubleDrop; 0]> =
            smallvec![PanicOnDoubleDrop::new(), PanicOnDoubleDrop::new(),];
        let result = ::std::panic::catch_unwind(move || {
            vec.insert_many(0, BadIter { hint: 3, count: 5 });
        });
        assert!(result.is_err());
    }

    #[test]
    fn panic_late_at_end() {
        let mut vec: SmallVec<[PanicOnDoubleDrop; 0]> =
            smallvec![PanicOnDoubleDrop::new(), PanicOnDoubleDrop::new(),];
        let result = ::std::panic::catch_unwind(move || {
            vec.insert_many(2, BadIter { hint: 3, count: 5 });
        });
        assert!(result.is_err());
    }
}

#[test]
#[should_panic]
fn test_invalid_grow() {
    let mut v: SmallVec<[u8; 8]> = SmallVec::new();
    v.extend(0..8);
    v.grow(5);
}

#[test]
#[should_panic]
fn drain_overflow() {
    let mut v: SmallVec<[u8; 8]> = smallvec![0];
    v.drain(..=std::usize::MAX);
}

#[test]
fn test_insert_from_slice() {
    let mut v: SmallVec<[u8; 8]> = SmallVec::new();
    for x in 0..4 {
        v.push(x);
    }
    assert_eq!(v.len(), 4);
    v.insert_from_slice(1, &[5, 6]);
    assert_eq!(
        &v.iter().map(|v| *v).collect::<Vec<_>>(),
        &[0, 5, 6, 1, 2, 3]
    );
}

#[test]
fn test_extend_from_slice() {
    let mut v: SmallVec<[u8; 8]> = SmallVec::new();
    for x in 0..4 {
        v.push(x);
    }
    assert_eq!(v.len(), 4);
    v.extend_from_slice(&[5, 6]);
    assert_eq!(
        &v.iter().map(|v| *v).collect::<Vec<_>>(),
        &[0, 1, 2, 3, 5, 6]
    );
}

#[test]
#[should_panic]
fn test_drop_panic_smallvec() {
    // This test should only panic once, and not double panic,
    // which would mean a double drop
    struct DropPanic;

    impl Drop for DropPanic {
        fn drop(&mut self) {
            panic!("drop");
        }
    }

    let mut v = SmallVec::<[_; 1]>::new();
    v.push(DropPanic);
}

#[test]
fn test_eq() {
    let mut a: SmallVec<[u32; 2]> = SmallVec::new();
    let mut b: SmallVec<[u32; 2]> = SmallVec::new();
    let mut c: SmallVec<[u32; 2]> = SmallVec::new();
    // a = [1, 2]
    a.push(1);
    a.push(2);
    // b = [1, 2]
    b.push(1);
    b.push(2);
    // c = [3, 4]
    c.push(3);
    c.push(4);

    assert!(a == b);
    assert!(a != c);
}

#[test]
fn test_ord() {
    let mut a: SmallVec<[u32; 2]> = SmallVec::new();
    let mut b: SmallVec<[u32; 2]> = SmallVec::new();
    let mut c: SmallVec<[u32; 2]> = SmallVec::new();
    // a = [1]
    a.push(1);
    // b = [1, 1]
    b.push(1);
    b.push(1);
    // c = [1, 2]
    c.push(1);
    c.push(2);

    assert!(a < b);
    assert!(b > a);
    assert!(b < c);
    assert!(c > b);
}

#[test]
fn test_hash() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;

    {
        let mut a: SmallVec<[u32; 2]> = SmallVec::new();
        let b = [1, 2];
        a.extend(b.iter().cloned());
        let mut hasher = DefaultHasher::new();
        assert_eq!(a.hash(&mut hasher), b.hash(&mut hasher));
    }
    {
        let mut a: SmallVec<[u32; 2]> = SmallVec::new();
        let b = [1, 2, 11, 12];
        a.extend(b.iter().cloned());
        let mut hasher = DefaultHasher::new();
        assert_eq!(a.hash(&mut hasher), b.hash(&mut hasher));
    }
}

#[test]
fn test_as_ref() {
    let mut a: SmallVec<[u32; 2]> = SmallVec::new();
    a.push(1);
    assert_eq!(a.as_ref(), [1]);
    a.push(2);
    assert_eq!(a.as_ref(), [1, 2]);
    a.push(3);
    assert_eq!(a.as_ref(), [1, 2, 3]);
}

#[test]
fn test_as_mut() {
    let mut a: SmallVec<[u32; 2]> = SmallVec::new();
    a.push(1);
    assert_eq!(a.as_mut(), [1]);
    a.push(2);
    assert_eq!(a.as_mut(), [1, 2]);
    a.push(3);
    assert_eq!(a.as_mut(), [1, 2, 3]);
    a.as_mut()[1] = 4;
    assert_eq!(a.as_mut(), [1, 4, 3]);
}

#[test]
fn test_borrow() {
    use std::borrow::Borrow;

    let mut a: SmallVec<[u32; 2]> = SmallVec::new();
    a.push(1);
    assert_eq!(a.borrow(), [1]);
    a.push(2);
    assert_eq!(a.borrow(), [1, 2]);
    a.push(3);
    assert_eq!(a.borrow(), [1, 2, 3]);
}

#[test]
fn test_borrow_mut() {
    use std::borrow::BorrowMut;

    let mut a: SmallVec<[u32; 2]> = SmallVec::new();
    a.push(1);
    assert_eq!(a.borrow_mut(), [1]);
    a.push(2);
    assert_eq!(a.borrow_mut(), [1, 2]);
    a.push(3);
    assert_eq!(a.borrow_mut(), [1, 2, 3]);
    BorrowMut::<[u32]>::borrow_mut(&mut a)[1] = 4;
    assert_eq!(a.borrow_mut(), [1, 4, 3]);
}

#[test]
fn test_from() {
    assert_eq!(&SmallVec::<[u32; 2]>::from(&[1][..])[..], [1]);
    assert_eq!(&SmallVec::<[u32; 2]>::from(&[1, 2, 3][..])[..], [1, 2, 3]);

    let vec = vec![];
    let small_vec: SmallVec<[u8; 3]> = SmallVec::from(vec);
    assert_eq!(&*small_vec, &[]);
    drop(small_vec);

    let vec = vec![1, 2, 3, 4, 5];
    let small_vec: SmallVec<[u8; 3]> = SmallVec::from(vec);
    assert_eq!(&*small_vec, &[1, 2, 3, 4, 5]);
    drop(small_vec);

    let vec = vec![1, 2, 3, 4, 5];
    let small_vec: SmallVec<[u8; 1]> = SmallVec::from(vec);
    assert_eq!(&*small_vec, &[1, 2, 3, 4, 5]);
    drop(small_vec);

    let array = [1];
    let small_vec: SmallVec<[u8; 1]> = SmallVec::from(array);
    assert_eq!(&*small_vec, &[1]);
    drop(small_vec);

    let array = [99; 128];
    let small_vec: SmallVec<[u8; 128]> = SmallVec::from(array);
    assert_eq!(&*small_vec, vec![99u8; 128].as_slice());
    drop(small_vec);
}

#[test]
fn test_from_slice() {
    assert_eq!(&SmallVec::<[u32; 2]>::from_slice(&[1][..])[..], [1]);
    assert_eq!(
        &SmallVec::<[u32; 2]>::from_slice(&[1, 2, 3][..])[..],
        [1, 2, 3]
    );
}

#[test]
fn test_exact_size_iterator() {
    let mut vec = SmallVec::<[u32; 2]>::from(&[1, 2, 3][..]);
    assert_eq!(vec.clone().into_iter().len(), 3);
    assert_eq!(vec.drain(..2).len(), 2);
    assert_eq!(vec.into_iter().len(), 1);
}

#[test]
fn test_into_iter_as_slice() {
    let vec = SmallVec::<[u32; 2]>::from(&[1, 2, 3][..]);
    let mut iter = vec.clone().into_iter();
    assert_eq!(iter.as_slice(), &[1, 2, 3]);
    assert_eq!(iter.as_mut_slice(), &[1, 2, 3]);
    iter.next();
    assert_eq!(iter.as_slice(), &[2, 3]);
    assert_eq!(iter.as_mut_slice(), &[2, 3]);
    iter.next_back();
    assert_eq!(iter.as_slice(), &[2]);
    assert_eq!(iter.as_mut_slice(), &[2]);
}

#[test]
fn test_into_iter_clone() {
    // Test that the cloned iterator yields identical elements and that it owns its own copy
    // (i.e. no use after move errors).
    let mut iter = SmallVec::<[u8; 2]>::from_iter(0..3).into_iter();
    let mut clone_iter = iter.clone();
    while let Some(x) = iter.next() {
        assert_eq!(x, clone_iter.next().unwrap());
    }
    assert_eq!(clone_iter.next(), None);
}

#[test]
fn test_into_iter_clone_partially_consumed_iterator() {
    // Test that the cloned iterator only contains the remaining elements of the original iterator.
    let mut iter = SmallVec::<[u8; 2]>::from_iter(0..3).into_iter().skip(1);
    let mut clone_iter = iter.clone();
    while let Some(x) = iter.next() {
        assert_eq!(x, clone_iter.next().unwrap());
    }
    assert_eq!(clone_iter.next(), None);
}

#[test]
fn test_into_iter_clone_empty_smallvec() {
    let mut iter = SmallVec::<[u8; 2]>::new().into_iter();
    let mut clone_iter = iter.clone();
    assert_eq!(iter.next(), None);
    assert_eq!(clone_iter.next(), None);
}

#[test]
fn shrink_to_fit_unspill() {
    let mut vec = SmallVec::<[u8; 2]>::from_iter(0..3);
    vec.pop();
    assert!(vec.spilled());
    vec.shrink_to_fit();
    assert!(!vec.spilled(), "shrink_to_fit will un-spill if possible");
}

#[test]
fn test_into_vec() {
    let vec = SmallVec::<[u8; 2]>::from_iter(0..2);
    assert_eq!(vec.into_vec(), vec![0, 1]);

    let vec = SmallVec::<[u8; 2]>::from_iter(0..3);
    assert_eq!(vec.into_vec(), vec![0, 1, 2]);
}

#[test]
fn test_into_inner() {
    let vec = SmallVec::<[u8; 2]>::from_iter(0..2);
    assert_eq!(vec.into_inner(), Ok([0, 1]));

    let vec = SmallVec::<[u8; 2]>::from_iter(0..1);
    assert_eq!(vec.clone().into_inner(), Err(vec));

    let vec = SmallVec::<[u8; 2]>::from_iter(0..3);
    assert_eq!(vec.clone().into_inner(), Err(vec));
}

#[test]
fn test_from_vec() {
    let vec = vec![];
    let small_vec: SmallVec<[u8; 3]> = SmallVec::from_vec(vec);
    assert_eq!(&*small_vec, &[]);
    drop(small_vec);

    let vec = vec![];
    let small_vec: SmallVec<[u8; 1]> = SmallVec::from_vec(vec);
    assert_eq!(&*small_vec, &[]);
    drop(small_vec);

    let vec = vec![1];
    let small_vec: SmallVec<[u8; 3]> = SmallVec::from_vec(vec);
    assert_eq!(&*small_vec, &[1]);
    drop(small_vec);

    let vec = vec![1, 2, 3];
    let small_vec: SmallVec<[u8; 3]> = SmallVec::from_vec(vec);
    assert_eq!(&*small_vec, &[1, 2, 3]);
    drop(small_vec);

    let vec = vec![1, 2, 3, 4, 5];
    let small_vec: SmallVec<[u8; 3]> = SmallVec::from_vec(vec);
    assert_eq!(&*small_vec, &[1, 2, 3, 4, 5]);
    drop(small_vec);

    let vec = vec![1, 2, 3, 4, 5];
    let small_vec: SmallVec<[u8; 1]> = SmallVec::from_vec(vec);
    assert_eq!(&*small_vec, &[1, 2, 3, 4, 5]);
    drop(small_vec);
}

#[test]
fn test_retain() {
    // Test inline data storage
    let mut sv: SmallVec<[i32; 5]> = SmallVec::from_slice(&[1, 2, 3, 3, 4]);
    sv.retain(|&mut i| i != 3);
    assert_eq!(sv.pop(), Some(4));
    assert_eq!(sv.pop(), Some(2));
    assert_eq!(sv.pop(), Some(1));
    assert_eq!(sv.pop(), None);

    // Test spilled data storage
    let mut sv: SmallVec<[i32; 3]> = SmallVec::from_slice(&[1, 2, 3, 3, 4]);
    sv.retain(|&mut i| i != 3);
    assert_eq!(sv.pop(), Some(4));
    assert_eq!(sv.pop(), Some(2));
    assert_eq!(sv.pop(), Some(1));
    assert_eq!(sv.pop(), None);

    // Test that drop implementations are called for inline.
    let one = Rc::new(1);
    let mut sv: SmallVec<[Rc<i32>; 3]> = SmallVec::new();
    sv.push(Rc::clone(&one));
    assert_eq!(Rc::strong_count(&one), 2);
    sv.retain(|_| false);
    assert_eq!(Rc::strong_count(&one), 1);

    // Test that drop implementations are called for spilled data.
    let mut sv: SmallVec<[Rc<i32>; 1]> = SmallVec::new();
    sv.push(Rc::clone(&one));
    sv.push(Rc::new(2));
    assert_eq!(Rc::strong_count(&one), 2);
    sv.retain(|_| false);
    assert_eq!(Rc::strong_count(&one), 1);
}

#[test]
fn test_dedup() {
    let mut dupes: SmallVec<[i32; 5]> = SmallVec::from_slice(&[1, 1, 2, 3, 3]);
    dupes.dedup();
    assert_eq!(&*dupes, &[1, 2, 3]);

    let mut empty: SmallVec<[i32; 5]> = SmallVec::new();
    empty.dedup();
    assert!(empty.is_empty());

    let mut all_ones: SmallVec<[i32; 5]> = SmallVec::from_slice(&[1, 1, 1, 1, 1]);
    all_ones.dedup();
    assert_eq!(all_ones.len(), 1);

    let mut no_dupes: SmallVec<[i32; 5]> = SmallVec::from_slice(&[1, 2, 3, 4, 5]);
    no_dupes.dedup();
    assert_eq!(no_dupes.len(), 5);
}

#[test]
fn test_resize() {
    let mut v: SmallVec<[i32; 8]> = SmallVec::new();
    v.push(1);
    v.resize(5, 0);
    assert_eq!(v[..], [1, 0, 0, 0, 0][..]);

    v.resize(2, -1);
    assert_eq!(v[..], [1, 0][..]);
}

#[cfg(feature = "write")]
#[test]
fn test_write() {
    use std::io::Write;

    let data = [1, 2, 3, 4, 5];

    let mut small_vec: SmallVec<[u8; 2]> = SmallVec::new();
    let len = small_vec.write(&data[..]).unwrap();
    assert_eq!(len, 5);
    assert_eq!(small_vec.as_ref(), data.as_ref());

    let mut small_vec: SmallVec<[u8; 2]> = SmallVec::new();
    small_vec.write_all(&data[..]).unwrap();
    assert_eq!(small_vec.as_ref(), data.as_ref());
}

#[cfg(feature = "serde")]
#[test]
fn test_serde() {
    use bincode1::{config, deserialize};
    let mut small_vec: SmallVec<[i32; 2]> = SmallVec::new();
    small_vec.push(1);
    let encoded = config().limit(100).serialize(&small_vec).unwrap();
    let decoded: SmallVec<[i32; 2]> = deserialize(&encoded).unwrap();
    assert_eq!(small_vec, decoded);
    small_vec.push(2);
    // Spill the vec
    small_vec.push(3);
    small_vec.push(4);
    // Check again after spilling.
    let encoded = config().limit(100).serialize(&small_vec).unwrap();
    let decoded: SmallVec<[i32; 2]> = deserialize(&encoded).unwrap();
    assert_eq!(small_vec, decoded);
}

#[test]
fn grow_to_shrink() {
    let mut v: SmallVec<[u8; 2]> = SmallVec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    assert!(v.spilled());
    v.clear();
    // Shrink to inline.
    v.grow(2);
    assert!(!v.spilled());
    assert_eq!(v.capacity(), 2);
    assert_eq!(v.len(), 0);
    v.push(4);
    assert_eq!(v[..], [4]);
}

#[test]
fn resumable_extend() {
    let s = "a b c";
    // This iterator yields: (Some('a'), None, Some('b'), None, Some('c')), None
    let it = s
        .chars()
        .scan(0, |_, ch| if ch.is_whitespace() { None } else { Some(ch) });
    let mut v: SmallVec<[char; 4]> = SmallVec::new();
    v.extend(it);
    assert_eq!(v[..], ['a']);
}

// #139
#[test]
fn uninhabited() {
    enum Void {}
    let _sv = SmallVec::<[Void; 8]>::new();
}

#[test]
fn grow_spilled_same_size() {
    let mut v: SmallVec<[u8; 2]> = SmallVec::new();
    v.push(0);
    v.push(1);
    v.push(2);
    assert!(v.spilled());
    assert_eq!(v.capacity(), 4);
    // grow with the same capacity
    v.grow(4);
    assert_eq!(v.capacity(), 4);
    assert_eq!(v[..], [0, 1, 2]);
}

#[cfg(feature = "const_generics")]
#[test]
fn const_generics() {
    let _v = SmallVec::<[i32; 987]>::default();
}

#[cfg(feature = "const_new")]
#[test]
fn const_new() {
    let v = const_new_inner();
    assert_eq!(v.capacity(), 4);
    assert_eq!(v.len(), 0);
    let v = const_new_inline_sized();
    assert_eq!(v.capacity(), 4);
    assert_eq!(v.len(), 4);
    assert_eq!(v[0], 1);
    let v = const_new_inline_args();
    assert_eq!(v.capacity(), 2);
    assert_eq!(v.len(), 2);
    assert_eq!(v[0], 1);
    assert_eq!(v[1], 4);
    let v = const_new_with_len();
    assert_eq!(v.capacity(), 4);
    assert_eq!(v.len(), 3);
    assert_eq!(v[0], 2);
    assert_eq!(v[1], 5);
    assert_eq!(v[2], 7);
}
#[cfg(feature = "const_new")]
const fn const_new_inner() -> SmallVec<[i32; 4]> {
    SmallVec::<[i32; 4]>::new_const()
}
#[cfg(feature = "const_new")]
const fn const_new_inline_sized() -> SmallVec<[i32; 4]> {
    crate::smallvec_inline![1; 4]
}
#[cfg(feature = "const_new")]
const fn const_new_inline_args() -> SmallVec<[i32; 2]> {
    crate::smallvec_inline![1, 4]
}
#[cfg(feature = "const_new")]
const fn const_new_with_len() -> SmallVec<[i32; 4]> {
    unsafe {
        SmallVec::<[i32; 4]>::from_const_with_len_unchecked([2, 5, 7, 0], 3)
    }
}

#[test]
fn empty_macro() {
    let _v: SmallVec<[u8; 1]> = smallvec![];
}

#[test]
fn zero_size_items() {
    SmallVec::<[(); 0]>::new().push(());
}

#[test]
fn test_insert_many_overflow() {
    let mut v: SmallVec<[u8; 1]> = SmallVec::new();
    v.push(123);

    // Prepare an iterator with small lower bound
    let iter = (0u8..5).filter(|n| n % 2 == 0);
    assert_eq!(iter.size_hint().0, 0);

    v.insert_many(0, iter);
    assert_eq!(&*v, &[0, 2, 4, 123]);
}

#[test]
fn test_clone_from() {
    let mut a: SmallVec<[u8; 2]> = SmallVec::new();
    a.push(1);
    a.push(2);
    a.push(3);

    let mut b: SmallVec<[u8; 2]> = SmallVec::new();
    b.push(10);

    let mut c: SmallVec<[u8; 2]> = SmallVec::new();
    c.push(20);
    c.push(21);
    c.push(22);

    a.clone_from(&b);
    assert_eq!(&*a, &[10]);

    b.clone_from(&c);
    assert_eq!(&*b, &[20, 21, 22]);
}

#[test]
fn test_size() {
    use core::mem::size_of;
    const PTR_SIZE: usize = size_of::<usize>();
    #[cfg(feature = "union")]
    {
        assert_eq!(3 * PTR_SIZE, size_of::<SmallVec<[u8; 0]>>());
        assert_eq!(3 * PTR_SIZE, size_of::<SmallVec<[u8; 1]>>());
        assert_eq!(3 * PTR_SIZE, size_of::<SmallVec<[u8; PTR_SIZE]>>());
        assert_eq!(3 * PTR_SIZE, size_of::<SmallVec<[u8; PTR_SIZE + 1]>>());
        assert_eq!(3 * PTR_SIZE, size_of::<SmallVec<[u8; 2 * PTR_SIZE]>>());
        assert_eq!(4 * PTR_SIZE, size_of::<SmallVec<[u8; 2 * PTR_SIZE + 1]>>());
    }
    #[cfg(not(feature = "union"))]
    {
        assert_eq!(3 * PTR_SIZE, size_of::<SmallVec<[u8; 0]>>());
        assert_eq!(3 * PTR_SIZE, size_of::<SmallVec<[u8; 1]>>());
        assert_eq!(3 * PTR_SIZE, size_of::<SmallVec<[u8; PTR_SIZE]>>());
        assert_eq!(4 * PTR_SIZE, size_of::<SmallVec<[u8; PTR_SIZE + 1]>>());
    }
}

#[cfg(feature = "drain_filter")]
#[test]
fn drain_filter() {
    let mut a: SmallVec<[u8; 2]> = smallvec![1u8, 2, 3, 4, 5, 6, 7, 8];

    let b: SmallVec<[u8; 2]> = a.drain_filter(|x| *x % 3 == 0).collect();

    assert_eq!(a, SmallVec::<[u8; 2]>::from_slice(&[1u8, 2, 4, 5, 7, 8]));
    assert_eq!(b, SmallVec::<[u8; 2]>::from_slice(&[3u8, 6]));
}

#[cfg(feature = "drain_keep_rest")]
#[test]
fn drain_keep_rest() {
    let mut a: SmallVec<[i32; 3]> = smallvec![1i32, 2, 3, 4, 5, 6, 7, 8];
    let mut df = a.drain_filter(|x| *x % 2 == 0);

    assert_eq!(df.next().unwrap(), 2);
    assert_eq!(df.next().unwrap(), 4);

    df.keep_rest();

    assert_eq!(a, SmallVec::<[i32; 3]>::from_slice(&[1i32, 3, 5, 6, 7, 8]));
}

/// This assortment of tests, in combination with miri, verifies we handle UB on fishy arguments
/// given to SmallVec. Draining and extending the allocation are fairly well-tested earlier, but
/// `smallvec.insert(usize::MAX, val)` once slipped by!
///
/// All code that indexes into SmallVecs should be tested with such "trivially wrong" args.
#[test]
fn max_dont_panic() {
    let mut sv: SmallVec<[i32; 2]> = smallvec![0];
    let _ = sv.get(usize::MAX);
    sv.truncate(usize::MAX);
}

#[test]
#[should_panic]
fn max_remove() {
    let mut sv: SmallVec<[i32; 2]> = smallvec![0];
    sv.remove(usize::MAX);
}

#[test]
#[should_panic]
fn max_swap_remove() {
    let mut sv: SmallVec<[i32; 2]> = smallvec![0];
    sv.swap_remove(usize::MAX);
}

#[test]
#[should_panic]
fn test_insert_out_of_bounds() {
    let mut v: SmallVec<[i32; 4]> = SmallVec::new();
    v.insert(10, 6);
}

#[cfg(feature = "impl_bincode")]
#[test]
fn test_bincode() {
    let config = bincode::config::standard();
    let mut small_vec: SmallVec<[i32; 2]> = SmallVec::new();
    let mut buffer = [0u8; 128];
    small_vec.push(1);
    let bytes_written = bincode::encode_into_slice(&small_vec, &mut buffer, config).unwrap();
    let (decoded, bytes_read) =
        bincode::decode_from_slice::<SmallVec<[i32; 2]>, _>(&buffer, config).unwrap();
    assert_eq!(bytes_written, bytes_read);
    assert_eq!(small_vec, decoded);
    let (decoded, bytes_read) =
        bincode::borrow_decode_from_slice::<SmallVec<[i32; 2]>, _>(&buffer, config).unwrap();
    assert_eq!(bytes_written, bytes_read);
    assert_eq!(small_vec, decoded);
    // Spill the vec
    small_vec.push(2);
    small_vec.push(3);
    small_vec.push(4);
    // Check again after spilling.
    let bytes_written = bincode::encode_into_slice(&small_vec, &mut buffer, config).unwrap();
    let (decoded, bytes_read) =
        bincode::decode_from_slice::<SmallVec<[i32; 2]>, _>(&buffer, config).unwrap();
    assert_eq!(bytes_written, bytes_read);
    assert_eq!(small_vec, decoded);
    let (decoded, bytes_read) =
        bincode::borrow_decode_from_slice::<SmallVec<[i32; 2]>, _>(&buffer, config).unwrap();
    assert_eq!(bytes_written, bytes_read);
    assert_eq!(small_vec, decoded);
}

#[cfg(feature = "impl_bincode")]
#[test]
fn test_bincode_u8() {
    let config = bincode::config::standard();
    let mut small_vec: SmallVec<[u8; 16]> = SmallVec::new();
    let mut buffer = [0u8; 128];
    small_vec.extend_from_slice(b"testing test");
    let bytes_written = bincode::encode_into_slice(&small_vec, &mut buffer, config).unwrap();
    let (decoded, bytes_read) =
        bincode::decode_from_slice::<SmallVec<[u8; 16]>, _>(&buffer, config).unwrap();
    assert_eq!(bytes_written, bytes_read);
    assert_eq!(small_vec, decoded);
    let (decoded, bytes_read) =
        bincode::borrow_decode_from_slice::<SmallVec<[u8; 16]>, _>(&buffer, config).unwrap();
    assert_eq!(bytes_written, bytes_read);
    assert_eq!(small_vec, decoded);
    // Spill the vec
    small_vec.extend_from_slice(b"some more testing");
    // Check again after spilling.
    let bytes_written = bincode::encode_into_slice(&small_vec, &mut buffer, config).unwrap();
    let (decoded, bytes_read) =
        bincode::decode_from_slice::<SmallVec<[u8; 16]>, _>(&buffer, config).unwrap();
    assert_eq!(bytes_written, bytes_read);
    assert_eq!(small_vec, decoded);
    let (decoded, bytes_read) =
        bincode::borrow_decode_from_slice::<SmallVec<[u8; 16]>, _>(&buffer, config).unwrap();
    assert_eq!(bytes_written, bytes_read);
    assert_eq!(small_vec, decoded);
}
