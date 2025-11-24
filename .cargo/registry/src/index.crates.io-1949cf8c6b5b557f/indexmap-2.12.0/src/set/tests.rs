use super::*;
use std::string::String;

#[test]
fn it_works() {
    let mut set = IndexSet::new();
    assert_eq!(set.is_empty(), true);
    set.insert(1);
    set.insert(1);
    assert_eq!(set.len(), 1);
    assert!(set.get(&1).is_some());
    assert_eq!(set.is_empty(), false);
}

#[test]
fn new() {
    let set = IndexSet::<String>::new();
    println!("{:?}", set);
    assert_eq!(set.capacity(), 0);
    assert_eq!(set.len(), 0);
    assert_eq!(set.is_empty(), true);
}

#[test]
fn insert() {
    let insert = [0, 4, 2, 12, 8, 7, 11, 5];
    let not_present = [1, 3, 6, 9, 10];
    let mut set = IndexSet::with_capacity(insert.len());

    for (i, &elt) in insert.iter().enumerate() {
        assert_eq!(set.len(), i);
        set.insert(elt);
        assert_eq!(set.len(), i + 1);
        assert_eq!(set.get(&elt), Some(&elt));
    }
    println!("{:?}", set);

    for &elt in &not_present {
        assert!(set.get(&elt).is_none());
    }
}

#[test]
fn insert_full() {
    let insert = vec![9, 2, 7, 1, 4, 6, 13];
    let present = vec![1, 6, 2];
    let mut set = IndexSet::with_capacity(insert.len());

    for (i, &elt) in insert.iter().enumerate() {
        assert_eq!(set.len(), i);
        let (index, success) = set.insert_full(elt);
        assert!(success);
        assert_eq!(Some(index), set.get_full(&elt).map(|x| x.0));
        assert_eq!(set.len(), i + 1);
    }

    let len = set.len();
    for &elt in &present {
        let (index, success) = set.insert_full(elt);
        assert!(!success);
        assert_eq!(Some(index), set.get_full(&elt).map(|x| x.0));
        assert_eq!(set.len(), len);
    }
}

#[test]
fn insert_2() {
    let mut set = IndexSet::with_capacity(16);

    let mut values = vec![];
    values.extend(0..16);
    values.extend(if cfg!(miri) { 32..64 } else { 128..267 });

    for &i in &values {
        let old_set = set.clone();
        set.insert(i);
        for value in old_set.iter() {
            if set.get(value).is_none() {
                println!("old_set: {:?}", old_set);
                println!("set: {:?}", set);
                panic!("did not find {} in set", value);
            }
        }
    }

    for &i in &values {
        assert!(set.get(&i).is_some(), "did not find {}", i);
    }
}

#[test]
fn insert_dup() {
    let mut elements = vec![0, 2, 4, 6, 8];
    let mut set: IndexSet<u8> = elements.drain(..).collect();
    {
        let (i, v) = set.get_full(&0).unwrap();
        assert_eq!(set.len(), 5);
        assert_eq!(i, 0);
        assert_eq!(*v, 0);
    }
    {
        let inserted = set.insert(0);
        let (i, v) = set.get_full(&0).unwrap();
        assert_eq!(set.len(), 5);
        assert_eq!(inserted, false);
        assert_eq!(i, 0);
        assert_eq!(*v, 0);
    }
}

#[test]
fn insert_order() {
    let insert = [0, 4, 2, 12, 8, 7, 11, 5, 3, 17, 19, 22, 23];
    let mut set = IndexSet::new();

    for &elt in &insert {
        set.insert(elt);
    }

    assert_eq!(set.iter().count(), set.len());
    assert_eq!(set.iter().count(), insert.len());
    for (a, b) in insert.iter().zip(set.iter()) {
        assert_eq!(a, b);
    }
    for (i, v) in (0..insert.len()).zip(set.iter()) {
        assert_eq!(set.get_index(i).unwrap(), v);
    }
}

#[test]
fn shift_insert() {
    let insert = [0, 4, 2, 12, 8, 7, 11, 5, 3, 17, 19, 22, 23];
    let mut set = IndexSet::new();

    for &elt in &insert {
        set.shift_insert(0, elt);
    }

    assert_eq!(set.iter().count(), set.len());
    assert_eq!(set.iter().count(), insert.len());
    for (a, b) in insert.iter().rev().zip(set.iter()) {
        assert_eq!(a, b);
    }
    for (i, v) in (0..insert.len()).zip(set.iter()) {
        assert_eq!(set.get_index(i).unwrap(), v);
    }

    // "insert" that moves an existing entry
    set.shift_insert(0, insert[0]);
    assert_eq!(set.iter().count(), insert.len());
    assert_eq!(insert[0], set[0]);
    for (a, b) in insert[1..].iter().rev().zip(set.iter().skip(1)) {
        assert_eq!(a, b);
    }
}

#[test]
fn replace() {
    let replace = [0, 4, 2, 12, 8, 7, 11, 5];
    let not_present = [1, 3, 6, 9, 10];
    let mut set = IndexSet::with_capacity(replace.len());

    for (i, &elt) in replace.iter().enumerate() {
        assert_eq!(set.len(), i);
        set.replace(elt);
        assert_eq!(set.len(), i + 1);
        assert_eq!(set.get(&elt), Some(&elt));
    }
    println!("{:?}", set);

    for &elt in &not_present {
        assert!(set.get(&elt).is_none());
    }
}

#[test]
fn replace_full() {
    let replace = vec![9, 2, 7, 1, 4, 6, 13];
    let present = vec![1, 6, 2];
    let mut set = IndexSet::with_capacity(replace.len());

    for (i, &elt) in replace.iter().enumerate() {
        assert_eq!(set.len(), i);
        let (index, replaced) = set.replace_full(elt);
        assert!(replaced.is_none());
        assert_eq!(Some(index), set.get_full(&elt).map(|x| x.0));
        assert_eq!(set.len(), i + 1);
    }

    let len = set.len();
    for &elt in &present {
        let (index, replaced) = set.replace_full(elt);
        assert_eq!(Some(elt), replaced);
        assert_eq!(Some(index), set.get_full(&elt).map(|x| x.0));
        assert_eq!(set.len(), len);
    }
}

#[test]
fn replace_2() {
    let mut set = IndexSet::with_capacity(16);

    let mut values = vec![];
    values.extend(0..16);
    values.extend(if cfg!(miri) { 32..64 } else { 128..267 });

    for &i in &values {
        let old_set = set.clone();
        set.replace(i);
        for value in old_set.iter() {
            if set.get(value).is_none() {
                println!("old_set: {:?}", old_set);
                println!("set: {:?}", set);
                panic!("did not find {} in set", value);
            }
        }
    }

    for &i in &values {
        assert!(set.get(&i).is_some(), "did not find {}", i);
    }
}

#[test]
fn replace_dup() {
    let mut elements = vec![0, 2, 4, 6, 8];
    let mut set: IndexSet<u8> = elements.drain(..).collect();
    {
        let (i, v) = set.get_full(&0).unwrap();
        assert_eq!(set.len(), 5);
        assert_eq!(i, 0);
        assert_eq!(*v, 0);
    }
    {
        let replaced = set.replace(0);
        let (i, v) = set.get_full(&0).unwrap();
        assert_eq!(set.len(), 5);
        assert_eq!(replaced, Some(0));
        assert_eq!(i, 0);
        assert_eq!(*v, 0);
    }
}

#[test]
fn replace_order() {
    let replace = [0, 4, 2, 12, 8, 7, 11, 5, 3, 17, 19, 22, 23];
    let mut set = IndexSet::new();

    for &elt in &replace {
        set.replace(elt);
    }

    assert_eq!(set.iter().count(), set.len());
    assert_eq!(set.iter().count(), replace.len());
    for (a, b) in replace.iter().zip(set.iter()) {
        assert_eq!(a, b);
    }
    for (i, v) in (0..replace.len()).zip(set.iter()) {
        assert_eq!(set.get_index(i).unwrap(), v);
    }
}

#[test]
fn replace_change() {
    // Check pointers to make sure it really changes
    let mut set = indexset!(vec![42]);
    let old_ptr = set[0].as_ptr();
    let new = set[0].clone();
    let new_ptr = new.as_ptr();
    assert_ne!(old_ptr, new_ptr);
    let replaced = set.replace(new).unwrap();
    assert_eq!(replaced.as_ptr(), old_ptr);
}

#[test]
fn grow() {
    let insert = [0, 4, 2, 12, 8, 7, 11];
    let not_present = [1, 3, 6, 9, 10];
    let mut set = IndexSet::with_capacity(insert.len());

    for (i, &elt) in insert.iter().enumerate() {
        assert_eq!(set.len(), i);
        set.insert(elt);
        assert_eq!(set.len(), i + 1);
        assert_eq!(set.get(&elt), Some(&elt));
    }

    println!("{:?}", set);
    for &elt in &insert {
        set.insert(elt * 10);
    }
    for &elt in &insert {
        set.insert(elt * 100);
    }
    for (i, &elt) in insert.iter().cycle().enumerate().take(100) {
        set.insert(elt * 100 + i as i32);
    }
    println!("{:?}", set);
    for &elt in &not_present {
        assert!(set.get(&elt).is_none());
    }
}

#[test]
fn reserve() {
    let mut set = IndexSet::<usize>::new();
    assert_eq!(set.capacity(), 0);
    set.reserve(100);
    let capacity = set.capacity();
    assert!(capacity >= 100);
    for i in 0..capacity {
        assert_eq!(set.len(), i);
        set.insert(i);
        assert_eq!(set.len(), i + 1);
        assert_eq!(set.capacity(), capacity);
        assert_eq!(set.get(&i), Some(&i));
    }
    set.insert(capacity);
    assert_eq!(set.len(), capacity + 1);
    assert!(set.capacity() > capacity);
    assert_eq!(set.get(&capacity), Some(&capacity));
}

#[test]
fn try_reserve() {
    let mut set = IndexSet::<usize>::new();
    assert_eq!(set.capacity(), 0);
    assert_eq!(set.try_reserve(100), Ok(()));
    assert!(set.capacity() >= 100);
    assert!(set.try_reserve(usize::MAX).is_err());
}

#[test]
fn shrink_to_fit() {
    let mut set = IndexSet::<usize>::new();
    assert_eq!(set.capacity(), 0);
    for i in 0..100 {
        assert_eq!(set.len(), i);
        set.insert(i);
        assert_eq!(set.len(), i + 1);
        assert!(set.capacity() >= i + 1);
        assert_eq!(set.get(&i), Some(&i));
        set.shrink_to_fit();
        assert_eq!(set.len(), i + 1);
        assert_eq!(set.capacity(), i + 1);
        assert_eq!(set.get(&i), Some(&i));
    }
}

#[test]
fn remove() {
    let insert = [0, 4, 2, 12, 8, 7, 11, 5, 3, 17, 19, 22, 23];
    let mut set = IndexSet::new();

    for &elt in &insert {
        set.insert(elt);
    }

    assert_eq!(set.iter().count(), set.len());
    assert_eq!(set.iter().count(), insert.len());
    for (a, b) in insert.iter().zip(set.iter()) {
        assert_eq!(a, b);
    }

    let remove_fail = [99, 77];
    let remove = [4, 12, 8, 7];

    for &value in &remove_fail {
        assert!(set.swap_remove_full(&value).is_none());
    }
    println!("{:?}", set);
    for &value in &remove {
        //println!("{:?}", set);
        let index = set.get_full(&value).unwrap().0;
        assert_eq!(set.swap_remove_full(&value), Some((index, value)));
    }
    println!("{:?}", set);

    for value in &insert {
        assert_eq!(set.get(value).is_some(), !remove.contains(value));
    }
    assert_eq!(set.len(), insert.len() - remove.len());
    assert_eq!(set.iter().count(), insert.len() - remove.len());
}

#[test]
fn swap_remove_index() {
    let insert = [0, 4, 2, 12, 8, 7, 11, 5, 3, 17, 19, 22, 23];
    let mut set = IndexSet::new();

    for &elt in &insert {
        set.insert(elt);
    }

    let mut vector = insert.to_vec();
    let remove_sequence = &[3, 3, 10, 4, 5, 4, 3, 0, 1];

    // check that the same swap remove sequence on vec and set
    // have the same result.
    for &rm in remove_sequence {
        let out_vec = vector.swap_remove(rm);
        let out_set = set.swap_remove_index(rm).unwrap();
        assert_eq!(out_vec, out_set);
    }
    assert_eq!(vector.len(), set.len());
    for (a, b) in vector.iter().zip(set.iter()) {
        assert_eq!(a, b);
    }
}

#[test]
fn partial_eq_and_eq() {
    let mut set_a = IndexSet::new();
    set_a.insert(1);
    set_a.insert(2);
    let mut set_b = set_a.clone();
    assert_eq!(set_a, set_b);
    set_b.swap_remove(&1);
    assert_ne!(set_a, set_b);

    let set_c: IndexSet<_> = set_b.into_iter().collect();
    assert_ne!(set_a, set_c);
    assert_ne!(set_c, set_a);
}

#[test]
fn extend() {
    let mut set = IndexSet::new();
    set.extend(vec![&1, &2, &3, &4]);
    set.extend(vec![5, 6]);
    assert_eq!(set.into_iter().collect::<Vec<_>>(), vec![1, 2, 3, 4, 5, 6]);
}

#[test]
fn comparisons() {
    let set_a: IndexSet<_> = (0..3).collect();
    let set_b: IndexSet<_> = (3..6).collect();
    let set_c: IndexSet<_> = (0..6).collect();
    let set_d: IndexSet<_> = (3..9).collect();

    assert!(!set_a.is_disjoint(&set_a));
    assert!(set_a.is_subset(&set_a));
    assert!(set_a.is_superset(&set_a));

    assert!(set_a.is_disjoint(&set_b));
    assert!(set_b.is_disjoint(&set_a));
    assert!(!set_a.is_subset(&set_b));
    assert!(!set_b.is_subset(&set_a));
    assert!(!set_a.is_superset(&set_b));
    assert!(!set_b.is_superset(&set_a));

    assert!(!set_a.is_disjoint(&set_c));
    assert!(!set_c.is_disjoint(&set_a));
    assert!(set_a.is_subset(&set_c));
    assert!(!set_c.is_subset(&set_a));
    assert!(!set_a.is_superset(&set_c));
    assert!(set_c.is_superset(&set_a));

    assert!(!set_c.is_disjoint(&set_d));
    assert!(!set_d.is_disjoint(&set_c));
    assert!(!set_c.is_subset(&set_d));
    assert!(!set_d.is_subset(&set_c));
    assert!(!set_c.is_superset(&set_d));
    assert!(!set_d.is_superset(&set_c));
}

#[test]
fn iter_comparisons() {
    use std::iter::empty;

    fn check<'a, I1, I2>(iter1: I1, iter2: I2)
    where
        I1: Iterator<Item = &'a i32>,
        I2: Iterator<Item = i32>,
    {
        assert!(iter1.copied().eq(iter2));
    }

    let set_a: IndexSet<_> = (0..3).collect();
    let set_b: IndexSet<_> = (3..6).collect();
    let set_c: IndexSet<_> = (0..6).collect();
    let set_d: IndexSet<_> = (3..9).rev().collect();

    check(set_a.difference(&set_a), empty());
    check(set_a.symmetric_difference(&set_a), empty());
    check(set_a.intersection(&set_a), 0..3);
    check(set_a.union(&set_a), 0..3);

    check(set_a.difference(&set_b), 0..3);
    check(set_b.difference(&set_a), 3..6);
    check(set_a.symmetric_difference(&set_b), 0..6);
    check(set_b.symmetric_difference(&set_a), (3..6).chain(0..3));
    check(set_a.intersection(&set_b), empty());
    check(set_b.intersection(&set_a), empty());
    check(set_a.union(&set_b), 0..6);
    check(set_b.union(&set_a), (3..6).chain(0..3));

    check(set_a.difference(&set_c), empty());
    check(set_c.difference(&set_a), 3..6);
    check(set_a.symmetric_difference(&set_c), 3..6);
    check(set_c.symmetric_difference(&set_a), 3..6);
    check(set_a.intersection(&set_c), 0..3);
    check(set_c.intersection(&set_a), 0..3);
    check(set_a.union(&set_c), 0..6);
    check(set_c.union(&set_a), 0..6);

    check(set_c.difference(&set_d), 0..3);
    check(set_d.difference(&set_c), (6..9).rev());
    check(
        set_c.symmetric_difference(&set_d),
        (0..3).chain((6..9).rev()),
    );
    check(set_d.symmetric_difference(&set_c), (6..9).rev().chain(0..3));
    check(set_c.intersection(&set_d), 3..6);
    check(set_d.intersection(&set_c), (3..6).rev());
    check(set_c.union(&set_d), (0..6).chain((6..9).rev()));
    check(set_d.union(&set_c), (3..9).rev().chain(0..3));
}

#[test]
fn ops() {
    let empty = IndexSet::<i32>::new();
    let set_a: IndexSet<_> = (0..3).collect();
    let set_b: IndexSet<_> = (3..6).collect();
    let set_c: IndexSet<_> = (0..6).collect();
    let set_d: IndexSet<_> = (3..9).rev().collect();

    #[allow(clippy::eq_op)]
    {
        assert_eq!(&set_a & &set_a, set_a);
        assert_eq!(&set_a | &set_a, set_a);
        assert_eq!(&set_a ^ &set_a, empty);
        assert_eq!(&set_a - &set_a, empty);
    }

    assert_eq!(&set_a & &set_b, empty);
    assert_eq!(&set_b & &set_a, empty);
    assert_eq!(&set_a | &set_b, set_c);
    assert_eq!(&set_b | &set_a, set_c);
    assert_eq!(&set_a ^ &set_b, set_c);
    assert_eq!(&set_b ^ &set_a, set_c);
    assert_eq!(&set_a - &set_b, set_a);
    assert_eq!(&set_b - &set_a, set_b);

    assert_eq!(&set_a & &set_c, set_a);
    assert_eq!(&set_c & &set_a, set_a);
    assert_eq!(&set_a | &set_c, set_c);
    assert_eq!(&set_c | &set_a, set_c);
    assert_eq!(&set_a ^ &set_c, set_b);
    assert_eq!(&set_c ^ &set_a, set_b);
    assert_eq!(&set_a - &set_c, empty);
    assert_eq!(&set_c - &set_a, set_b);

    assert_eq!(&set_c & &set_d, set_b);
    assert_eq!(&set_d & &set_c, set_b);
    assert_eq!(&set_c | &set_d, &set_a | &set_d);
    assert_eq!(&set_d | &set_c, &set_a | &set_d);
    assert_eq!(&set_c ^ &set_d, &set_a | &(&set_d - &set_b));
    assert_eq!(&set_d ^ &set_c, &set_a | &(&set_d - &set_b));
    assert_eq!(&set_c - &set_d, set_a);
    assert_eq!(&set_d - &set_c, &set_d - &set_b);
}

#[test]
#[cfg(feature = "std")]
fn from_array() {
    let set1 = IndexSet::from([1, 2, 3, 4]);
    let set2: IndexSet<_> = [1, 2, 3, 4].into();

    assert_eq!(set1, set2);
}

#[test]
fn iter_default() {
    struct Item;
    fn assert_default<T>()
    where
        T: Default + Iterator,
    {
        assert!(T::default().next().is_none());
    }
    assert_default::<Iter<'static, Item>>();
    assert_default::<IntoIter<Item>>();
}

#[test]
#[allow(deprecated)]
fn take() {
    let mut index_set: IndexSet<i32> = IndexSet::new();
    index_set.insert(10);
    assert_eq!(index_set.len(), 1);

    let result = index_set.take(&10);
    assert_eq!(result, Some(10));
    assert_eq!(index_set.len(), 0);

    let result = index_set.take(&20);
    assert_eq!(result, None);
}

#[test]
fn swap_take() {
    let mut index_set: IndexSet<i32> = IndexSet::new();
    index_set.insert(10);
    index_set.insert(20);
    index_set.insert(30);
    index_set.insert(40);
    assert_eq!(index_set.len(), 4);

    let result = index_set.swap_take(&20);
    assert_eq!(result, Some(20));
    assert_eq!(index_set.len(), 3);
    assert_eq!(index_set.as_slice(), &[10, 40, 30]);

    let result = index_set.swap_take(&50);
    assert_eq!(result, None);
}

#[test]
fn sort_unstable() {
    let mut index_set: IndexSet<i32> = IndexSet::new();
    index_set.insert(30);
    index_set.insert(20);
    index_set.insert(10);

    index_set.sort_unstable();
    assert_eq!(index_set.as_slice(), &[10, 20, 30]);
}

#[test]
fn try_reserve_exact() {
    let mut index_set: IndexSet<i32> = IndexSet::new();
    index_set.insert(10);
    index_set.insert(20);
    index_set.insert(30);
    index_set.shrink_to_fit();
    assert_eq!(index_set.capacity(), 3);

    index_set.try_reserve_exact(2).unwrap();
    assert_eq!(index_set.capacity(), 5);
}

#[test]
fn shift_remove_full() {
    let mut set: IndexSet<i32> = IndexSet::new();
    set.insert(10);
    set.insert(20);
    set.insert(30);
    set.insert(40);
    set.insert(50);

    let result = set.shift_remove_full(&20);
    assert_eq!(result, Some((1, 20)));
    assert_eq!(set.len(), 4);
    assert_eq!(set.as_slice(), &[10, 30, 40, 50]);

    let result = set.shift_remove_full(&50);
    assert_eq!(result, Some((3, 50)));
    assert_eq!(set.len(), 3);
    assert_eq!(set.as_slice(), &[10, 30, 40]);

    let result = set.shift_remove_full(&60);
    assert_eq!(result, None);
    assert_eq!(set.len(), 3);
    assert_eq!(set.as_slice(), &[10, 30, 40]);
}

#[test]
fn shift_remove_index() {
    let mut set: IndexSet<i32> = IndexSet::new();
    set.insert(10);
    set.insert(20);
    set.insert(30);
    set.insert(40);
    set.insert(50);

    let result = set.shift_remove_index(1);
    assert_eq!(result, Some(20));
    assert_eq!(set.len(), 4);
    assert_eq!(set.as_slice(), &[10, 30, 40, 50]);

    let result = set.shift_remove_index(1);
    assert_eq!(result, Some(30));
    assert_eq!(set.len(), 3);
    assert_eq!(set.as_slice(), &[10, 40, 50]);

    let result = set.shift_remove_index(3);
    assert_eq!(result, None);
    assert_eq!(set.len(), 3);
    assert_eq!(set.as_slice(), &[10, 40, 50]);
}

#[test]
fn sort_unstable_by() {
    let mut set: IndexSet<i32> = IndexSet::from([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    set.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(set.as_slice(), &[10, 9, 8, 7, 6, 5, 4, 3, 2, 1]);
}

#[test]
fn sort_by() {
    let mut set: IndexSet<i32> = IndexSet::new();
    set.insert(3);
    set.insert(1);
    set.insert(2);
    set.sort_by(|a, b| a.cmp(b));
    assert_eq!(set.as_slice(), &[1, 2, 3]);
}

#[test]
fn drain() {
    let mut set: IndexSet<i32> = IndexSet::new();
    set.insert(1);
    set.insert(2);
    set.insert(3);

    {
        let drain = set.drain(0..2);
        assert_eq!(drain.as_slice(), &[1, 2]);
    }

    assert_eq!(set.len(), 1);
    assert_eq!(set.as_slice(), &[3]);
}

#[test]
fn split_off() {
    let mut set: IndexSet<i32> = IndexSet::from([1, 2, 3, 4, 5]);
    let split_set: IndexSet<i32> = set.split_off(3);

    assert_eq!(split_set.len(), 2);
    assert_eq!(split_set.as_slice(), &[4, 5]);

    assert_eq!(set.len(), 3);
    assert_eq!(set.as_slice(), &[1, 2, 3]);
}

#[test]
fn retain() {
    let mut set: IndexSet<i32> = IndexSet::from([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    set.retain(|&x| x > 4);
    assert_eq!(set.len(), 6);
    assert_eq!(set.as_slice(), &[5, 6, 7, 8, 9, 10]);

    set.retain(|_| false);
    assert_eq!(set.len(), 0);
}

#[test]
fn first() {
    let mut index_set: IndexSet<i32> = IndexSet::new();
    index_set.insert(10);
    index_set.insert(20);
    index_set.insert(30);

    let result = index_set.first();
    assert_eq!(*result.unwrap(), 10);

    index_set.clear();
    let result = index_set.first();
    assert!(result.is_none());
}

#[test]
fn sort_by_key() {
    let mut index_set: IndexSet<i32> = IndexSet::new();
    index_set.insert(3);
    index_set.insert(1);
    index_set.insert(2);
    index_set.insert(0);
    index_set.sort_by_key(|&x| -x);
    assert_eq!(index_set.as_slice(), &[3, 2, 1, 0]);
}

#[test]
fn sort_unstable_by_key() {
    let mut index_set: IndexSet<i32> = IndexSet::new();
    index_set.insert(3);
    index_set.insert(1);
    index_set.insert(2);
    index_set.insert(0);
    index_set.sort_unstable_by_key(|&x| -x);
    assert_eq!(index_set.as_slice(), &[3, 2, 1, 0]);
}

#[test]
fn sort_by_cached_key() {
    let mut index_set: IndexSet<i32> = IndexSet::new();
    index_set.insert(3);
    index_set.insert(1);
    index_set.insert(2);
    index_set.insert(0);
    index_set.sort_by_cached_key(|&x| -x);
    assert_eq!(index_set.as_slice(), &[3, 2, 1, 0]);
}

#[test]
fn insert_sorted() {
    let mut set: IndexSet<i32> = IndexSet::<i32>::new();
    set.insert_sorted(1);
    set.insert_sorted(3);
    assert_eq!(set.insert_sorted(2), (1, true));
}

#[test]
fn binary_search() {
    let mut set: IndexSet<i32> = IndexSet::new();
    set.insert(100);
    set.insert(300);
    set.insert(200);
    set.insert(400);
    let result = set.binary_search(&200);
    assert_eq!(result, Ok(2));

    let result = set.binary_search(&500);
    assert_eq!(result, Err(4));
}

#[test]
fn sorted_unstable_by() {
    let mut set: IndexSet<i32> = IndexSet::from([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    set.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(set.as_slice(), &[10, 9, 8, 7, 6, 5, 4, 3, 2, 1]);
}

#[test]
fn last() {
    let mut set: IndexSet<i32> = IndexSet::new();
    set.insert(1);
    set.insert(2);
    set.insert(3);
    set.insert(4);
    set.insert(5);
    set.insert(6);

    assert_eq!(set.last(), Some(&6));

    set.pop();
    assert_eq!(set.last(), Some(&5));

    set.clear();
    assert_eq!(set.last(), None);
}

#[test]
fn get_range() {
    let set: IndexSet<i32> = IndexSet::from([1, 2, 3, 4, 5]);
    let result = set.get_range(0..3);
    let slice: &Slice<i32> = result.unwrap();
    assert_eq!(slice, &[1, 2, 3]);

    let result = set.get_range(0..0);
    assert_eq!(result.unwrap().len(), 0);

    let result = set.get_range(2..1);
    assert!(result.is_none());
}

#[test]
fn shift_take() {
    let mut set: IndexSet<i32> = IndexSet::new();
    set.insert(1);
    set.insert(2);
    set.insert(3);
    set.insert(4);
    set.insert(5);

    let result = set.shift_take(&2);
    assert_eq!(result, Some(2));
    assert_eq!(set.len(), 4);
    assert_eq!(set.as_slice(), &[1, 3, 4, 5]);

    let result = set.shift_take(&5);
    assert_eq!(result, Some(5));
    assert_eq!(set.len(), 3);
    assert_eq!(set.as_slice(), &[1, 3, 4]);

    let result = set.shift_take(&5);
    assert_eq!(result, None);
    assert_eq!(set.len(), 3);
    assert_eq!(set.as_slice(), &[1, 3, 4]);
}

#[test]
fn test_binary_search_by() {
    // adapted from std's test for binary_search
    let b: IndexSet<i32> = [].into();
    assert_eq!(b.binary_search_by(|x| x.cmp(&5)), Err(0));

    let b: IndexSet<i32> = [4].into();
    assert_eq!(b.binary_search_by(|x| x.cmp(&3)), Err(0));
    assert_eq!(b.binary_search_by(|x| x.cmp(&4)), Ok(0));
    assert_eq!(b.binary_search_by(|x| x.cmp(&5)), Err(1));

    let b: IndexSet<i32> = [1, 2, 4, 6, 8, 9].into();
    assert_eq!(b.binary_search_by(|x| x.cmp(&5)), Err(3));
    assert_eq!(b.binary_search_by(|x| x.cmp(&6)), Ok(3));
    assert_eq!(b.binary_search_by(|x| x.cmp(&7)), Err(4));
    assert_eq!(b.binary_search_by(|x| x.cmp(&8)), Ok(4));

    let b: IndexSet<i32> = [1, 2, 4, 5, 6, 8].into();
    assert_eq!(b.binary_search_by(|x| x.cmp(&9)), Err(6));

    let b: IndexSet<i32> = [1, 2, 4, 6, 7, 8, 9].into();
    assert_eq!(b.binary_search_by(|x| x.cmp(&6)), Ok(3));
    assert_eq!(b.binary_search_by(|x| x.cmp(&5)), Err(3));
    assert_eq!(b.binary_search_by(|x| x.cmp(&8)), Ok(5));

    let b: IndexSet<i32> = [1, 2, 4, 5, 6, 8, 9].into();
    assert_eq!(b.binary_search_by(|x| x.cmp(&7)), Err(5));
    assert_eq!(b.binary_search_by(|x| x.cmp(&0)), Err(0));

    let b: IndexSet<i32> = [1, 3, 3, 3, 7].into();
    assert_eq!(b.binary_search_by(|x| x.cmp(&0)), Err(0));
    assert_eq!(b.binary_search_by(|x| x.cmp(&1)), Ok(0));
    assert_eq!(b.binary_search_by(|x| x.cmp(&2)), Err(1));
    // diff from std as set merges the duplicate keys
    assert!(match b.binary_search_by(|x| x.cmp(&3)) {
        Ok(1..=2) => true,
        _ => false,
    });
    assert!(match b.binary_search_by(|x| x.cmp(&3)) {
        Ok(1..=2) => true,
        _ => false,
    });
    assert_eq!(b.binary_search_by(|x| x.cmp(&4)), Err(2));
    assert_eq!(b.binary_search_by(|x| x.cmp(&5)), Err(2));
    assert_eq!(b.binary_search_by(|x| x.cmp(&6)), Err(2));
    assert_eq!(b.binary_search_by(|x| x.cmp(&7)), Ok(2));
    assert_eq!(b.binary_search_by(|x| x.cmp(&8)), Err(3));
}

#[test]
fn test_binary_search_by_key() {
    // adapted from std's test for binary_search
    let b: IndexSet<i32> = [].into();
    assert_eq!(b.binary_search_by_key(&5, |&x| x), Err(0));

    let b: IndexSet<i32> = [4].into();
    assert_eq!(b.binary_search_by_key(&3, |&x| x), Err(0));
    assert_eq!(b.binary_search_by_key(&4, |&x| x), Ok(0));
    assert_eq!(b.binary_search_by_key(&5, |&x| x), Err(1));

    let b: IndexSet<i32> = [1, 2, 4, 6, 8, 9].into();
    assert_eq!(b.binary_search_by_key(&5, |&x| x), Err(3));
    assert_eq!(b.binary_search_by_key(&6, |&x| x), Ok(3));
    assert_eq!(b.binary_search_by_key(&7, |&x| x), Err(4));
    assert_eq!(b.binary_search_by_key(&8, |&x| x), Ok(4));

    let b: IndexSet<i32> = [1, 2, 4, 5, 6, 8].into();
    assert_eq!(b.binary_search_by_key(&9, |&x| x), Err(6));

    let b: IndexSet<i32> = [1, 2, 4, 6, 7, 8, 9].into();
    assert_eq!(b.binary_search_by_key(&6, |&x| x), Ok(3));
    assert_eq!(b.binary_search_by_key(&5, |&x| x), Err(3));
    assert_eq!(b.binary_search_by_key(&8, |&x| x), Ok(5));

    let b: IndexSet<i32> = [1, 2, 4, 5, 6, 8, 9].into();
    assert_eq!(b.binary_search_by_key(&7, |&x| x), Err(5));
    assert_eq!(b.binary_search_by_key(&0, |&x| x), Err(0));

    let b: IndexSet<i32> = [1, 3, 3, 3, 7].into();
    assert_eq!(b.binary_search_by_key(&0, |&x| x), Err(0));
    assert_eq!(b.binary_search_by_key(&1, |&x| x), Ok(0));
    assert_eq!(b.binary_search_by_key(&2, |&x| x), Err(1));
    // diff from std as set merges the duplicate keys
    assert!(match b.binary_search_by_key(&3, |&x| x) {
        Ok(1..=2) => true,
        _ => false,
    });
    assert!(match b.binary_search_by_key(&3, |&x| x) {
        Ok(1..=2) => true,
        _ => false,
    });
    assert_eq!(b.binary_search_by_key(&4, |&x| x), Err(2));
    assert_eq!(b.binary_search_by_key(&5, |&x| x), Err(2));
    assert_eq!(b.binary_search_by_key(&6, |&x| x), Err(2));
    assert_eq!(b.binary_search_by_key(&7, |&x| x), Ok(2));
    assert_eq!(b.binary_search_by_key(&8, |&x| x), Err(3));
}

#[test]
fn test_partition_point() {
    // adapted from std's test for partition_point
    let b: IndexSet<i32> = [].into();
    assert_eq!(b.partition_point(|&x| x < 5), 0);

    let b: IndexSet<_> = [4].into();
    assert_eq!(b.partition_point(|&x| x < 3), 0);
    assert_eq!(b.partition_point(|&x| x < 4), 0);
    assert_eq!(b.partition_point(|&x| x < 5), 1);

    let b: IndexSet<_> = [1, 2, 4, 6, 8, 9].into();
    assert_eq!(b.partition_point(|&x| x < 5), 3);
    assert_eq!(b.partition_point(|&x| x < 6), 3);
    assert_eq!(b.partition_point(|&x| x < 7), 4);
    assert_eq!(b.partition_point(|&x| x < 8), 4);

    let b: IndexSet<_> = [1, 2, 4, 5, 6, 8].into();
    assert_eq!(b.partition_point(|&x| x < 9), 6);

    let b: IndexSet<_> = [1, 2, 4, 6, 7, 8, 9].into();
    assert_eq!(b.partition_point(|&x| x < 6), 3);
    assert_eq!(b.partition_point(|&x| x < 5), 3);
    assert_eq!(b.partition_point(|&x| x < 8), 5);

    let b: IndexSet<_> = [1, 2, 4, 5, 6, 8, 9].into();
    assert_eq!(b.partition_point(|&x| x < 7), 5);
    assert_eq!(b.partition_point(|&x| x < 0), 0);

    let b: IndexSet<_> = [1, 3, 3, 3, 7].into();
    assert_eq!(b.partition_point(|&x| x < 0), 0);
    assert_eq!(b.partition_point(|&x| x < 1), 0);
    assert_eq!(b.partition_point(|&x| x < 2), 1);
    assert_eq!(b.partition_point(|&x| x < 3), 1);
    assert_eq!(b.partition_point(|&x| x < 4), 2); // diff from std as set merges the duplicate keys
    assert_eq!(b.partition_point(|&x| x < 5), 2);
    assert_eq!(b.partition_point(|&x| x < 6), 2);
    assert_eq!(b.partition_point(|&x| x < 7), 2);
    assert_eq!(b.partition_point(|&x| x < 8), 3);
}

#[test]
fn is_sorted() {
    fn expect(set: &IndexSet<i32>, e: [bool; 4]) {
        assert_eq!(e[0], set.is_sorted());
        assert_eq!(e[1], set.is_sorted_by(|v1, v2| v1 < v2));
        assert_eq!(e[2], set.is_sorted_by(|v1, v2| v1 > v2));
        assert_eq!(e[3], set.is_sorted_by_key(|v| v));
    }

    let mut set = IndexSet::<i32>::from_iter(0..10);
    expect(&set, [true, true, false, true]);

    set.replace_index(5, -1).unwrap();
    expect(&set, [false, false, false, false]);
}

#[test]
fn is_sorted_trivial() {
    fn expect(set: &IndexSet<i32>, e: [bool; 5]) {
        assert_eq!(e[0], set.is_sorted());
        assert_eq!(e[1], set.is_sorted_by(|_, _| true));
        assert_eq!(e[2], set.is_sorted_by(|_, _| false));
        assert_eq!(e[3], set.is_sorted_by_key(|_| 0f64));
        assert_eq!(e[4], set.is_sorted_by_key(|_| f64::NAN));
    }

    let mut set = IndexSet::<i32>::default();
    expect(&set, [true, true, true, true, true]);

    set.insert(0);
    expect(&set, [true, true, true, true, true]);

    set.insert(1);
    expect(&set, [true, true, false, true, false]);

    set.reverse();
    expect(&set, [false, true, false, true, false]);
}
