#![allow(unstable_name_collisions)]

use itertools::Itertools;
use quickcheck::Arbitrary;
use quickcheck::{quickcheck, TestResult};
use rand::Rng;
use std::fmt::Debug;

struct Unspecialized<I>(I);

impl<I> Iterator for Unspecialized<I>
where
    I: Iterator,
{
    type Item = I::Item;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<I> DoubleEndedIterator for Unspecialized<I>
where
    I: DoubleEndedIterator,
{
    #[inline(always)]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back()
    }
}

fn test_specializations<I>(it: &I)
where
    I::Item: Eq + Debug + Clone,
    I: Iterator + Clone,
{
    macro_rules! check_specialized {
        ($src:expr, |$it:pat| $closure:expr) => {
            // Many iterators special-case the first elements, so we test specializations for iterators that have already been advanced.
            let mut src = $src.clone();
            for _ in 0..5 {
                let $it = src.clone();
                let v1 = $closure;
                let $it = Unspecialized(src.clone());
                let v2 = $closure;
                assert_eq!(v1, v2);
                src.next();
            }
        }
    }
    check_specialized!(it, |i| i.count());
    check_specialized!(it, |i| i.last());
    check_specialized!(it, |i| i.collect::<Vec<_>>());
    check_specialized!(it, |i| {
        let mut parameters_from_fold = vec![];
        let fold_result = i.fold(vec![], |mut acc, v: I::Item| {
            parameters_from_fold.push((acc.clone(), v.clone()));
            acc.push(v);
            acc
        });
        (parameters_from_fold, fold_result)
    });
    check_specialized!(it, |mut i| {
        let mut parameters_from_all = vec![];
        let first = i.next();
        let all_result = i.all(|x| {
            parameters_from_all.push(x.clone());
            Some(x) == first
        });
        (parameters_from_all, all_result)
    });
    let size = it.clone().count();
    for n in 0..size + 2 {
        check_specialized!(it, |mut i| i.nth(n));
    }
    // size_hint is a bit harder to check
    let mut it_sh = it.clone();
    for n in 0..size + 2 {
        let len = it_sh.clone().count();
        let (min, max) = it_sh.size_hint();
        assert_eq!(size - n.min(size), len);
        assert!(min <= len);
        if let Some(max) = max {
            assert!(len <= max);
        }
        it_sh.next();
    }
}

fn test_double_ended_specializations<I>(it: &I)
where
    I::Item: Eq + Debug + Clone,
    I: DoubleEndedIterator + Clone,
{
    macro_rules! check_specialized {
        ($src:expr, |$it:pat| $closure:expr) => {
            // Many iterators special-case the first elements, so we test specializations for iterators that have already been advanced.
            let mut src = $src.clone();
            for step in 0..8 {
                let $it = src.clone();
                let v1 = $closure;
                let $it = Unspecialized(src.clone());
                let v2 = $closure;
                assert_eq!(v1, v2);
                if step % 2 == 0 {
                    src.next();
                } else {
                    src.next_back();
                }
            }
        }
    }
    check_specialized!(it, |i| {
        let mut parameters_from_rfold = vec![];
        let rfold_result = i.rfold(vec![], |mut acc, v: I::Item| {
            parameters_from_rfold.push((acc.clone(), v.clone()));
            acc.push(v);
            acc
        });
        (parameters_from_rfold, rfold_result)
    });
    let size = it.clone().count();
    for n in 0..size + 2 {
        check_specialized!(it, |mut i| i.nth_back(n));
    }
}

quickcheck! {
    fn interleave(v: Vec<u8>, w: Vec<u8>) -> () {
        test_specializations(&v.iter().interleave(w.iter()));
    }

    fn interleave_shortest(v: Vec<u8>, w: Vec<u8>) -> () {
        test_specializations(&v.iter().interleave_shortest(w.iter()));
    }

    fn batching(v: Vec<u8>) -> () {
        test_specializations(&v.iter().batching(Iterator::next));
    }

    fn tuple_windows(v: Vec<u8>) -> () {
        test_specializations(&v.iter().tuple_windows::<(_,)>());
        test_specializations(&v.iter().tuple_windows::<(_, _)>());
        test_specializations(&v.iter().tuple_windows::<(_, _, _)>());
    }

    fn circular_tuple_windows(v: Vec<u8>) -> () {
        test_specializations(&v.iter().circular_tuple_windows::<(_,)>());
        test_specializations(&v.iter().circular_tuple_windows::<(_, _)>());
        test_specializations(&v.iter().circular_tuple_windows::<(_, _, _)>());
    }

    fn tuples(v: Vec<u8>) -> () {
        test_specializations(&v.iter().tuples::<(_,)>());
        test_specializations(&v.iter().tuples::<(_, _)>());
        test_specializations(&v.iter().tuples::<(_, _, _)>());
    }

    fn cartesian_product(a: Vec<u8>, b: Vec<u8>) -> TestResult {
        if a.len() * b.len() > 100 {
            return TestResult::discard();
        }
        test_specializations(&a.iter().cartesian_product(&b));
        TestResult::passed()
    }

    fn multi_cartesian_product(a: Vec<u8>, b: Vec<u8>, c: Vec<u8>) -> TestResult {
        if a.len() * b.len() * c.len() > 100 {
            return TestResult::discard();
        }
        test_specializations(&vec![a, b, c].into_iter().multi_cartesian_product());
        TestResult::passed()
    }

    fn coalesce(v: Vec<u8>) -> () {
        test_specializations(&v.iter().coalesce(|x, y| if x == y { Ok(x) } else { Err((x, y)) }))
    }

    fn dedup(v: Vec<u8>) -> () {
        test_specializations(&v.iter().dedup())
    }

    fn dedup_by(v: Vec<u8>) -> () {
        test_specializations(&v.iter().dedup_by(PartialOrd::ge))
    }

    fn dedup_with_count(v: Vec<u8>) -> () {
        test_specializations(&v.iter().dedup_with_count())
    }

    fn dedup_by_with_count(v: Vec<u8>) -> () {
        test_specializations(&v.iter().dedup_by_with_count(PartialOrd::ge))
    }

    fn duplicates(v: Vec<u8>) -> () {
        let it = v.iter().duplicates();
        test_specializations(&it);
        test_double_ended_specializations(&it);
    }

    fn duplicates_by(v: Vec<u8>) -> () {
        let it = v.iter().duplicates_by(|x| *x % 10);
        test_specializations(&it);
        test_double_ended_specializations(&it);
    }

    fn unique(v: Vec<u8>) -> () {
        let it = v.iter().unique();
        test_specializations(&it);
        test_double_ended_specializations(&it);
    }

    fn unique_by(v: Vec<u8>) -> () {
        let it = v.iter().unique_by(|x| *x % 50);
        test_specializations(&it);
        test_double_ended_specializations(&it);
    }

    fn take_while_inclusive(v: Vec<u8>) -> () {
        test_specializations(&v.iter().copied().take_while_inclusive(|&x| x < 100));
    }

    fn while_some(v: Vec<u8>) -> () {
        test_specializations(&v.iter().map(|&x| if x < 100 { Some(2 * x) } else { None }).while_some());
    }

    fn pad_using(v: Vec<u8>) -> () {
        use std::convert::TryFrom;
        let it = v.iter().copied().pad_using(10, |i| u8::try_from(5 * i).unwrap_or(u8::MAX));
        test_specializations(&it);
        test_double_ended_specializations(&it);
    }

    fn with_position(v: Vec<u8>) -> () {
        test_specializations(&v.iter().with_position());
    }

    fn positions(v: Vec<u8>) -> () {
        let it = v.iter().positions(|x| x % 5 == 0);
        test_specializations(&it);
        test_double_ended_specializations(&it);
    }

    fn update(v: Vec<u8>) -> () {
        let it = v.iter().copied().update(|x| *x = x.wrapping_mul(7));
        test_specializations(&it);
        test_double_ended_specializations(&it);
    }

    fn tuple_combinations(v: Vec<u8>) -> TestResult {
        if v.len() > 10 {
            return TestResult::discard();
        }
        test_specializations(&v.iter().tuple_combinations::<(_,)>());
        test_specializations(&v.iter().tuple_combinations::<(_, _)>());
        test_specializations(&v.iter().tuple_combinations::<(_, _, _)>());
        TestResult::passed()
    }

    fn intersperse(v: Vec<u8>) -> () {
        test_specializations(&v.into_iter().intersperse(0));
    }

    fn intersperse_with(v: Vec<u8>) -> () {
        test_specializations(&v.into_iter().intersperse_with(|| 0));
    }

    fn combinations(a: Vec<u8>, n: u8) -> TestResult {
        if n > 3 || a.len() > 8 {
            return TestResult::discard();
        }
        test_specializations(&a.iter().combinations(n as usize));
        TestResult::passed()
    }

    fn combinations_with_replacement(a: Vec<u8>, n: u8) -> TestResult {
        if n > 3 || a.len() > 7 {
            return TestResult::discard();
        }
        test_specializations(&a.iter().combinations_with_replacement(n as usize));
        TestResult::passed()
    }

    fn permutations(a: Vec<u8>, n: u8) -> TestResult {
        if n > 3 || a.len() > 8 {
            return TestResult::discard();
        }
        test_specializations(&a.iter().permutations(n as usize));
        TestResult::passed()
    }

    fn powerset(a: Vec<u8>) -> TestResult {
        if a.len() > 6 {
            return TestResult::discard();
        }
        test_specializations(&a.iter().powerset());
        TestResult::passed()
    }

    fn zip_longest(a: Vec<u8>, b: Vec<u8>) -> () {
        let it = a.into_iter().zip_longest(b);
        test_specializations(&it);
        test_double_ended_specializations(&it);
    }

    fn zip_eq(a: Vec<u8>) -> () {
        test_specializations(&a.iter().zip_eq(a.iter().rev()))
    }

    fn multizip(a: Vec<u8>) -> () {
        let it = itertools::multizip((a.iter(), a.iter().rev(), a.iter().take(50)));
        test_specializations(&it);
        test_double_ended_specializations(&it);
    }

    fn izip(a: Vec<u8>, b: Vec<u8>) -> () {
        test_specializations(&itertools::izip!(b.iter(), a, b.iter().rev()));
    }

    fn iproduct(a: Vec<u8>, b: Vec<u8>, c: Vec<u8>) -> TestResult {
        if a.len() * b.len() * c.len() > 200 {
            return TestResult::discard();
        }
        test_specializations(&itertools::iproduct!(a, b.iter(), c));
        TestResult::passed()
    }

    fn repeat_n(element: i8, n: u8) -> () {
        let it = itertools::repeat_n(element, n as usize);
        test_specializations(&it);
        test_double_ended_specializations(&it);
    }

    fn exactly_one_error(v: Vec<u8>) -> TestResult {
        // Use `at_most_one` would be similar.
        match v.iter().exactly_one() {
            Ok(_) => TestResult::discard(),
            Err(it) => {
                test_specializations(&it);
                TestResult::passed()
            }
        }
    }
}

quickcheck! {
    fn put_back_qc(test_vec: Vec<i32>) -> () {
        test_specializations(&itertools::put_back(test_vec.iter()));
        let mut pb = itertools::put_back(test_vec.into_iter());
        pb.put_back(1);
        test_specializations(&pb);
    }

    fn put_back_n(v: Vec<u8>, n: u8) -> () {
        let mut it = itertools::put_back_n(v);
        for k in 0..n {
            it.put_back(k);
        }
        test_specializations(&it);
    }

    fn multipeek(v: Vec<u8>, n: u8) -> () {
        let mut it = v.into_iter().multipeek();
        for _ in 0..n {
            it.peek();
        }
        test_specializations(&it);
    }

    fn peek_nth_with_peek(v: Vec<u8>, n: u8) -> () {
        let mut it = itertools::peek_nth(v);
        for _ in 0..n {
            it.peek();
        }
        test_specializations(&it);
    }

    fn peek_nth_with_peek_nth(v: Vec<u8>, n: u8) -> () {
        let mut it = itertools::peek_nth(v);
        it.peek_nth(n as usize);
        test_specializations(&it);
    }

    fn peek_nth_with_peek_mut(v: Vec<u8>, n: u8) -> () {
        let mut it = itertools::peek_nth(v);
        for _ in 0..n {
            if let Some(x) = it.peek_mut() {
                *x = x.wrapping_add(50);
            }
        }
        test_specializations(&it);
    }

    fn peek_nth_with_peek_nth_mut(v: Vec<u8>, n: u8) -> () {
        let mut it = itertools::peek_nth(v);
        if let Some(x) = it.peek_nth_mut(n as usize) {
            *x = x.wrapping_add(50);
        }
        test_specializations(&it);
    }
}

quickcheck! {
    fn merge(a: Vec<u8>, b: Vec<u8>) -> () {
        test_specializations(&a.into_iter().merge(b))
    }

    fn merge_by(a: Vec<u8>, b: Vec<u8>) -> () {
        test_specializations(&a.into_iter().merge_by(b, PartialOrd::ge))
    }

    fn merge_join_by_ordering(i1: Vec<u8>, i2: Vec<u8>) -> () {
        test_specializations(&i1.into_iter().merge_join_by(i2, Ord::cmp));
    }

    fn merge_join_by_bool(i1: Vec<u8>, i2: Vec<u8>) -> () {
        test_specializations(&i1.into_iter().merge_join_by(i2, PartialOrd::ge));
    }

    fn kmerge(a: Vec<i8>, b: Vec<i8>, c: Vec<i8>) -> () {
        test_specializations(&vec![a, b, c]
            .into_iter()
            .map(|v| v.into_iter().sorted())
            .kmerge());
    }

    fn kmerge_by(a: Vec<i8>, b: Vec<i8>, c: Vec<i8>) -> () {
        test_specializations(&vec![a, b, c]
            .into_iter()
            .map(|v| v.into_iter().sorted_by_key(|a| a.abs()))
            .kmerge_by(|a, b| a.abs() < b.abs()));
    }
}

quickcheck! {
    fn map_into(v: Vec<u8>) -> () {
        let it = v.into_iter().map_into::<u32>();
        test_specializations(&it);
        test_double_ended_specializations(&it);
    }

    fn map_ok(v: Vec<Result<u8, char>>) -> () {
        let it = v.into_iter().map_ok(|u| u.checked_add(1));
        test_specializations(&it);
        test_double_ended_specializations(&it);
    }

    fn filter_ok(v: Vec<Result<u8, char>>) -> () {
        test_specializations(&v.into_iter().filter_ok(|&i| i < 20));
    }

    fn filter_map_ok(v: Vec<Result<u8, char>>) -> () {
        test_specializations(&v.into_iter().filter_map_ok(|i| if i < 20 { Some(i * 2) } else { None }));
    }

    // `SmallIter2<u8>` because `Vec<u8>` is too slow and we get bad coverage from a singleton like Option<u8>
    fn flatten_ok(v: Vec<Result<SmallIter2<u8>, char>>) -> () {
        let it = v.into_iter().flatten_ok();
        test_specializations(&it);
        test_double_ended_specializations(&it);
    }
}

quickcheck! {
    // TODO Replace this function by a normal call to test_specializations
    fn process_results(v: Vec<Result<u8, u8>>) -> () {
        helper(v.iter().copied());
        helper(v.iter().copied().filter(Result::is_ok));

        fn helper(it: impl DoubleEndedIterator<Item = Result<u8, u8>> + Clone) {
            macro_rules! check_results_specialized {
                ($src:expr, |$it:pat| $closure:expr) => {
                    assert_eq!(
                        itertools::process_results($src.clone(), |$it| $closure),
                        itertools::process_results($src.clone(), |i| {
                            let $it = Unspecialized(i);
                            $closure
                        }),
                    )
                }
            }

            check_results_specialized!(it, |i| i.count());
            check_results_specialized!(it, |i| i.last());
            check_results_specialized!(it, |i| i.collect::<Vec<_>>());
            check_results_specialized!(it, |i| i.rev().collect::<Vec<_>>());
            check_results_specialized!(it, |i| {
                let mut parameters_from_fold = vec![];
                let fold_result = i.fold(vec![], |mut acc, v| {
                    parameters_from_fold.push((acc.clone(), v));
                    acc.push(v);
                    acc
                });
                (parameters_from_fold, fold_result)
            });
            check_results_specialized!(it, |i| {
                let mut parameters_from_rfold = vec![];
                let rfold_result = i.rfold(vec![], |mut acc, v| {
                    parameters_from_rfold.push((acc.clone(), v));
                    acc.push(v);
                    acc
                });
                (parameters_from_rfold, rfold_result)
            });
            check_results_specialized!(it, |mut i| {
                let mut parameters_from_all = vec![];
                let first = i.next();
                let all_result = i.all(|x| {
                    parameters_from_all.push(x);
                    Some(x)==first
                });
                (parameters_from_all, all_result)
            });
            let size = it.clone().count();
            for n in 0..size + 2 {
                check_results_specialized!(it, |mut i| i.nth(n));
            }
            for n in 0..size + 2 {
                check_results_specialized!(it, |mut i| i.nth_back(n));
            }
        }
    }
}

/// Like `VecIntoIter<T>` with maximum 2 elements.
#[derive(Debug, Clone, Default)]
enum SmallIter2<T> {
    #[default]
    Zero,
    One(T),
    Two(T, T),
}

impl<T: Arbitrary> Arbitrary for SmallIter2<T> {
    fn arbitrary<G: quickcheck::Gen>(g: &mut G) -> Self {
        match g.gen_range(0u8, 3) {
            0 => Self::Zero,
            1 => Self::One(T::arbitrary(g)),
            2 => Self::Two(T::arbitrary(g), T::arbitrary(g)),
            _ => unreachable!(),
        }
    }
    // maybe implement shrink too, maybe not
}

impl<T> Iterator for SmallIter2<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        match std::mem::take(self) {
            Self::Zero => None,
            Self::One(val) => Some(val),
            Self::Two(val, second) => {
                *self = Self::One(second);
                Some(val)
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = match self {
            Self::Zero => 0,
            Self::One(_) => 1,
            Self::Two(_, _) => 2,
        };
        (len, Some(len))
    }
}

impl<T> DoubleEndedIterator for SmallIter2<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        match std::mem::take(self) {
            Self::Zero => None,
            Self::One(val) => Some(val),
            Self::Two(first, val) => {
                *self = Self::One(first);
                Some(val)
            }
        }
    }
}
