#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)] // some code is tested for type checking only

#[cfg(not(feature = "std"))]
#[macro_use]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use core::fmt::Debug;

use derive_more::IntoIterator;

#[track_caller]
fn assert_iter<T: PartialEq + Debug, I: IntoIterator<Item = T>>(iter: I, vals: &[T]) {
    assert_eq!(iter.into_iter().collect::<Vec<_>>(), vals);
}

#[derive(IntoIterator)]
#[into_iterator(owned, ref, ref_mut)]
struct MyVec(Vec<i32>);

#[test]
fn tuple_single() {
    let mut vals = vec![1, 2, 3];
    let mut iter = MyVec(vals.clone());

    assert_iter(&mut iter, &vals.iter_mut().collect::<Vec<_>>());
    assert_iter(&iter, &vals.iter().collect::<Vec<_>>());
    assert_iter(iter, &vals);
}

#[derive(IntoIterator)]
#[into_iterator(owned, ref, ref_mut)]
struct Numbers {
    numbers: Vec<i32>,
}

#[test]
fn named_single() {
    let mut vals = vec![1, 2, 3];
    let mut iter = Numbers {
        numbers: vals.clone(),
    };

    assert_iter(&mut iter, &vals.iter_mut().collect::<Vec<_>>());
    assert_iter(&iter, &vals.iter().collect::<Vec<_>>());
    assert_iter(iter, &vals);
}

#[derive(IntoIterator)]
struct Numbers2 {
    #[into_iterator(owned, ref, ref_mut)]
    numbers: Vec<i32>,
    useless: bool,
    useless2: bool,
}

fn named_many() {
    let mut vals = vec![1, 2, 3];
    let mut iter = Numbers2 {
        numbers: vals.clone(),
        useless: true,
        useless2: true,
    };

    assert_iter(&mut iter, &vals.iter_mut().collect::<Vec<_>>());
    assert_iter(&iter, &vals.iter().collect::<Vec<_>>());
    assert_iter(iter, &vals);
}

#[derive(IntoIterator)]
struct Numbers3 {
    #[into_iterator(ref, ref_mut)]
    numbers: Vec<i32>,
    useless: bool,
    useless2: bool,
}

// Test that `owned` is not enabled when `ref`/`ref_mut` are enabled without `owned`.
impl ::core::iter::IntoIterator for Numbers3 {
    type Item = <Vec<i32> as ::core::iter::IntoIterator>::Item;
    type IntoIter = <Vec<i32> as ::core::iter::IntoIterator>::IntoIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        <Vec<i32> as ::core::iter::IntoIterator>::into_iter(self.numbers)
    }
}

#[derive(IntoIterator)]
struct Generic1<T> {
    #[into_iterator(owned, ref, ref_mut)]
    items: Vec<T>,
}

#[test]
fn generic() {
    let mut vals = vec![1, 2, 3];
    let mut iter = Generic1 {
        items: vals.clone(),
    };

    assert_iter(&mut iter, &vals.iter_mut().collect::<Vec<_>>());
    assert_iter(&iter, &vals.iter().collect::<Vec<_>>());
    assert_iter(iter, &vals);
}

#[derive(IntoIterator)]
struct Generic2<'a, T, U: Send>
where
    T: Send,
{
    #[into_iterator(owned, ref, ref_mut)]
    items: Vec<T>,
    useless: &'a U,
}

#[test]
fn generic_bounds() {
    let mut vals = vec![1, 2, 3];
    let useless = false;
    let mut iter = Generic2 {
        items: vals.clone(),
        useless: &useless,
    };

    assert_iter(&mut iter, &vals.iter_mut().collect::<Vec<_>>());
    assert_iter(&iter, &vals.iter().collect::<Vec<_>>());
    assert_iter(iter, &vals);
}

#[derive(IntoIterator)]
struct Generic3<'a, 'b, T> {
    #[into_iterator(owned)]
    items: &'a mut Vec<&'b mut T>,
}

#[test]
fn generic_refs() {
    let mut numbers = vec![1, 2, 3];
    let mut numbers2 = numbers.clone();

    let mut number_refs = numbers.iter_mut().collect::<Vec<_>>();
    let mut number_refs2 = numbers2.iter_mut().collect::<Vec<_>>();

    assert_iter(
        Generic3 {
            items: &mut number_refs,
        },
        &number_refs2.iter_mut().collect::<Vec<_>>(),
    )
}

#[derive(IntoIterator)]
struct Generic4<T> {
    #[into_iterator]
    items: Vec<T>,
    useless: bool,
}

#[test]
fn generic_owned() {
    let numbers = vec![1, 2, 3];

    assert_iter(
        Generic4 {
            items: numbers.clone(),
            useless: true,
        },
        &numbers,
    );
}
