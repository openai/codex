#![warn(rust_2018_idioms)]

use slab::*;

use std::{
    iter::FromIterator,
    panic::{catch_unwind, resume_unwind, AssertUnwindSafe},
};

#[test]
fn insert_get_remove_one() {
    let mut slab = Slab::new();
    assert!(slab.is_empty());

    let key = slab.insert(10);

    assert_eq!(slab[key], 10);
    assert_eq!(slab.get(key), Some(&10));
    assert!(!slab.is_empty());
    assert!(slab.contains(key));

    assert_eq!(slab.remove(key), 10);
    assert!(!slab.contains(key));
    assert!(slab.get(key).is_none());
}

#[test]
fn insert_get_many() {
    let mut slab = Slab::with_capacity(10);

    for i in 0..10 {
        let key = slab.insert(i + 10);
        assert_eq!(slab[key], i + 10);
    }

    assert_eq!(slab.capacity(), 10);

    // Storing another one grows the slab
    let key = slab.insert(20);
    assert_eq!(slab[key], 20);

    // Capacity grows by 2x
    assert_eq!(slab.capacity(), 20);
}

#[test]
fn insert_get_remove_many() {
    let mut slab = Slab::with_capacity(10);
    let mut keys = vec![];

    for i in 0..10 {
        for j in 0..10 {
            let val = (i * 10) + j;

            let key = slab.insert(val);
            keys.push((key, val));
            assert_eq!(slab[key], val);
        }

        for (key, val) in keys.drain(..) {
            assert_eq!(val, slab.remove(key));
        }
    }

    assert_eq!(10, slab.capacity());
}

#[test]
fn insert_with_vacant_entry() {
    let mut slab = Slab::with_capacity(1);
    let key;

    {
        let entry = slab.vacant_entry();
        key = entry.key();
        entry.insert(123);
    }

    assert_eq!(123, slab[key]);
}

#[test]
fn get_vacant_entry_without_using() {
    let mut slab = Slab::<usize>::with_capacity(1);
    let key = slab.vacant_entry().key();
    assert_eq!(key, slab.vacant_entry().key());
}

#[test]
#[should_panic(expected = "invalid key")]
fn invalid_get_panics() {
    let slab = Slab::<usize>::with_capacity(1);
    let _ = &slab[0];
}

#[test]
#[should_panic(expected = "invalid key")]
fn invalid_get_mut_panics() {
    let mut slab = Slab::<usize>::new();
    let _ = &mut slab[0];
}

#[test]
#[should_panic(expected = "invalid key")]
fn double_remove_panics() {
    let mut slab = Slab::<usize>::with_capacity(1);
    let key = slab.insert(123);
    slab.remove(key);
    slab.remove(key);
}

#[test]
#[should_panic(expected = "invalid key")]
fn invalid_remove_panics() {
    let mut slab = Slab::<usize>::with_capacity(1);
    slab.remove(0);
}

#[test]
fn slab_get_mut() {
    let mut slab = Slab::new();
    let key = slab.insert(1);

    slab[key] = 2;
    assert_eq!(slab[key], 2);

    *slab.get_mut(key).unwrap() = 3;
    assert_eq!(slab[key], 3);
}

#[test]
fn key_of_tagged() {
    let mut slab = Slab::new();
    slab.insert(0);
    assert_eq!(slab.key_of(&slab[0]), 0);
}

#[test]
fn key_of_layout_optimizable() {
    // Entry<&str> doesn't need a discriminant tag because it can use the
    // nonzero-ness of ptr and store Vacant's next at the same offset as len
    let mut slab = Slab::new();
    slab.insert("foo");
    slab.insert("bar");
    let third = slab.insert("baz");
    slab.insert("quux");
    assert_eq!(slab.key_of(&slab[third]), third);
}

#[test]
fn key_of_zst() {
    let mut slab = Slab::new();
    slab.insert(());
    let second = slab.insert(());
    slab.insert(());
    assert_eq!(slab.key_of(&slab[second]), second);
}

#[test]
fn reserve_does_not_allocate_if_available() {
    let mut slab = Slab::with_capacity(10);
    let mut keys = vec![];

    for i in 0..6 {
        keys.push(slab.insert(i));
    }

    for key in 0..4 {
        slab.remove(key);
    }

    assert!(slab.capacity() - slab.len() == 8);

    slab.reserve(8);
    assert_eq!(10, slab.capacity());
}

#[test]
fn reserve_exact_does_not_allocate_if_available() {
    let mut slab = Slab::with_capacity(10);
    let mut keys = vec![];

    for i in 0..6 {
        keys.push(slab.insert(i));
    }

    for key in 0..4 {
        slab.remove(key);
    }

    assert!(slab.capacity() - slab.len() == 8);

    slab.reserve_exact(8);
    assert_eq!(10, slab.capacity());
}

#[test]
#[should_panic(expected = "capacity overflow")]
fn reserve_does_panic_with_capacity_overflow() {
    let mut slab = Slab::with_capacity(10);
    slab.insert(true);
    slab.reserve(isize::MAX as usize);
}

#[test]
#[should_panic(expected = "capacity overflow")]
fn reserve_does_panic_with_capacity_overflow_bytes() {
    let mut slab = Slab::with_capacity(10);
    slab.insert(1u16);
    slab.reserve((isize::MAX as usize) / 2);
}

#[test]
#[should_panic(expected = "capacity overflow")]
fn reserve_exact_does_panic_with_capacity_overflow() {
    let mut slab = Slab::with_capacity(10);
    slab.insert(true);
    slab.reserve_exact(isize::MAX as usize);
}

#[test]
fn retain() {
    let mut slab = Slab::with_capacity(2);

    let key1 = slab.insert(0);
    let key2 = slab.insert(1);

    slab.retain(|key, x| {
        assert_eq!(key, *x);
        *x % 2 == 0
    });

    assert_eq!(slab.len(), 1);
    assert_eq!(slab[key1], 0);
    assert!(!slab.contains(key2));

    // Ensure consistency is retained
    let key = slab.insert(123);
    assert_eq!(key, key2);

    assert_eq!(2, slab.len());
    assert_eq!(2, slab.capacity());

    // Inserting another element grows
    let key = slab.insert(345);
    assert_eq!(key, 2);

    assert_eq!(4, slab.capacity());
}

#[test]
fn into_iter() {
    let mut slab = Slab::new();

    for i in 0..8 {
        slab.insert(i);
    }
    slab.remove(0);
    slab.remove(4);
    slab.remove(5);
    slab.remove(7);

    let vals: Vec<_> = slab
        .into_iter()
        .inspect(|&(key, val)| assert_eq!(key, val))
        .map(|(_, val)| val)
        .collect();
    assert_eq!(vals, vec![1, 2, 3, 6]);
}

#[test]
fn into_iter_rev() {
    let mut slab = Slab::new();

    for i in 0..4 {
        slab.insert(i);
    }

    let mut iter = slab.into_iter();
    assert_eq!(iter.next_back(), Some((3, 3)));
    assert_eq!(iter.next_back(), Some((2, 2)));
    assert_eq!(iter.next(), Some((0, 0)));
    assert_eq!(iter.next_back(), Some((1, 1)));
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
}

#[test]
fn iter() {
    let mut slab = Slab::new();

    for i in 0..4 {
        slab.insert(i);
    }

    let vals: Vec<_> = slab
        .iter()
        .enumerate()
        .map(|(i, (key, val))| {
            assert_eq!(i, key);
            *val
        })
        .collect();
    assert_eq!(vals, vec![0, 1, 2, 3]);

    slab.remove(1);

    let vals: Vec<_> = slab.iter().map(|(_, r)| *r).collect();
    assert_eq!(vals, vec![0, 2, 3]);
}

#[test]
fn iter_rev() {
    let mut slab = Slab::new();

    for i in 0..4 {
        slab.insert(i);
    }
    slab.remove(0);

    let vals = slab.iter().rev().collect::<Vec<_>>();
    assert_eq!(vals, vec![(3, &3), (2, &2), (1, &1)]);
}

#[test]
fn iter_mut() {
    let mut slab = Slab::new();

    for i in 0..4 {
        slab.insert(i);
    }

    for (i, (key, e)) in slab.iter_mut().enumerate() {
        assert_eq!(i, key);
        *e += 1;
    }

    let vals: Vec<_> = slab.iter().map(|(_, r)| *r).collect();
    assert_eq!(vals, vec![1, 2, 3, 4]);

    slab.remove(2);

    for (_, e) in slab.iter_mut() {
        *e += 1;
    }

    let vals: Vec<_> = slab.iter().map(|(_, r)| *r).collect();
    assert_eq!(vals, vec![2, 3, 5]);
}

#[test]
fn iter_mut_rev() {
    let mut slab = Slab::new();

    for i in 0..4 {
        slab.insert(i);
    }
    slab.remove(2);

    {
        let mut iter = slab.iter_mut();
        assert_eq!(iter.next(), Some((0, &mut 0)));
        let mut prev_key = !0;
        for (key, e) in iter.rev() {
            *e += 10;
            assert!(prev_key > key);
            prev_key = key;
        }
    }

    assert_eq!(slab[0], 0);
    assert_eq!(slab[1], 11);
    assert_eq!(slab[3], 13);
    assert!(!slab.contains(2));
}

#[test]
fn from_iterator_sorted() {
    let mut slab = (0..5).map(|i| (i, i)).collect::<Slab<_>>();
    assert_eq!(slab.len(), 5);
    assert_eq!(slab[0], 0);
    assert_eq!(slab[2], 2);
    assert_eq!(slab[4], 4);
    assert_eq!(slab.vacant_entry().key(), 5);
}

#[test]
fn from_iterator_new_in_order() {
    // all new keys come in increasing order, but existing keys are overwritten
    let mut slab = [(0, 'a'), (1, 'a'), (1, 'b'), (0, 'b'), (9, 'a'), (0, 'c')]
        .iter()
        .cloned()
        .collect::<Slab<_>>();
    assert_eq!(slab.len(), 3);
    assert_eq!(slab[0], 'c');
    assert_eq!(slab[1], 'b');
    assert_eq!(slab[9], 'a');
    assert_eq!(slab.get(5), None);
    assert_eq!(slab.vacant_entry().key(), 8);
}

#[test]
fn from_iterator_unordered() {
    let mut slab = vec![(1, "one"), (50, "fifty"), (3, "three"), (20, "twenty")]
        .into_iter()
        .collect::<Slab<_>>();
    assert_eq!(slab.len(), 4);
    assert_eq!(slab.vacant_entry().key(), 0);
    let mut iter = slab.iter();
    assert_eq!(iter.next(), Some((1, &"one")));
    assert_eq!(iter.next(), Some((3, &"three")));
    assert_eq!(iter.next(), Some((20, &"twenty")));
    assert_eq!(iter.next(), Some((50, &"fifty")));
    assert_eq!(iter.next(), None);
}

// https://github.com/tokio-rs/slab/issues/100
#[test]
fn from_iterator_issue_100() {
    let mut slab: slab::Slab<()> = vec![(1, ())].into_iter().collect();
    assert_eq!(slab.len(), 1);
    assert_eq!(slab.insert(()), 0);
    assert_eq!(slab.insert(()), 2);
    assert_eq!(slab.insert(()), 3);

    let mut slab: slab::Slab<()> = vec![(1, ()), (2, ())].into_iter().collect();
    assert_eq!(slab.len(), 2);
    assert_eq!(slab.insert(()), 0);
    assert_eq!(slab.insert(()), 3);
    assert_eq!(slab.insert(()), 4);

    let mut slab: slab::Slab<()> = vec![(1, ()), (3, ())].into_iter().collect();
    assert_eq!(slab.len(), 2);
    assert_eq!(slab.insert(()), 2);
    assert_eq!(slab.insert(()), 0);
    assert_eq!(slab.insert(()), 4);

    let mut slab: slab::Slab<()> = vec![(0, ()), (2, ()), (3, ()), (5, ())]
        .into_iter()
        .collect();
    assert_eq!(slab.len(), 4);
    assert_eq!(slab.insert(()), 4);
    assert_eq!(slab.insert(()), 1);
    assert_eq!(slab.insert(()), 6);
}

#[test]
fn clear() {
    let mut slab = Slab::new();

    for i in 0..4 {
        slab.insert(i);
    }

    // clear full
    slab.clear();
    assert!(slab.is_empty());

    assert_eq!(0, slab.len());
    assert_eq!(4, slab.capacity());

    for i in 0..2 {
        slab.insert(i);
    }

    let vals: Vec<_> = slab.iter().map(|(_, r)| *r).collect();
    assert_eq!(vals, vec![0, 1]);

    // clear half-filled
    slab.clear();
    assert!(slab.is_empty());
}

#[test]
fn shrink_to_fit_empty() {
    let mut slab = Slab::<bool>::with_capacity(20);
    slab.shrink_to_fit();
    assert_eq!(slab.capacity(), 0);
}

#[test]
fn shrink_to_fit_no_vacant() {
    let mut slab = Slab::with_capacity(20);
    slab.insert(String::new());
    slab.shrink_to_fit();
    assert!(slab.capacity() < 10);
}

#[test]
fn shrink_to_fit_doesnt_move() {
    let mut slab = Slab::with_capacity(8);
    slab.insert("foo");
    let bar = slab.insert("bar");
    slab.insert("baz");
    let quux = slab.insert("quux");
    slab.remove(quux);
    slab.remove(bar);
    slab.shrink_to_fit();
    assert_eq!(slab.len(), 2);
    assert!(slab.capacity() >= 3);
    assert_eq!(slab.get(0), Some(&"foo"));
    assert_eq!(slab.get(2), Some(&"baz"));
    assert_eq!(slab.vacant_entry().key(), bar);
}

#[test]
fn shrink_to_fit_doesnt_recreate_list_when_nothing_can_be_done() {
    let mut slab = Slab::with_capacity(16);
    for i in 0..4 {
        slab.insert(Box::new(i));
    }
    slab.remove(0);
    slab.remove(2);
    slab.remove(1);
    assert_eq!(slab.vacant_entry().key(), 1);
    slab.shrink_to_fit();
    assert_eq!(slab.len(), 1);
    assert!(slab.capacity() >= 4);
    assert_eq!(slab.vacant_entry().key(), 1);
}

#[test]
fn compact_empty() {
    let mut slab = Slab::new();
    slab.compact(|_, _, _| panic!());
    assert_eq!(slab.len(), 0);
    assert_eq!(slab.capacity(), 0);
    slab.reserve(20);
    slab.compact(|_, _, _| panic!());
    assert_eq!(slab.len(), 0);
    assert_eq!(slab.capacity(), 0);
    slab.insert(0);
    slab.insert(1);
    slab.insert(2);
    slab.remove(1);
    slab.remove(2);
    slab.remove(0);
    slab.compact(|_, _, _| panic!());
    assert_eq!(slab.len(), 0);
    assert_eq!(slab.capacity(), 0);
}

#[test]
fn compact_no_moves_needed() {
    let mut slab = Slab::new();
    for i in 0..10 {
        slab.insert(i);
    }
    slab.remove(8);
    slab.remove(9);
    slab.remove(6);
    slab.remove(7);
    slab.compact(|_, _, _| panic!());
    assert_eq!(slab.len(), 6);
    for ((index, &value), want) in slab.iter().zip(0..6) {
        assert!(index == value);
        assert_eq!(index, want);
    }
    assert!(slab.capacity() >= 6 && slab.capacity() < 10);
}

#[test]
fn compact_moves_successfully() {
    let mut slab = Slab::with_capacity(20);
    for i in 0..10 {
        slab.insert(i);
    }
    for &i in &[0, 5, 9, 6, 3] {
        slab.remove(i);
    }
    let mut moved = 0;
    slab.compact(|&mut v, from, to| {
        assert!(from > to);
        assert!(from >= 5);
        assert!(to < 5);
        assert_eq!(from, v);
        moved += 1;
        true
    });
    assert_eq!(slab.len(), 5);
    assert_eq!(moved, 2);
    assert_eq!(slab.vacant_entry().key(), 5);
    assert!(slab.capacity() >= 5 && slab.capacity() < 20);
    let mut iter = slab.iter();
    assert_eq!(iter.next(), Some((0, &8)));
    assert_eq!(iter.next(), Some((1, &1)));
    assert_eq!(iter.next(), Some((2, &2)));
    assert_eq!(iter.next(), Some((3, &7)));
    assert_eq!(iter.next(), Some((4, &4)));
    assert_eq!(iter.next(), None);
}

#[test]
fn compact_doesnt_move_if_closure_errors() {
    let mut slab = Slab::with_capacity(20);
    for i in 0..10 {
        slab.insert(i);
    }
    for &i in &[9, 3, 1, 4, 0] {
        slab.remove(i);
    }
    slab.compact(|&mut v, from, to| {
        assert!(from > to);
        assert_eq!(from, v);
        v != 6
    });
    assert_eq!(slab.len(), 5);
    assert!(slab.capacity() >= 7 && slab.capacity() < 20);
    assert_eq!(slab.vacant_entry().key(), 3);
    let mut iter = slab.iter();
    assert_eq!(iter.next(), Some((0, &8)));
    assert_eq!(iter.next(), Some((1, &7)));
    assert_eq!(iter.next(), Some((2, &2)));
    assert_eq!(iter.next(), Some((5, &5)));
    assert_eq!(iter.next(), Some((6, &6)));
    assert_eq!(iter.next(), None);
}

#[test]
fn compact_handles_closure_panic() {
    let mut slab = Slab::new();
    for i in 0..10 {
        slab.insert(i);
    }
    for i in 1..6 {
        slab.remove(i);
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        slab.compact(|&mut v, from, to| {
            assert!(from > to);
            assert_eq!(from, v);
            if v == 7 {
                panic!("test");
            }
            true
        })
    }));
    match result {
        Err(ref payload) if payload.downcast_ref() == Some(&"test") => {}
        Err(bug) => resume_unwind(bug),
        Ok(()) => unreachable!(),
    }
    assert_eq!(slab.len(), 5 - 1);
    assert_eq!(slab.vacant_entry().key(), 3);
    let mut iter = slab.iter();
    assert_eq!(iter.next(), Some((0, &0)));
    assert_eq!(iter.next(), Some((1, &9)));
    assert_eq!(iter.next(), Some((2, &8)));
    assert_eq!(iter.next(), Some((6, &6)));
    assert_eq!(iter.next(), None);
}

#[test]
fn fully_consumed_drain() {
    let mut slab = Slab::new();

    for i in 0..3 {
        slab.insert(i);
    }

    {
        let mut drain = slab.drain();
        assert_eq!(Some(0), drain.next());
        assert_eq!(Some(1), drain.next());
        assert_eq!(Some(2), drain.next());
        assert_eq!(None, drain.next());
    }

    assert!(slab.is_empty());
}

#[test]
fn partially_consumed_drain() {
    let mut slab = Slab::new();

    for i in 0..3 {
        slab.insert(i);
    }

    {
        let mut drain = slab.drain();
        assert_eq!(Some(0), drain.next());
    }

    assert!(slab.is_empty())
}

#[test]
fn drain_rev() {
    let mut slab = Slab::new();
    for i in 0..10 {
        slab.insert(i);
    }
    slab.remove(9);

    let vals: Vec<u64> = slab.drain().rev().collect();
    assert_eq!(vals, (0..9).rev().collect::<Vec<u64>>());
}

#[test]
fn try_remove() {
    let mut slab = Slab::new();

    let key = slab.insert(1);

    assert_eq!(slab.try_remove(key), Some(1));
    assert_eq!(slab.try_remove(key), None);
    assert_eq!(slab.get(key), None);
}

#[test]
fn const_new() {
    static _SLAB: Slab<()> = Slab::new();
}

#[test]
fn clone_from() {
    let mut slab1 = Slab::new();
    let mut slab2 = Slab::new();
    for i in 0..5 {
        slab1.insert(i);
        slab2.insert(2 * i);
        slab2.insert(2 * i + 1);
    }
    slab1.remove(1);
    slab1.remove(3);
    slab2.clone_from(&slab1);

    let mut iter2 = slab2.iter();
    assert_eq!(iter2.next(), Some((0, &0)));
    assert_eq!(iter2.next(), Some((2, &2)));
    assert_eq!(iter2.next(), Some((4, &4)));
    assert_eq!(iter2.next(), None);
    assert!(slab2.capacity() >= 10);
}

#[test]
fn get_disjoint_mut() {
    let mut slab = Slab::from_iter((0..5).enumerate());
    slab.remove(1);
    slab.remove(3);

    assert_eq!(slab.get_disjoint_mut([]), Ok([]));

    assert_eq!(
        slab.get_disjoint_mut([4, 2, 0]).unwrap().map(|x| *x),
        [4, 2, 0]
    );

    assert_eq!(
        slab.get_disjoint_mut([42, 2, 1, 2]),
        Err(GetDisjointMutError::OverlappingIndices)
    );

    assert_eq!(
        slab.get_disjoint_mut([1, 5]),
        Err(GetDisjointMutError::IndexVacant)
    );

    assert_eq!(
        slab.get_disjoint_mut([5, 1]),
        Err(GetDisjointMutError::IndexOutOfBounds)
    );

    let [a, b] = slab.get_disjoint_mut([0, 4]).unwrap();
    (*a, *b) = (*b, *a);
    assert_eq!(slab[0], 4);
    assert_eq!(slab[4], 0);
}

#[test]
fn get_disjoint_mut_out_of_bounds_index_error() {
    let mut slab: Slab<i32> = Slab::with_capacity(10);
    slab.insert(1);
    slab.insert(2);

    // Index 0 and 1 are valid, but index 5 is out of bounds (beyond len)
    assert_eq!(
        slab.get_disjoint_mut([0, 1, 5]),
        Err(GetDisjointMutError::IndexOutOfBounds)
    );
}
