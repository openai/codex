#![allow(unstable_name_collisions)]

use itertools::Itertools;

#[derive(Debug, Clone)]
#[must_use = "iterators are lazy and do nothing unless consumed"]
struct Panicking;

impl Iterator for Panicking {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        panic!("iterator adaptor is not lazy")
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(0))
    }
}

impl ExactSizeIterator for Panicking {}

/// ## Usage example
/// ```compile_fail
/// must_use_tests! {
///     name {
///         Panicking.name(); // Add `let _ =` only if required (encountered error).
///     }
///     // ...
/// }
/// ```
///
/// **TODO:** test missing `must_use` attributes better, maybe with a new lint.
macro_rules! must_use_tests {
    ($($(#[$attr:meta])* $name:ident $body:block)*) => {
        $(
            /// `#[deny(unused_must_use)]` should force us to ignore the resulting iterators
            /// by adding `let _ = ...;` on every iterator.
            /// If it does not, then a `must_use` attribute is missing on the associated struct.
            ///
            /// However, it's only helpful if we don't add `let _ =` before seeing if there is an error or not.
            /// And it does not protect us against removed `must_use` attributes.
            /// There is no simple way to test this yet.
            #[deny(unused_must_use)]
            #[test]
            $(#[$attr])*
            fn $name() $body
        )*
    };
}

must_use_tests! {
    // Itertools trait:
    interleave {
        let _ = Panicking.interleave(Panicking);
    }
    interleave_shortest {
        let _ = Panicking.interleave_shortest(Panicking);
    }
    intersperse {
        let _ = Panicking.intersperse(0);
    }
    intersperse_with {
        let _ = Panicking.intersperse_with(|| 0);
    }
    get {
        let _ = Panicking.get(1..4);
        let _ = Panicking.get(1..=4);
        let _ = Panicking.get(1..);
        let _ = Panicking.get(..4);
        let _ = Panicking.get(..=4);
        let _ = Panicking.get(..);
    }
    zip_longest {
        let _ = Panicking.zip_longest(Panicking);
    }
    zip_eq {
        let _ = Panicking.zip_eq(Panicking);
    }
    batching {
        let _ = Panicking.batching(Iterator::next);
    }
    chunk_by {
        // ChunkBy
        let _ = Panicking.chunk_by(|x| *x);
        // Groups
        let _ = Panicking.chunk_by(|x| *x).into_iter();
    }
    chunks {
        // IntoChunks
        let _ = Panicking.chunks(1);
        let _ = Panicking.chunks(2);
        // Chunks
        let _ = Panicking.chunks(1).into_iter();
        let _ = Panicking.chunks(2).into_iter();
    }
    tuple_windows {
        let _ = Panicking.tuple_windows::<(_,)>();
        let _ = Panicking.tuple_windows::<(_, _)>();
        let _ = Panicking.tuple_windows::<(_, _, _)>();
    }
    circular_tuple_windows {
        let _ = Panicking.circular_tuple_windows::<(_,)>();
        let _ = Panicking.circular_tuple_windows::<(_, _)>();
        let _ = Panicking.circular_tuple_windows::<(_, _, _)>();
    }
    tuples {
        let _ = Panicking.tuples::<(_,)>();
        let _ = Panicking.tuples::<(_, _)>();
        let _ = Panicking.tuples::<(_, _, _)>();
    }
    tee {
        let _ = Panicking.tee();
    }
    map_into {
        let _ = Panicking.map_into::<u16>();
    }
    map_ok {
        let _ = Panicking.map(Ok::<u8, ()>).map_ok(|x| x + 1);
    }
    filter_ok {
        let _ = Panicking.map(Ok::<u8, ()>).filter_ok(|x| x % 2 == 0);
    }
    filter_map_ok {
        let _ = Panicking.map(Ok::<u8, ()>).filter_map_ok(|x| {
            if x % 2 == 0 {
                Some(x + 1)
            } else {
                None
            }
        });
    }
    flatten_ok {
        let _ = Panicking.map(|x| Ok::<_, ()>([x])).flatten_ok();
    }
    merge {
        let _ = Panicking.merge(Panicking);
    }
    merge_by {
        let _ = Panicking.merge_by(Panicking, |_, _| true);
    }
    merge_join_by {
        let _ = Panicking.merge_join_by(Panicking, |_, _| true);
        let _ = Panicking.merge_join_by(Panicking, Ord::cmp);
    }
    #[should_panic]
    kmerge {
        let _ = Panicking.map(|_| Panicking).kmerge();
    }
    #[should_panic]
    kmerge_by {
        let _ = Panicking.map(|_| Panicking).kmerge_by(|_, _| true);
    }
    cartesian_product {
        let _ = Panicking.cartesian_product(Panicking);
    }
    multi_cartesian_product {
        let _ = vec![Panicking, Panicking, Panicking].into_iter().multi_cartesian_product();
    }
    coalesce {
        let _ = Panicking.coalesce(|x, y| if x == y { Ok(x) } else { Err((x, y)) });
    }
    dedup {
        let _ = Panicking.dedup();
    }
    dedup_by {
        let _ = Panicking.dedup_by(|_, _| true);
    }
    dedup_with_count {
        let _ = Panicking.dedup_with_count();
    }
    dedup_by_with_count {
        let _ = Panicking.dedup_by_with_count(|_, _| true);
    }
    duplicates {
        let _ = Panicking.duplicates();
    }
    duplicates_by {
        let _ = Panicking.duplicates_by(|x| *x);
    }
    unique {
        let _ = Panicking.unique();
    }
    unique_by {
        let _ = Panicking.unique_by(|x| *x);
    }
    peeking_take_while {
        let _ = Panicking.peekable().peeking_take_while(|x| x % 2 == 0);
    }
    take_while_ref {
        let _ = Panicking.take_while_ref(|x| x % 2 == 0);
    }
    take_while_inclusive {
        let _ = Panicking.take_while_inclusive(|x| x % 2 == 0);
    }
    while_some {
        let _ = Panicking.map(Some).while_some();
    }
    tuple_combinations1 {
        let _ = Panicking.tuple_combinations::<(_,)>();
    }
    #[should_panic]
    tuple_combinations2 {
        let _ = Panicking.tuple_combinations::<(_, _)>();
    }
    #[should_panic]
    tuple_combinations3 {
        let _ = Panicking.tuple_combinations::<(_, _, _)>();
    }
    combinations {
        let _ = Panicking.combinations(0);
        let _ = Panicking.combinations(1);
        let _ = Panicking.combinations(2);
    }
    combinations_with_replacement {
        let _ = Panicking.combinations_with_replacement(0);
        let _ = Panicking.combinations_with_replacement(1);
        let _ = Panicking.combinations_with_replacement(2);
    }
    permutations {
        let _ = Panicking.permutations(0);
        let _ = Panicking.permutations(1);
        let _ = Panicking.permutations(2);
    }
    powerset {
        let _ = Panicking.powerset();
    }
    pad_using {
        let _ = Panicking.pad_using(25, |_| 10);
    }
    with_position {
        let _ = Panicking.with_position();
    }
    positions {
        let _ = Panicking.positions(|v| v % 2 == 0);
    }
    update {
        let _ = Panicking.update(|n| *n += 1);
    }
    multipeek {
        let _ = Panicking.multipeek();
    }
    // Not iterator themselves but still lazy.
    into_grouping_map {
        let _ = Panicking.map(|x| (x, x + 1)).into_grouping_map();
    }
    into_grouping_map_by {
        let _ = Panicking.into_grouping_map_by(|x| *x);
    }
    // Macros:
    iproduct {
        let _ = itertools::iproduct!(Panicking);
        let _ = itertools::iproduct!(Panicking, Panicking);
        let _ = itertools::iproduct!(Panicking, Panicking, Panicking);
    }
    izip {
        let _ = itertools::izip!(Panicking);
        let _ = itertools::izip!(Panicking, Panicking);
        let _ = itertools::izip!(Panicking, Panicking, Panicking);
    }
    chain {
        let _ = itertools::chain!(Panicking);
        let _ = itertools::chain!(Panicking, Panicking);
        let _ = itertools::chain!(Panicking, Panicking, Panicking);
    }
    // Free functions:
    multizip {
        let _ = itertools::multizip((Panicking, Panicking));
    }
    put_back {
        let _ = itertools::put_back(Panicking);
        let _ = itertools::put_back(Panicking).with_value(15);
    }
    peek_nth {
        let _ = itertools::peek_nth(Panicking);
    }
    put_back_n {
        let _ = itertools::put_back_n(Panicking);
    }
    rciter {
        let _ = itertools::rciter(Panicking);
    }
}
