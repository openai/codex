//! The purpose of these tests is to cover corner cases of iterators
//! and adaptors.
//!
//! In particular we test the tedious size_hint and exact size correctness.
//!
//! **NOTE:** Due to performance limitations, these tests are not run with miri!
//! They cannot be relied upon to discover soundness issues.

#![cfg(not(miri))]
#![allow(deprecated, unstable_name_collisions)]

use itertools::free::{
    cloned, enumerate, multipeek, peek_nth, put_back, put_back_n, rciter, zip, zip_eq,
};
use itertools::Itertools;
use itertools::{iproduct, izip, multizip, EitherOrBoth};
use quickcheck as qc;
use std::cmp::{max, min, Ordering};
use std::collections::{HashMap, HashSet};
use std::default::Default;
use std::num::Wrapping;
use std::ops::Range;

use quickcheck::TestResult;
use rand::seq::SliceRandom;
use rand::Rng;

/// Trait for size hint modifier types
trait HintKind: Copy + Send + qc::Arbitrary {
    fn loosen_bounds(&self, org_hint: (usize, Option<usize>)) -> (usize, Option<usize>);
}

/// Exact size hint variant that leaves hints unchanged
#[derive(Clone, Copy, Debug)]
struct Exact {}

impl HintKind for Exact {
    fn loosen_bounds(&self, org_hint: (usize, Option<usize>)) -> (usize, Option<usize>) {
        org_hint
    }
}

impl qc::Arbitrary for Exact {
    fn arbitrary<G: qc::Gen>(_: &mut G) -> Self {
        Self {}
    }
}

/// Inexact size hint variant to simulate imprecise (but valid) size hints
///
/// Will always decrease the lower bound and increase the upper bound
/// of the size hint by set amounts.
#[derive(Clone, Copy, Debug)]
struct Inexact {
    underestimate: usize,
    overestimate: usize,
}

impl HintKind for Inexact {
    fn loosen_bounds(&self, org_hint: (usize, Option<usize>)) -> (usize, Option<usize>) {
        let (org_lower, org_upper) = org_hint;
        (
            org_lower.saturating_sub(self.underestimate),
            org_upper.and_then(move |x| x.checked_add(self.overestimate)),
        )
    }
}

impl qc::Arbitrary for Inexact {
    fn arbitrary<G: qc::Gen>(g: &mut G) -> Self {
        let ue_value = usize::arbitrary(g);
        let oe_value = usize::arbitrary(g);
        // Compensate for quickcheck using extreme values too rarely
        let ue_choices = &[0, ue_value, usize::MAX];
        let oe_choices = &[0, oe_value, usize::MAX];
        Self {
            underestimate: *ue_choices.choose(g).unwrap(),
            overestimate: *oe_choices.choose(g).unwrap(),
        }
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
        let underestimate_value = self.underestimate;
        let overestimate_value = self.overestimate;
        Box::new(underestimate_value.shrink().flat_map(move |ue_value| {
            overestimate_value.shrink().map(move |oe_value| Self {
                underestimate: ue_value,
                overestimate: oe_value,
            })
        }))
    }
}

/// Our base iterator that we can impl Arbitrary for
///
/// By default we'll return inexact bounds estimates for size_hint
/// to make tests harder to pass.
///
/// NOTE: Iter is tricky and is not fused, to help catch bugs.
/// At the end it will return None once, then return Some(0),
/// then return None again.
#[derive(Clone, Debug)]
struct Iter<T, SK: HintKind = Inexact> {
    iterator: Range<T>,
    // fuse/done flag
    fuse_flag: i32,
    hint_kind: SK,
}

impl<T, HK> Iter<T, HK>
where
    HK: HintKind,
{
    fn new(it: Range<T>, hint_kind: HK) -> Self {
        Self {
            iterator: it,
            fuse_flag: 0,
            hint_kind,
        }
    }
}

impl<T, HK> Iterator for Iter<T, HK>
where
    Range<T>: Iterator,
    <Range<T> as Iterator>::Item: Default,
    HK: HintKind,
{
    type Item = <Range<T> as Iterator>::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let elt = self.iterator.next();
        if elt.is_none() {
            self.fuse_flag += 1;
            // check fuse flag
            if self.fuse_flag == 2 {
                return Some(Default::default());
            }
        }
        elt
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let org_hint = self.iterator.size_hint();
        self.hint_kind.loosen_bounds(org_hint)
    }
}

impl<T, HK> DoubleEndedIterator for Iter<T, HK>
where
    Range<T>: DoubleEndedIterator,
    <Range<T> as Iterator>::Item: Default,
    HK: HintKind,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iterator.next_back()
    }
}

impl<T> ExactSizeIterator for Iter<T, Exact>
where
    Range<T>: ExactSizeIterator,
    <Range<T> as Iterator>::Item: Default,
{
}

impl<T, HK> qc::Arbitrary for Iter<T, HK>
where
    T: qc::Arbitrary,
    HK: HintKind,
{
    fn arbitrary<G: qc::Gen>(g: &mut G) -> Self {
        Self::new(T::arbitrary(g)..T::arbitrary(g), HK::arbitrary(g))
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
        let r = self.iterator.clone();
        let hint_kind = self.hint_kind;
        Box::new(r.start.shrink().flat_map(move |a| {
            r.end
                .shrink()
                .map(move |b| Self::new(a.clone()..b, hint_kind))
        }))
    }
}

/// A meta-iterator which yields `Iter<i32>`s whose start/endpoints are
/// increased or decreased linearly on each iteration.
#[derive(Clone, Debug)]
struct ShiftRange<HK = Inexact> {
    range_start: i32,
    range_end: i32,
    start_step: i32,
    end_step: i32,
    iter_count: u32,
    hint_kind: HK,
}

impl<HK> Iterator for ShiftRange<HK>
where
    HK: HintKind,
{
    type Item = Iter<i32, HK>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.iter_count == 0 {
            return None;
        }

        let iter = Iter::new(self.range_start..self.range_end, self.hint_kind);

        self.range_start += self.start_step;
        self.range_end += self.end_step;
        self.iter_count -= 1;

        Some(iter)
    }
}

impl ExactSizeIterator for ShiftRange<Exact> {}

impl<HK> qc::Arbitrary for ShiftRange<HK>
where
    HK: HintKind,
{
    fn arbitrary<G: qc::Gen>(g: &mut G) -> Self {
        const MAX_STARTING_RANGE_DIFF: i32 = 32;
        const MAX_STEP_MODULO: i32 = 8;
        const MAX_ITER_COUNT: u32 = 3;

        let range_start = qc::Arbitrary::arbitrary(g);
        let range_end = range_start + g.gen_range(0, MAX_STARTING_RANGE_DIFF + 1);
        let start_step = g.gen_range(-MAX_STEP_MODULO, MAX_STEP_MODULO + 1);
        let end_step = g.gen_range(-MAX_STEP_MODULO, MAX_STEP_MODULO + 1);
        let iter_count = g.gen_range(0, MAX_ITER_COUNT + 1);
        let hint_kind = qc::Arbitrary::arbitrary(g);

        Self {
            range_start,
            range_end,
            start_step,
            end_step,
            iter_count,
            hint_kind,
        }
    }
}

fn correct_count<I, F>(get_it: F) -> bool
where
    I: Iterator,
    F: Fn() -> I,
{
    let mut counts = vec![get_it().count()];

    'outer: loop {
        let mut it = get_it();

        for _ in 0..(counts.len() - 1) {
            if it.next().is_none() {
                panic!("Iterator shouldn't be finished, may not be deterministic");
            }
        }

        if it.next().is_none() {
            break 'outer;
        }

        counts.push(it.count());
    }

    let total_actual_count = counts.len() - 1;

    for (i, returned_count) in counts.into_iter().enumerate() {
        let actual_count = total_actual_count - i;
        if actual_count != returned_count {
            println!(
                "Total iterations: {} True count: {} returned count: {}",
                i, actual_count, returned_count
            );

            return false;
        }
    }

    true
}

fn correct_size_hint<I: Iterator>(mut it: I) -> bool {
    // record size hint at each iteration
    let initial_hint = it.size_hint();
    let mut hints = Vec::with_capacity(initial_hint.0 + 1);
    hints.push(initial_hint);
    while let Some(_) = it.next() {
        hints.push(it.size_hint())
    }

    let mut true_count = hints.len(); // start off +1 too much

    // check all the size hints
    for &(low, hi) in &hints {
        true_count -= 1;
        if low > true_count || (hi.is_some() && hi.unwrap() < true_count) {
            println!("True size: {:?}, size hint: {:?}", true_count, (low, hi));
            //println!("All hints: {:?}", hints);
            return false;
        }
    }
    true
}

fn exact_size<I: ExactSizeIterator>(mut it: I) -> bool {
    // check every iteration
    let (mut low, mut hi) = it.size_hint();
    if Some(low) != hi {
        return false;
    }
    while let Some(_) = it.next() {
        let (xlow, xhi) = it.size_hint();
        if low != xlow + 1 {
            return false;
        }
        low = xlow;
        hi = xhi;
        if Some(low) != hi {
            return false;
        }
    }
    let (low, hi) = it.size_hint();
    low == 0 && hi == Some(0)
}

// Exact size for this case, without ExactSizeIterator
fn exact_size_for_this<I: Iterator>(mut it: I) -> bool {
    // check every iteration
    let (mut low, mut hi) = it.size_hint();
    if Some(low) != hi {
        return false;
    }
    while let Some(_) = it.next() {
        let (xlow, xhi) = it.size_hint();
        if low != xlow + 1 {
            return false;
        }
        low = xlow;
        hi = xhi;
        if Some(low) != hi {
            return false;
        }
    }
    let (low, hi) = it.size_hint();
    low == 0 && hi == Some(0)
}

/*
 * NOTE: Range<i8> is broken!
 * (all signed ranges are)
#[quickcheck]
fn size_range_i8(a: Iter<i8>) -> bool {
    exact_size(a)
}

#[quickcheck]
fn size_range_i16(a: Iter<i16>) -> bool {
    exact_size(a)
}

#[quickcheck]
fn size_range_u8(a: Iter<u8>) -> bool {
    exact_size(a)
}
 */

macro_rules! quickcheck {
    // accept several property function definitions
    // The property functions can use pattern matching and `mut` as usual
    // in the function arguments, but the functions can not be generic.
    {$($(#$attr:tt)* fn $fn_name:ident($($arg:tt)*) -> $ret:ty { $($code:tt)* })*} => (
        $(
            #[test]
            $(#$attr)*
            fn $fn_name() {
                fn prop($($arg)*) -> $ret {
                    $($code)*
                }
                ::quickcheck::quickcheck(quickcheck!(@fn prop [] $($arg)*));
            }
        )*
    );
    // parse argument list (with patterns allowed) into prop as fn(_, _) -> _
    (@fn $f:ident [$($t:tt)*]) => {
        $f as fn($($t),*) -> _
    };
    (@fn $f:ident [$($p:tt)*] : $($tail:tt)*) => {
        quickcheck!(@fn $f [$($p)* _] $($tail)*)
    };
    (@fn $f:ident [$($p:tt)*] $t:tt $($tail:tt)*) => {
        quickcheck!(@fn $f [$($p)*] $($tail)*)
    };
}

quickcheck! {

    fn size_product(a: Iter<u16>, b: Iter<u16>) -> bool {
        correct_size_hint(a.cartesian_product(b))
    }
    fn size_product3(a: Iter<u16>, b: Iter<u16>, c: Iter<u16>) -> bool {
        correct_size_hint(iproduct!(a, b, c))
    }

    fn correct_cartesian_product3(a: Iter<u16>, b: Iter<u16>, c: Iter<u16>,
                                  take_manual: usize) -> ()
    {
        // test correctness of iproduct through regular iteration (take)
        // and through fold.
        let ac = a.clone();
        let br = &b.clone();
        let cr = &c.clone();
        let answer: Vec<_> = ac.flat_map(move |ea| br.clone().flat_map(move |eb| cr.clone().map(move |ec| (ea, eb, ec)))).collect();
        let mut product_iter = iproduct!(a, b, c);
        let mut actual = Vec::new();

        actual.extend((&mut product_iter).take(take_manual));
        if actual.len() == take_manual {
            product_iter.fold((), |(), elt| actual.push(elt));
        }
        assert_eq!(answer, actual);
    }

    fn size_multi_product(a: ShiftRange) -> bool {
        correct_size_hint(a.multi_cartesian_product())
    }
    fn correct_multi_product3(a: ShiftRange, take_manual: usize) -> () {
        // Fix no. of iterators at 3
        let a = ShiftRange { iter_count: 3, ..a };

        // test correctness of MultiProduct through regular iteration (take)
        // and through fold.
        let mut iters = a.clone();
        let i0 = iters.next().unwrap();
        let i1r = &iters.next().unwrap();
        let i2r = &iters.next().unwrap();
        let answer: Vec<_> = i0.flat_map(move |ei0| i1r.clone().flat_map(move |ei1| i2r.clone().map(move |ei2| vec![ei0, ei1, ei2]))).collect();
        let mut multi_product = a.clone().multi_cartesian_product();
        let mut actual = Vec::new();

        actual.extend((&mut multi_product).take(take_manual));
        if actual.len() == take_manual {
            multi_product.fold((), |(), elt| actual.push(elt));
        }
        assert_eq!(answer, actual);

        assert_eq!(answer.into_iter().last(), a.multi_cartesian_product().last());
    }

    fn correct_empty_multi_product() -> () {
        let empty = Vec::<std::vec::IntoIter<i32>>::new().into_iter().multi_cartesian_product();
        assert!(correct_size_hint(empty.clone()));
        itertools::assert_equal(empty, std::iter::once(Vec::new()))
    }

    fn size_multipeek(a: Iter<u16, Exact>, s: u8) -> bool {
        let mut it = multipeek(a);
        // peek a few times
        for _ in 0..s {
            it.peek();
        }
        exact_size(it)
    }

    fn size_peek_nth(a: Iter<u16, Exact>, s: u8) -> bool {
        let mut it = peek_nth(a);
        // peek a few times
        for n in 0..s {
            it.peek_nth(n as usize);
        }
        exact_size(it)
    }

    fn equal_merge(mut a: Vec<i16>, mut b: Vec<i16>) -> bool {
        a.sort();
        b.sort();
        let mut merged = a.clone();
        merged.extend(b.iter().cloned());
        merged.sort();
        itertools::equal(&merged, a.iter().merge(&b))
    }
    fn size_merge(a: Iter<u16>, b: Iter<u16>) -> bool {
        correct_size_hint(a.merge(b))
    }
    fn size_zip(a: Iter<i16, Exact>, b: Iter<i16, Exact>, c: Iter<i16, Exact>) -> bool {
        let filt = a.clone().dedup();
        correct_size_hint(multizip((filt, b.clone(), c.clone()))) &&
            exact_size(multizip((a, b, c)))
    }
    fn size_zip_rc(a: Iter<i16>, b: Iter<i16>) -> bool {
        let rc = rciter(a);
        correct_size_hint(multizip((&rc, &rc, b)))
    }

    fn size_zip_macro(a: Iter<i16, Exact>, b: Iter<i16, Exact>, c: Iter<i16, Exact>) -> bool {
        let filt = a.clone().dedup();
        correct_size_hint(izip!(filt, b.clone(), c.clone())) &&
            exact_size(izip!(a, b, c))
    }
    fn equal_kmerge(mut a: Vec<i16>, mut b: Vec<i16>, mut c: Vec<i16>) -> bool {
        use itertools::free::kmerge;
        a.sort();
        b.sort();
        c.sort();
        let mut merged = a.clone();
        merged.extend(b.iter().cloned());
        merged.extend(c.iter().cloned());
        merged.sort();
        itertools::equal(merged.into_iter(), kmerge(vec![a, b, c]))
    }

    // Any number of input iterators
    fn equal_kmerge_2(mut inputs: Vec<Vec<i16>>) -> bool {
        use itertools::free::kmerge;
        // sort the inputs
        for input in &mut inputs {
            input.sort();
        }
        let mut merged = inputs.concat();
        merged.sort();
        itertools::equal(merged.into_iter(), kmerge(inputs))
    }

    // Any number of input iterators
    fn equal_kmerge_by_ge(mut inputs: Vec<Vec<i16>>) -> bool {
        // sort the inputs
        for input in &mut inputs {
            input.sort();
            input.reverse();
        }
        let mut merged = inputs.concat();
        merged.sort();
        merged.reverse();
        itertools::equal(merged.into_iter(),
                         inputs.into_iter().kmerge_by(|x, y| x >= y))
    }

    // Any number of input iterators
    fn equal_kmerge_by_lt(mut inputs: Vec<Vec<i16>>) -> bool {
        // sort the inputs
        for input in &mut inputs {
            input.sort();
        }
        let mut merged = inputs.concat();
        merged.sort();
        itertools::equal(merged.into_iter(),
                         inputs.into_iter().kmerge_by(|x, y| x < y))
    }

    // Any number of input iterators
    fn equal_kmerge_by_le(mut inputs: Vec<Vec<i16>>) -> bool {
        // sort the inputs
        for input in &mut inputs {
            input.sort();
        }
        let mut merged = inputs.concat();
        merged.sort();
        itertools::equal(merged.into_iter(),
                         inputs.into_iter().kmerge_by(|x, y| x <= y))
    }
    fn size_kmerge(a: Iter<i16>, b: Iter<i16>, c: Iter<i16>) -> bool {
        use itertools::free::kmerge;
        correct_size_hint(kmerge(vec![a, b, c]))
    }
    fn equal_zip_eq(a: Vec<i32>, b: Vec<i32>) -> bool {
        let len = std::cmp::min(a.len(), b.len());
        let a = &a[..len];
        let b = &b[..len];
        itertools::equal(zip_eq(a, b), zip(a, b))
    }

    #[should_panic]
    fn zip_eq_panics(a: Vec<u8>, b: Vec<u8>) -> TestResult {
        if a.len() == b.len() { return TestResult::discard(); }
        zip_eq(a.iter(), b.iter()).for_each(|_| {});
        TestResult::passed() // won't come here
    }

    fn equal_positions(a: Vec<i32>) -> bool {
        let with_pos = a.iter().positions(|v| v % 2 == 0);
        let without = a.iter().enumerate().filter(|(_, v)| *v % 2 == 0).map(|(i, _)| i);
        itertools::equal(with_pos.clone(), without.clone())
            && itertools::equal(with_pos.rev(), without.rev())
    }
    fn size_zip_longest(a: Iter<i16, Exact>, b: Iter<i16, Exact>) -> bool {
        let filt = a.clone().dedup();
        let filt2 = b.clone().dedup();
        correct_size_hint(filt.zip_longest(b.clone())) &&
        correct_size_hint(a.clone().zip_longest(filt2)) &&
            exact_size(a.zip_longest(b))
    }
    fn size_2_zip_longest(a: Iter<i16>, b: Iter<i16>) -> bool {
        let it = a.clone().zip_longest(b.clone());
        let jt = a.clone().zip_longest(b.clone());
        itertools::equal(a,
                         it.filter_map(|elt| match elt {
                             EitherOrBoth::Both(x, _) => Some(x),
                             EitherOrBoth::Left(x) => Some(x),
                             _ => None,
                         }
                         ))
            &&
        itertools::equal(b,
                         jt.filter_map(|elt| match elt {
                             EitherOrBoth::Both(_, y) => Some(y),
                             EitherOrBoth::Right(y) => Some(y),
                             _ => None,
                         }
                         ))
    }
    fn size_interleave(a: Iter<i16>, b: Iter<i16>) -> bool {
        correct_size_hint(a.interleave(b))
    }
    fn exact_interleave(a: Iter<i16, Exact>, b: Iter<i16, Exact>) -> bool {
        exact_size_for_this(a.interleave(b))
    }
    fn size_interleave_shortest(a: Iter<i16>, b: Iter<i16>) -> bool {
        correct_size_hint(a.interleave_shortest(b))
    }
    fn exact_interleave_shortest(a: Vec<()>, b: Vec<()>) -> bool {
        exact_size_for_this(a.iter().interleave_shortest(&b))
    }
    fn size_intersperse(a: Iter<i16>, x: i16) -> bool {
        correct_size_hint(a.intersperse(x))
    }
    fn equal_intersperse(a: Vec<i32>, x: i32) -> bool {
        let mut inter = false;
        let mut i = 0;
        for elt in a.iter().cloned().intersperse(x) {
            if inter {
                if elt != x { return false }
            } else {
                if elt != a[i] { return false }
                i += 1;
            }
            inter = !inter;
        }
        true
    }

    fn equal_combinations_2(a: Vec<u8>) -> bool {
        let mut v = Vec::new();
        for (i, x) in enumerate(&a) {
            for y in &a[i + 1..] {
                v.push((x, y));
            }
        }
        itertools::equal(a.iter().tuple_combinations::<(_, _)>(), v)
    }

    fn collect_tuple_matches_size(a: Iter<i16>) -> bool {
        let size = a.clone().count();
        a.collect_tuple::<(_, _, _)>().is_some() == (size == 3)
    }

    fn correct_permutations(vals: HashSet<i32>, k: usize) -> () {
        // Test permutations only on iterators of distinct integers, to prevent
        // false positives.

        const MAX_N: usize = 5;

        let n = min(vals.len(), MAX_N);
        let vals: HashSet<i32> = vals.into_iter().take(n).collect();

        let perms = vals.iter().permutations(k);

        let mut actual = HashSet::new();

        for perm in perms {
            assert_eq!(perm.len(), k);

            let all_items_valid = perm.iter().all(|p| vals.contains(p));
            assert!(all_items_valid, "perm contains value not from input: {:?}", perm);

            // Check that all perm items are distinct
            let distinct_len = {
                let perm_set: HashSet<_> = perm.iter().collect();
                perm_set.len()
            };
            assert_eq!(perm.len(), distinct_len);

            // Check that the perm is new
            assert!(actual.insert(perm.clone()), "perm already encountered: {:?}", perm);
        }
    }

    fn permutations_lexic_order(a: usize, b: usize) -> () {
        let a = a % 6;
        let b = b % 6;

        let n = max(a, b);
        let k = min (a, b);

        let expected_first: Vec<usize> = (0..k).collect();
        let expected_last: Vec<usize> = ((n - k)..n).rev().collect();

        let mut perms = (0..n).permutations(k);

        let mut curr_perm = match perms.next() {
            Some(p) => p,
            None => { return; }
        };

        assert_eq!(expected_first, curr_perm);

        for next_perm in perms {
            assert!(
                next_perm > curr_perm,
                "next perm isn't greater-than current; next_perm={:?} curr_perm={:?} n={}",
                next_perm, curr_perm, n
            );

            curr_perm = next_perm;
        }

        assert_eq!(expected_last, curr_perm);

    }

    fn permutations_count(n: usize, k: usize) -> bool {
        let n = n % 6;

        correct_count(|| (0..n).permutations(k))
    }

    fn permutations_size(a: Iter<i32>, k: usize) -> bool {
        correct_size_hint(a.take(5).permutations(k))
    }

    fn permutations_k0_yields_once(n: usize) -> () {
        let k = 0;
        let expected: Vec<Vec<usize>> = vec![vec![]];
        let actual = (0..n).permutations(k).collect_vec();

        assert_eq!(expected, actual);
    }
}

quickcheck! {
    fn correct_peek_nth(mut a: Vec<u16>) -> () {
        let mut it = peek_nth(a.clone());
        for start_pos in 0..a.len() + 2 {
            for real_idx in start_pos..a.len() + 2 {
                let peek_idx = real_idx - start_pos;
                assert_eq!(it.peek_nth(peek_idx), a.get(real_idx));
                assert_eq!(it.peek_nth_mut(peek_idx), a.get_mut(real_idx));
            }
            assert_eq!(it.next(), a.get(start_pos).copied());
        }
    }

    fn peek_nth_mut_replace(a: Vec<u16>, b: Vec<u16>) -> () {
        let mut it = peek_nth(a.iter());
        for (i, m) in b.iter().enumerate().take(a.len().min(b.len())) {
            *it.peek_nth_mut(i).unwrap() = m;
        }
        for (i, m) in a.iter().enumerate() {
             assert_eq!(it.next().unwrap(), b.get(i).unwrap_or(m));
        }
        assert_eq!(it.next(), None);
        assert_eq!(it.next(), None);
    }

    fn peek_nth_next_if(a: Vec<u8>) -> () {
        let mut it = peek_nth(a.clone());
        for (idx, mut value) in a.iter().copied().enumerate() {
            let should_be_none = it.next_if(|x| x != &value);
            assert_eq!(should_be_none, None);
            if value % 5 == 0 {
                // Sometimes, peek up to 3 further.
                let n = value as usize % 3;
                let nth = it.peek_nth(n);
                assert_eq!(nth, a.get(idx + n));
            } else if value % 5 == 1 {
                // Sometimes, peek next element mutably.
                if let Some(v) = it.peek_mut() {
                    *v = v.wrapping_sub(1);
                    let should_be_none = it.next_if_eq(&value);
                    assert_eq!(should_be_none, None);
                    value = value.wrapping_sub(1);
                }
            }
            let eq = it.next_if_eq(&value);
            assert_eq!(eq, Some(value));
        }
    }
}

quickcheck! {
    fn dedup_via_coalesce(a: Vec<i32>) -> bool {
        let mut b = a.clone();
        b.dedup();
        itertools::equal(
            &b,
            a
                .iter()
                .coalesce(|x, y| {
                    if x==y {
                        Ok(x)
                    } else {
                        Err((x, y))
                    }
                })
                .fold(vec![], |mut v, n| {
                    v.push(n);
                    v
                })
        )
    }
}

quickcheck! {
    fn equal_dedup(a: Vec<i32>) -> bool {
        let mut b = a.clone();
        b.dedup();
        itertools::equal(&b, a.iter().dedup())
    }
}

quickcheck! {
    fn equal_dedup_by(a: Vec<(i32, i32)>) -> bool {
        let mut b = a.clone();
        b.dedup_by(|x, y| x.0==y.0);
        itertools::equal(&b, a.iter().dedup_by(|x, y| x.0==y.0))
    }
}

quickcheck! {
    fn size_dedup(a: Vec<i32>) -> bool {
        correct_size_hint(a.iter().dedup())
    }
}

quickcheck! {
    fn size_dedup_by(a: Vec<(i32, i32)>) -> bool {
        correct_size_hint(a.iter().dedup_by(|x, y| x.0==y.0))
    }
}

quickcheck! {
    fn exact_repeatn((n, x): (usize, i32)) -> bool {
        let it = itertools::repeat_n(x, n);
        exact_size(it)
    }
}

quickcheck! {
    fn size_put_back(a: Vec<u8>, x: Option<u8>) -> bool {
        let mut it = put_back(a.into_iter());
        if let Some(t) = x {
            it.put_back(t);
        }
        correct_size_hint(it)
    }
}

quickcheck! {
    fn size_put_backn(a: Vec<u8>, b: Vec<u8>) -> bool {
        let mut it = put_back_n(a.into_iter());
        for elt in b {
            it.put_back(elt)
        }
        correct_size_hint(it)
    }
}

quickcheck! {
    fn merge_join_by_ordering_vs_bool(a: Vec<u8>, b: Vec<u8>) -> bool {
        use either::Either;
        use itertools::free::merge_join_by;
        let mut has_equal = false;
        let it_ord = merge_join_by(a.clone(), b.clone(), Ord::cmp).flat_map(|v| match v {
            EitherOrBoth::Both(l, r) => {
                has_equal = true;
                vec![Either::Left(l), Either::Right(r)]
            }
            EitherOrBoth::Left(l) => vec![Either::Left(l)],
            EitherOrBoth::Right(r) => vec![Either::Right(r)],
        });
        let it_bool = merge_join_by(a, b, PartialOrd::le);
        itertools::equal(it_ord, it_bool) || has_equal
    }
    fn merge_join_by_bool_unwrapped_is_merge_by(a: Vec<u8>, b: Vec<u8>) -> bool {
        use either::Either;
        use itertools::free::merge_join_by;
        let it = a.clone().into_iter().merge_by(b.clone(), PartialOrd::ge);
        let it_join = merge_join_by(a, b, PartialOrd::ge).map(Either::into_inner);
        itertools::equal(it, it_join)
    }
}

quickcheck! {
    fn size_tee(a: Vec<u8>) -> bool {
        let (mut t1, mut t2) = a.iter().tee();
        t1.next();
        t1.next();
        t2.next();
        exact_size(t1) && exact_size(t2)
    }
}

quickcheck! {
    fn size_tee_2(a: Vec<u8>) -> bool {
        let (mut t1, mut t2) = a.iter().dedup().tee();
        t1.next();
        t1.next();
        t2.next();
        correct_size_hint(t1) && correct_size_hint(t2)
    }
}

quickcheck! {
    fn size_take_while_ref(a: Vec<u8>, stop: u8) -> bool {
        correct_size_hint(a.iter().take_while_ref(|x| **x != stop))
    }
}

quickcheck! {
    fn equal_partition(a: Vec<i32>) -> bool {
        let mut a = a;
        let mut ap = a.clone();
        let split_index = itertools::partition(&mut ap, |x| *x >= 0);
        let parted = (0..split_index).all(|i| ap[i] >= 0) &&
            (split_index..a.len()).all(|i| ap[i] < 0);

        a.sort();
        ap.sort();
        parted && (a == ap)
    }
}

quickcheck! {
    fn size_combinations(a: Iter<i16>) -> bool {
        let it = a.clone().tuple_combinations::<(_, _)>();
        correct_size_hint(it.clone()) && it.count() == binomial(a.count(), 2)
    }

    fn exact_size_combinations_1(a: Vec<u8>) -> bool {
        let it = a.iter().tuple_combinations::<(_,)>();
        exact_size_for_this(it.clone()) && it.count() == binomial(a.len(), 1)
    }
    fn exact_size_combinations_2(a: Vec<u8>) -> bool {
        let it = a.iter().tuple_combinations::<(_, _)>();
        exact_size_for_this(it.clone()) && it.count() == binomial(a.len(), 2)
    }
    fn exact_size_combinations_3(mut a: Vec<u8>) -> bool {
        a.truncate(15);
        let it = a.iter().tuple_combinations::<(_, _, _)>();
        exact_size_for_this(it.clone()) && it.count() == binomial(a.len(), 3)
    }
}

fn binomial(n: usize, k: usize) -> usize {
    if k > n {
        0
    } else {
        (n - k + 1..=n).product::<usize>() / (1..=k).product::<usize>()
    }
}

quickcheck! {
    fn equal_combinations(it: Iter<i16>) -> bool {
        let values = it.clone().collect_vec();
        let mut cmb = it.tuple_combinations();
        for i in 0..values.len() {
            for j in i+1..values.len() {
                let pair = (values[i], values[j]);
                if pair != cmb.next().unwrap() {
                    return false;
                }
            }
        }
        cmb.next().is_none()
    }
}

quickcheck! {
    fn size_pad_tail(it: Iter<i8>, pad: u8) -> bool {
        correct_size_hint(it.clone().pad_using(pad as usize, |_| 0)) &&
            correct_size_hint(it.dropping(1).rev().pad_using(pad as usize, |_| 0))
    }
}

quickcheck! {
    fn size_pad_tail2(it: Iter<i8, Exact>, pad: u8) -> bool {
        exact_size(it.pad_using(pad as usize, |_| 0))
    }
}

quickcheck! {
    fn size_powerset(it: Iter<u8, Exact>) -> bool {
        // Powerset cardinality gets large very quickly, limit input to keep test fast.
        correct_size_hint(it.take(12).powerset())
    }
}

quickcheck! {
    fn size_duplicates(it: Iter<i8>) -> bool {
        correct_size_hint(it.duplicates())
    }
}

quickcheck! {
    fn size_unique(it: Iter<i8>) -> bool {
        correct_size_hint(it.unique())
    }

    fn count_unique(it: Vec<i8>, take_first: u8) -> () {
        let answer = {
            let mut v = it.clone();
            v.sort(); v.dedup();
            v.len()
        };
        let mut iter = cloned(&it).unique();
        let first_count = (&mut iter).take(take_first as usize).count();
        let rest_count = iter.count();
        assert_eq!(answer, first_count + rest_count);
    }
}

quickcheck! {
    fn fuzz_chunk_by_lazy_1(it: Iter<u8>) -> bool {
        let jt = it.clone();
        let chunks = it.chunk_by(|k| *k);
        itertools::equal(jt, chunks.into_iter().flat_map(|(_, x)| x))
    }
}

quickcheck! {
    fn fuzz_chunk_by_lazy_2(data: Vec<u8>) -> bool {
        let chunks = data.iter().chunk_by(|k| *k / 10);
        let res = itertools::equal(data.iter(), chunks.into_iter().flat_map(|(_, x)| x));
        res
    }
}

quickcheck! {
    fn fuzz_chunk_by_lazy_3(data: Vec<u8>) -> bool {
        let grouper = data.iter().chunk_by(|k| *k / 10);
        let chunks = grouper.into_iter().collect_vec();
        let res = itertools::equal(data.iter(), chunks.into_iter().flat_map(|(_, x)| x));
        res
    }
}

quickcheck! {
    fn fuzz_chunk_by_lazy_duo(data: Vec<u8>, order: Vec<(bool, bool)>) -> bool {
        let grouper = data.iter().chunk_by(|k| *k / 3);
        let mut chunks1 = grouper.into_iter();
        let mut chunks2 = grouper.into_iter();
        let mut elts = Vec::<&u8>::new();
        let mut old_chunks = Vec::new();

        let tup1 = |(_, b)| b;
        for &(ord, consume_now) in &order {
            let iter = &mut [&mut chunks1, &mut chunks2][ord as usize];
            match iter.next() {
                Some((_, gr)) => if consume_now {
                    for og in old_chunks.drain(..) {
                        elts.extend(og);
                    }
                    elts.extend(gr);
                } else {
                    old_chunks.push(gr);
                },
                None => break,
            }
        }
        for og in old_chunks.drain(..) {
            elts.extend(og);
        }
        for gr in chunks1.map(&tup1) { elts.extend(gr); }
        for gr in chunks2.map(&tup1) { elts.extend(gr); }
        itertools::assert_equal(&data, elts);
        true
    }
}

quickcheck! {
    fn chunk_clone_equal(a: Vec<u8>, size: u8) -> () {
        let mut size = size;
        if size == 0 {
            size += 1;
        }
        let it = a.chunks(size as usize);
        itertools::assert_equal(it.clone(), it);
    }
}

quickcheck! {
    fn equal_chunks_lazy(a: Vec<u8>, size: u8) -> bool {
        let mut size = size;
        if size == 0 {
            size += 1;
        }
        let chunks = a.iter().chunks(size as usize);
        let it = a.chunks(size as usize);
        for (a, b) in chunks.into_iter().zip(it) {
            if !itertools::equal(a, b) {
                return false;
            }
        }
        true
    }
}

// tuple iterators
quickcheck! {
    fn equal_circular_tuple_windows_1(a: Vec<u8>) -> bool {
        let x = a.iter().map(|e| (e,) );
        let y = a.iter().circular_tuple_windows::<(_,)>();
        itertools::assert_equal(x,y);
        true
    }

    fn equal_circular_tuple_windows_2(a: Vec<u8>) -> bool {
        let x = (0..a.len()).map(|start_idx| (
            &a[start_idx],
            &a[(start_idx + 1) % a.len()],
        ));
        let y = a.iter().circular_tuple_windows::<(_, _)>();
        itertools::assert_equal(x,y);
        true
    }

    fn equal_circular_tuple_windows_3(a: Vec<u8>) -> bool {
        let x = (0..a.len()).map(|start_idx| (
            &a[start_idx],
            &a[(start_idx + 1) % a.len()],
            &a[(start_idx + 2) % a.len()],
        ));
        let y = a.iter().circular_tuple_windows::<(_, _, _)>();
        itertools::assert_equal(x,y);
        true
    }

    fn equal_circular_tuple_windows_4(a: Vec<u8>) -> bool {
        let x = (0..a.len()).map(|start_idx| (
            &a[start_idx],
            &a[(start_idx + 1) % a.len()],
            &a[(start_idx + 2) % a.len()],
            &a[(start_idx + 3) % a.len()],
        ));
        let y = a.iter().circular_tuple_windows::<(_, _, _, _)>();
        itertools::assert_equal(x,y);
        true
    }

    fn equal_cloned_circular_tuple_windows(a: Vec<u8>) -> bool {
        let x = a.iter().circular_tuple_windows::<(_, _, _, _)>();
        let y = x.clone();
        itertools::assert_equal(x,y);
        true
    }

    fn equal_cloned_circular_tuple_windows_noninitial(a: Vec<u8>) -> bool {
        let mut x = a.iter().circular_tuple_windows::<(_, _, _, _)>();
        let _ = x.next();
        let y = x.clone();
        itertools::assert_equal(x,y);
        true
    }

    fn equal_cloned_circular_tuple_windows_complete(a: Vec<u8>) -> bool {
        let mut x = a.iter().circular_tuple_windows::<(_, _, _, _)>();
        for _ in x.by_ref() {}
        let y = x.clone();
        itertools::assert_equal(x,y);
        true
    }

    fn circular_tuple_windows_exact_size(a: Vec<u8>) -> bool {
        exact_size(a.iter().circular_tuple_windows::<(_, _, _, _)>())
    }

    fn equal_tuple_windows_1(a: Vec<u8>) -> bool {
        let x = a.windows(1).map(|s| (&s[0], ));
        let y = a.iter().tuple_windows::<(_,)>();
        itertools::equal(x, y)
    }

    fn equal_tuple_windows_2(a: Vec<u8>) -> bool {
        let x = a.windows(2).map(|s| (&s[0], &s[1]));
        let y = a.iter().tuple_windows::<(_, _)>();
        itertools::equal(x, y)
    }

    fn equal_tuple_windows_3(a: Vec<u8>) -> bool {
        let x = a.windows(3).map(|s| (&s[0], &s[1], &s[2]));
        let y = a.iter().tuple_windows::<(_, _, _)>();
        itertools::equal(x, y)
    }

    fn equal_tuple_windows_4(a: Vec<u8>) -> bool {
        let x = a.windows(4).map(|s| (&s[0], &s[1], &s[2], &s[3]));
        let y = a.iter().tuple_windows::<(_, _, _, _)>();
        itertools::equal(x, y)
    }

    fn tuple_windows_exact_size_1(a: Vec<u8>) -> bool {
        exact_size(a.iter().tuple_windows::<(_,)>())
    }

    fn tuple_windows_exact_size_4(a: Vec<u8>) -> bool {
        exact_size(a.iter().tuple_windows::<(_, _, _, _)>())
    }

    fn equal_tuples_1(a: Vec<u8>) -> bool {
        let x = a.chunks(1).map(|s| (&s[0], ));
        let y = a.iter().tuples::<(_,)>();
        itertools::equal(x, y)
    }

    fn equal_tuples_2(a: Vec<u8>) -> bool {
        let x = a.chunks(2).filter(|s| s.len() == 2).map(|s| (&s[0], &s[1]));
        let y = a.iter().tuples::<(_, _)>();
        itertools::equal(x, y)
    }

    fn equal_tuples_3(a: Vec<u8>) -> bool {
        let x = a.chunks(3).filter(|s| s.len() == 3).map(|s| (&s[0], &s[1], &s[2]));
        let y = a.iter().tuples::<(_, _, _)>();
        itertools::equal(x, y)
    }

    fn equal_tuples_4(a: Vec<u8>) -> bool {
        let x = a.chunks(4).filter(|s| s.len() == 4).map(|s| (&s[0], &s[1], &s[2], &s[3]));
        let y = a.iter().tuples::<(_, _, _, _)>();
        itertools::equal(x, y)
    }

    fn exact_tuple_buffer(a: Vec<u8>) -> bool {
        let mut iter = a.iter().tuples::<(_, _, _, _)>();
        (&mut iter).last();
        let buffer = iter.into_buffer();
        assert_eq!(buffer.len(), a.len() % 4);
        exact_size(buffer)
    }

    fn tuples_size_hint_inexact(a: Iter<u8>) -> bool {
        correct_size_hint(a.clone().tuples::<(_,)>())
            && correct_size_hint(a.clone().tuples::<(_, _)>())
            && correct_size_hint(a.tuples::<(_, _, _, _)>())
    }

    fn tuples_size_hint_exact(a: Iter<u8, Exact>) -> bool {
        exact_size(a.clone().tuples::<(_,)>())
            && exact_size(a.clone().tuples::<(_, _)>())
            && exact_size(a.tuples::<(_, _, _, _)>())
    }
}

// with_position
quickcheck! {
    fn with_position_exact_size_1(a: Vec<u8>) -> bool {
        exact_size_for_this(a.iter().with_position())
    }
    fn with_position_exact_size_2(a: Iter<u8, Exact>) -> bool {
        exact_size_for_this(a.with_position())
    }
}

quickcheck! {
    fn correct_group_map_modulo_key(a: Vec<u8>, modulo: u8) -> () {
        let modulo = if modulo == 0 { 1 } else { modulo }; // Avoid `% 0`
        let count = a.len();
        let lookup = a.into_iter().map(|i| (i % modulo, i)).into_group_map();

        assert_eq!(lookup.values().flat_map(|vals| vals.iter()).count(), count);

        for (&key, vals) in lookup.iter() {
            assert!(vals.iter().all(|&val| val % modulo == key));
        }
    }
}

/// A peculiar type: Equality compares both tuple items, but ordering only the
/// first item.  This is so we can check the stability property easily.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Val(u32, u32);

impl PartialOrd<Self> for Val {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Val {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl qc::Arbitrary for Val {
    fn arbitrary<G: qc::Gen>(g: &mut G) -> Self {
        let (x, y) = <(u32, u32)>::arbitrary(g);
        Self(x, y)
    }
    fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
        Box::new((self.0, self.1).shrink().map(|(x, y)| Self(x, y)))
    }
}

quickcheck! {
    fn minmax(a: Vec<Val>) -> bool {
        use itertools::MinMaxResult;


        let minmax = a.iter().minmax();
        let expected = match a.len() {
            0 => MinMaxResult::NoElements,
            1 => MinMaxResult::OneElement(&a[0]),
            _ => MinMaxResult::MinMax(a.iter().min().unwrap(),
                                      a.iter().max().unwrap()),
        };
        minmax == expected
    }
}

quickcheck! {
    fn minmax_f64(a: Vec<f64>) -> TestResult {
        use itertools::MinMaxResult;

        if a.iter().any(|x| x.is_nan()) {
            return TestResult::discard();
        }

        let min = cloned(&a).fold1(f64::min);
        let max = cloned(&a).fold1(f64::max);

        let minmax = cloned(&a).minmax();
        let expected = match a.len() {
            0 => MinMaxResult::NoElements,
            1 => MinMaxResult::OneElement(min.unwrap()),
            _ => MinMaxResult::MinMax(min.unwrap(), max.unwrap()),
        };
        TestResult::from_bool(minmax == expected)
    }
}

quickcheck! {
    fn tree_reduce_f64(mut a: Vec<f64>) -> TestResult {
        fn collapse_adjacent<F>(x: Vec<f64>, mut f: F) -> Vec<f64>
            where F: FnMut(f64, f64) -> f64
        {
            let mut out = Vec::new();
            for i in (0..x.len()).step_by(2) {
                if i == x.len()-1 {
                    out.push(x[i])
                } else {
                    out.push(f(x[i], x[i+1]));
                }
            }
            out
        }

        if a.iter().any(|x| x.is_nan()) {
            return TestResult::discard();
        }

        let actual = a.iter().cloned().tree_reduce(f64::atan2);

        while a.len() > 1 {
            a = collapse_adjacent(a, f64::atan2);
        }
        let expected = a.pop();

        TestResult::from_bool(actual == expected)
    }
}

quickcheck! {
    fn exactly_one_i32(a: Vec<i32>) -> TestResult {
        let ret = a.iter().cloned().exactly_one();
        match a.len() {
            1 => TestResult::from_bool(ret.unwrap() == a[0]),
            _ => TestResult::from_bool(ret.unwrap_err().eq(a.iter().cloned())),
        }
    }
}

quickcheck! {
    fn at_most_one_i32(a: Vec<i32>) -> TestResult {
        let ret = a.iter().cloned().at_most_one();
        match a.len() {
            0 => TestResult::from_bool(ret.unwrap().is_none()),
            1 => TestResult::from_bool(ret.unwrap() == Some(a[0])),
            _ => TestResult::from_bool(ret.unwrap_err().eq(a.iter().cloned())),
        }
    }
}

quickcheck! {
    fn consistent_grouping_map_with_by(a: Vec<u8>, modulo: u8) -> () {
        let modulo = if modulo == 0 { 1 } else { modulo }; // Avoid `% 0`

        let lookup_grouping_map = a.iter().copied().map(|i| (i % modulo, i)).into_grouping_map().collect::<Vec<_>>();
        let lookup_grouping_map_by = a.iter().copied().into_grouping_map_by(|i| i % modulo).collect::<Vec<_>>();

        assert_eq!(lookup_grouping_map, lookup_grouping_map_by);
    }

    fn correct_grouping_map_by_aggregate_modulo_key(a: Vec<u8>, modulo: u8) -> () {
        let modulo = if modulo < 2 { 2 } else { modulo } as u64; // Avoid `% 0`
        let lookup = a.iter()
            .map(|&b| b as u64) // Avoid overflows
            .into_grouping_map_by(|i| i % modulo)
            .aggregate(|acc, &key, val| {
                assert!(val % modulo == key);
                if val % (modulo - 1) == 0 {
                    None
                } else {
                    Some(acc.unwrap_or(0) + val)
                }
            });

        let group_map_lookup = a.iter()
            .map(|&b| b as u64)
            .map(|i| (i % modulo, i))
            .into_group_map()
            .into_iter()
            .filter_map(|(key, vals)| {
                vals.into_iter().fold(None, |acc, val| {
                    if val % (modulo - 1) == 0 {
                        None
                    } else {
                        Some(acc.unwrap_or(0) + val)
                    }
                }).map(|new_val| (key, new_val))
            })
            .collect::<HashMap<_,_>>();
        assert_eq!(lookup, group_map_lookup);

        for m in 0..modulo {
            assert_eq!(
                lookup.get(&m).copied(),
                a.iter()
                    .map(|&b| b as u64)
                    .filter(|&val| val % modulo == m)
                    .fold(None, |acc, val| {
                        if val % (modulo - 1) == 0 {
                            None
                        } else {
                            Some(acc.unwrap_or(0) + val)
                        }
                    })
            );
        }
    }

    fn correct_grouping_map_by_fold_with_modulo_key(a: Vec<u8>, modulo: u8) -> () {
        #[derive(Debug, Default, PartialEq)]
        struct Accumulator {
            acc: u64,
        }

        let modulo = if modulo == 0 { 1 } else { modulo } as u64; // Avoid `% 0`
        let lookup = a.iter().map(|&b| b as u64) // Avoid overflows
            .into_grouping_map_by(|i| i % modulo)
            .fold_with(|_key, _val| Default::default(), |Accumulator { acc }, &key, val| {
                assert!(val % modulo == key);
                let acc = acc + val;
                Accumulator { acc }
            });

        let group_map_lookup = a.iter()
            .map(|&b| b as u64)
            .map(|i| (i % modulo, i))
            .into_group_map()
            .into_iter()
            .map(|(key, vals)| (key, vals.into_iter().sum())).map(|(key, acc)| (key,Accumulator { acc }))
            .collect::<HashMap<_,_>>();
        assert_eq!(lookup, group_map_lookup);

        for (&key, &Accumulator { acc: sum }) in lookup.iter() {
            assert_eq!(sum, a.iter().map(|&b| b as u64).filter(|&val| val % modulo == key).sum::<u64>());
        }
    }

    fn correct_grouping_map_by_fold_modulo_key(a: Vec<u8>, modulo: u8) -> () {
        let modulo = if modulo == 0 { 1 } else { modulo } as u64; // Avoid `% 0`
        let lookup = a.iter().map(|&b| b as u64) // Avoid overflows
            .into_grouping_map_by(|i| i % modulo)
            .fold(0u64, |acc, &key, val| {
                assert!(val % modulo == key);
                acc + val
            });

        let group_map_lookup = a.iter()
            .map(|&b| b as u64)
            .map(|i| (i % modulo, i))
            .into_group_map()
            .into_iter()
            .map(|(key, vals)| (key, vals.into_iter().sum()))
            .collect::<HashMap<_,_>>();
        assert_eq!(lookup, group_map_lookup);

        for (&key, &sum) in lookup.iter() {
            assert_eq!(sum, a.iter().map(|&b| b as u64).filter(|&val| val % modulo == key).sum::<u64>());
        }
    }

    fn correct_grouping_map_by_reduce_modulo_key(a: Vec<u8>, modulo: u8) -> () {
        let modulo = if modulo == 0 { 1 } else { modulo } as u64; // Avoid `% 0`
        let lookup = a.iter().map(|&b| b as u64) // Avoid overflows
            .into_grouping_map_by(|i| i % modulo)
            .reduce(|acc, &key, val| {
                assert!(val % modulo == key);
                acc + val
            });

        let group_map_lookup = a.iter()
            .map(|&b| b as u64)
            .map(|i| (i % modulo, i))
            .into_group_map()
            .into_iter()
            .map(|(key, vals)| (key, vals.into_iter().reduce(|acc, val| acc + val).unwrap()))
            .collect::<HashMap<_,_>>();
        assert_eq!(lookup, group_map_lookup);

        for (&key, &sum) in lookup.iter() {
            assert_eq!(sum, a.iter().map(|&b| b as u64).filter(|&val| val % modulo == key).sum::<u64>());
        }
    }

    fn correct_grouping_map_by_collect_modulo_key(a: Vec<u8>, modulo: u8) -> () {
        let modulo = if modulo == 0 { 1 } else { modulo }; // Avoid `% 0`
        let lookup_grouping_map = a.iter().copied().into_grouping_map_by(|i| i % modulo).collect::<Vec<_>>();
        let lookup_group_map = a.iter().copied().map(|i| (i % modulo, i)).into_group_map();

        assert_eq!(lookup_grouping_map, lookup_group_map);
    }

    fn correct_grouping_map_by_max_modulo_key(a: Vec<u8>, modulo: u8) -> () {
        let modulo = if modulo == 0 { 1 } else { modulo }; // Avoid `% 0`
        let lookup = a.iter().copied().into_grouping_map_by(|i| i % modulo).max();

        let group_map_lookup = a.iter().copied()
            .map(|i| (i % modulo, i))
            .into_group_map()
            .into_iter()
            .map(|(key, vals)| (key, vals.into_iter().max().unwrap()))
            .collect::<HashMap<_,_>>();
        assert_eq!(lookup, group_map_lookup);

        for (&key, &max) in lookup.iter() {
            assert_eq!(Some(max), a.iter().copied().filter(|&val| val % modulo == key).max());
        }
    }

    fn correct_grouping_map_by_max_by_modulo_key(a: Vec<u8>, modulo: u8) -> () {
        let modulo = if modulo == 0 { 1 } else { modulo }; // Avoid `% 0`
        let lookup = a.iter().copied().into_grouping_map_by(|i| i % modulo).max_by(|_, v1, v2| v1.cmp(v2));

        let group_map_lookup = a.iter().copied()
            .map(|i| (i % modulo, i))
            .into_group_map()
            .into_iter()
            .map(|(key, vals)| (key, vals.into_iter().max_by(|v1, v2| v1.cmp(v2)).unwrap()))
            .collect::<HashMap<_,_>>();
        assert_eq!(lookup, group_map_lookup);

        for (&key, &max) in lookup.iter() {
            assert_eq!(Some(max), a.iter().copied().filter(|&val| val % modulo == key).max_by(|v1, v2| v1.cmp(v2)));
        }
    }

    fn correct_grouping_map_by_max_by_key_modulo_key(a: Vec<u8>, modulo: u8) -> () {
        let modulo = if modulo == 0 { 1 } else { modulo }; // Avoid `% 0`
        let lookup = a.iter().copied().into_grouping_map_by(|i| i % modulo).max_by_key(|_, &val| val);

        let group_map_lookup = a.iter().copied()
            .map(|i| (i % modulo, i))
            .into_group_map()
            .into_iter()
            .map(|(key, vals)| (key, vals.into_iter().max_by_key(|&val| val).unwrap()))
            .collect::<HashMap<_,_>>();
        assert_eq!(lookup, group_map_lookup);

        for (&key, &max) in lookup.iter() {
            assert_eq!(Some(max), a.iter().copied().filter(|&val| val % modulo == key).max_by_key(|&val| val));
        }
    }

    fn correct_grouping_map_by_min_modulo_key(a: Vec<u8>, modulo: u8) -> () {
        let modulo = if modulo == 0 { 1 } else { modulo }; // Avoid `% 0`
        let lookup = a.iter().copied().into_grouping_map_by(|i| i % modulo).min();

        let group_map_lookup = a.iter().copied()
            .map(|i| (i % modulo, i))
            .into_group_map()
            .into_iter()
            .map(|(key, vals)| (key, vals.into_iter().min().unwrap()))
            .collect::<HashMap<_,_>>();
        assert_eq!(lookup, group_map_lookup);

        for (&key, &min) in lookup.iter() {
            assert_eq!(Some(min), a.iter().copied().filter(|&val| val % modulo == key).min());
        }
    }

    fn correct_grouping_map_by_min_by_modulo_key(a: Vec<u8>, modulo: u8) -> () {
        let modulo = if modulo == 0 { 1 } else { modulo }; // Avoid `% 0`
        let lookup = a.iter().copied().into_grouping_map_by(|i| i % modulo).min_by(|_, v1, v2| v1.cmp(v2));

        let group_map_lookup = a.iter().copied()
            .map(|i| (i % modulo, i))
            .into_group_map()
            .into_iter()
            .map(|(key, vals)| (key, vals.into_iter().min_by(|v1, v2| v1.cmp(v2)).unwrap()))
            .collect::<HashMap<_,_>>();
        assert_eq!(lookup, group_map_lookup);

        for (&key, &min) in lookup.iter() {
            assert_eq!(Some(min), a.iter().copied().filter(|&val| val % modulo == key).min_by(|v1, v2| v1.cmp(v2)));
        }
    }

    fn correct_grouping_map_by_min_by_key_modulo_key(a: Vec<u8>, modulo: u8) -> () {
        let modulo = if modulo == 0 { 1 } else { modulo }; // Avoid `% 0`
        let lookup = a.iter().copied().into_grouping_map_by(|i| i % modulo).min_by_key(|_, &val| val);

        let group_map_lookup = a.iter().copied()
            .map(|i| (i % modulo, i))
            .into_group_map()
            .into_iter()
            .map(|(key, vals)| (key, vals.into_iter().min_by_key(|&val| val).unwrap()))
            .collect::<HashMap<_,_>>();
        assert_eq!(lookup, group_map_lookup);

        for (&key, &min) in lookup.iter() {
            assert_eq!(Some(min), a.iter().copied().filter(|&val| val % modulo == key).min_by_key(|&val| val));
        }
    }

    fn correct_grouping_map_by_minmax_modulo_key(a: Vec<u8>, modulo: u8) -> () {
        let modulo = if modulo == 0 { 1 } else { modulo }; // Avoid `% 0`
        let lookup = a.iter().copied().into_grouping_map_by(|i| i % modulo).minmax();

        let group_map_lookup = a.iter().copied()
            .map(|i| (i % modulo, i))
            .into_group_map()
            .into_iter()
            .map(|(key, vals)| (key, vals.into_iter().minmax()))
            .collect::<HashMap<_,_>>();
        assert_eq!(lookup, group_map_lookup);

        for (&key, &minmax) in lookup.iter() {
            assert_eq!(minmax, a.iter().copied().filter(|&val| val % modulo == key).minmax());
        }
    }

    fn correct_grouping_map_by_minmax_by_modulo_key(a: Vec<u8>, modulo: u8) -> () {
        let modulo = if modulo == 0 { 1 } else { modulo }; // Avoid `% 0`
        let lookup = a.iter().copied().into_grouping_map_by(|i| i % modulo).minmax_by(|_, v1, v2| v1.cmp(v2));

        let group_map_lookup = a.iter().copied()
            .map(|i| (i % modulo, i))
            .into_group_map()
            .into_iter()
            .map(|(key, vals)| (key, vals.into_iter().minmax_by(|v1, v2| v1.cmp(v2))))
            .collect::<HashMap<_,_>>();
        assert_eq!(lookup, group_map_lookup);

        for (&key, &minmax) in lookup.iter() {
            assert_eq!(minmax, a.iter().copied().filter(|&val| val % modulo == key).minmax_by(|v1, v2| v1.cmp(v2)));
        }
    }

    fn correct_grouping_map_by_minmax_by_key_modulo_key(a: Vec<u8>, modulo: u8) -> () {
        let modulo = if modulo == 0 { 1 } else { modulo }; // Avoid `% 0`
        let lookup = a.iter().copied().into_grouping_map_by(|i| i % modulo).minmax_by_key(|_, &val| val);

        let group_map_lookup = a.iter().copied()
            .map(|i| (i % modulo, i))
            .into_group_map()
            .into_iter()
            .map(|(key, vals)| (key, vals.into_iter().minmax_by_key(|&val| val)))
            .collect::<HashMap<_,_>>();
        assert_eq!(lookup, group_map_lookup);

        for (&key, &minmax) in lookup.iter() {
            assert_eq!(minmax, a.iter().copied().filter(|&val| val % modulo == key).minmax_by_key(|&val| val));
        }
    }

    fn correct_grouping_map_by_sum_modulo_key(a: Vec<u8>, modulo: u8) -> () {
        let modulo = if modulo == 0 { 1 } else { modulo } as u64; // Avoid `% 0`
        let lookup = a.iter().map(|&b| b as u64) // Avoid overflows
            .into_grouping_map_by(|i| i % modulo)
            .sum();

        let group_map_lookup = a.iter().map(|&b| b as u64)
            .map(|i| (i % modulo, i))
            .into_group_map()
            .into_iter()
            .map(|(key, vals)| (key, vals.into_iter().sum()))
            .collect::<HashMap<_,_>>();
        assert_eq!(lookup, group_map_lookup);

        for (&key, &sum) in lookup.iter() {
            assert_eq!(sum, a.iter().map(|&b| b as u64).filter(|&val| val % modulo == key).sum::<u64>());
        }
    }

    fn correct_grouping_map_by_product_modulo_key(a: Vec<u8>, modulo: u8) -> () {
        let modulo = Wrapping(if modulo == 0 { 1 } else { modulo } as u64); // Avoid `% 0`
        let lookup = a.iter().map(|&b| Wrapping(b as u64)) // Avoid overflows
            .into_grouping_map_by(|i| i % modulo)
            .product();

        let group_map_lookup = a.iter().map(|&b| Wrapping(b as u64))
            .map(|i| (i % modulo, i))
            .into_group_map()
            .into_iter()
            .map(|(key, vals)| (key, vals.into_iter().product::<Wrapping<u64>>()))
            .collect::<HashMap<_,_>>();
        assert_eq!(lookup, group_map_lookup);

        for (&key, &prod) in lookup.iter() {
            assert_eq!(
                prod,
                a.iter()
                    .map(|&b| Wrapping(b as u64))
                    .filter(|&val| val % modulo == key)
                    .product::<Wrapping<u64>>()
            );
        }
    }

    // This should check that if multiple elements are equally minimum or maximum
    // then `max`, `min` and `minmax` pick the first minimum and the last maximum.
    // This is to be consistent with `std::iter::max` and `std::iter::min`.
    fn correct_grouping_map_by_min_max_minmax_order_modulo_key() -> () {
        use itertools::MinMaxResult;

        let lookup = (0..=10)
            .into_grouping_map_by(|_| 0)
            .max_by(|_, _, _| Ordering::Equal);

        assert_eq!(lookup[&0], 10);

        let lookup = (0..=10)
            .into_grouping_map_by(|_| 0)
            .min_by(|_, _, _| Ordering::Equal);

        assert_eq!(lookup[&0], 0);

        let lookup = (0..=10)
            .into_grouping_map_by(|_| 0)
            .minmax_by(|_, _, _| Ordering::Equal);

        assert_eq!(lookup[&0], MinMaxResult::MinMax(0, 10));
    }
}

quickcheck! {
    fn counts(nums: Vec<isize>) -> TestResult {
        let counts = nums.iter().counts();
        for (&item, &count) in counts.iter() {
            #[allow(clippy::absurd_extreme_comparisons)]
            if count <= 0 {
                return TestResult::failed();
            }
            if count != nums.iter().filter(|&x| x == item).count() {
                return TestResult::failed();
            }
        }
        for item in nums.iter() {
            if !counts.contains_key(item) {
                return TestResult::failed();
            }
        }
        TestResult::passed()
    }
}

quickcheck! {
    fn test_double_ended_zip_2(a: Vec<u8>, b: Vec<u8>) -> TestResult {
        let mut x =
          multizip((a.clone().into_iter(), b.clone().into_iter()))
            .collect_vec();
        x.reverse();

        let y =
          multizip((a.into_iter(), b.into_iter()))
          .rfold(Vec::new(), |mut vec, e| { vec.push(e); vec });

        TestResult::from_bool(itertools::equal(x, y))
    }

    fn test_double_ended_zip_3(a: Vec<u8>, b: Vec<u8>, c: Vec<u8>) -> TestResult {
        let mut x =
          multizip((a.clone().into_iter(), b.clone().into_iter(), c.clone().into_iter()))
            .collect_vec();
        x.reverse();

        let y =
          multizip((a.into_iter(), b.into_iter(), c.into_iter()))
          .rfold(Vec::new(), |mut vec, e| { vec.push(e); vec });

        TestResult::from_bool(itertools::equal(x, y))
    }
}

fn is_fused<I: Iterator>(mut it: I) -> bool {
    for _ in it.by_ref() {}
    for _ in 0..10 {
        if it.next().is_some() {
            return false;
        }
    }
    true
}

quickcheck! {
    fn fused_combination(a: Iter<i16>) -> bool
    {
        is_fused(a.clone().combinations(1)) &&
        is_fused(a.combinations(3))
    }

    fn fused_combination_with_replacement(a: Iter<i16>) -> bool
    {
        is_fused(a.clone().combinations_with_replacement(1)) &&
        is_fused(a.combinations_with_replacement(3))
    }

    fn fused_tuple_combination(a: Iter<i16>) -> bool
    {
        is_fused(a.clone().fuse().tuple_combinations::<(_,)>()) &&
        is_fused(a.fuse().tuple_combinations::<(_,_,_)>())
    }

    fn fused_unique(a: Iter<i16>) -> bool
    {
        is_fused(a.fuse().unique())
    }

    fn fused_unique_by(a: Iter<i16>) -> bool
    {
        is_fused(a.fuse().unique_by(|x| x % 100))
    }

    fn fused_interleave_shortest(a: Iter<i16>, b: Iter<i16>) -> bool
    {
        !is_fused(a.clone().interleave_shortest(b.clone())) &&
        is_fused(a.fuse().interleave_shortest(b.fuse()))
    }

    fn fused_product(a: Iter<i16>, b: Iter<i16>) -> bool
    {
        is_fused(a.fuse().cartesian_product(b.fuse()))
    }

    fn fused_merge(a: Iter<i16>, b: Iter<i16>) -> bool
    {
        is_fused(a.fuse().merge(b.fuse()))
    }

    fn fused_filter_ok(a: Iter<i16>) -> bool
    {
        is_fused(a.map(|x| if x % 2 == 0 {Ok(x)} else {Err(x)} )
                 .filter_ok(|x| x % 3 == 0)
                 .fuse())
    }

    fn fused_filter_map_ok(a: Iter<i16>) -> bool
    {
        is_fused(a.map(|x| if x % 2 == 0 {Ok(x)} else {Err(x)} )
                 .filter_map_ok(|x| if x % 3 == 0 {Some(x / 3)} else {None})
                 .fuse())
    }

    fn fused_positions(a: Iter<i16>) -> bool
    {
        !is_fused(a.clone().positions(|x|x%2==0)) &&
        is_fused(a.fuse().positions(|x|x%2==0))
    }

    fn fused_update(a: Iter<i16>) -> bool
    {
        !is_fused(a.clone().update(|x|*x+=1)) &&
        is_fused(a.fuse().update(|x|*x+=1))
    }

    fn fused_tuple_windows(a: Iter<i16>) -> bool
    {
        is_fused(a.fuse().tuple_windows::<(_,_)>())
    }

    fn fused_pad_using(a: Iter<i16>) -> bool
    {
        is_fused(a.fuse().pad_using(100,|_|0))
    }
}

quickcheck! {
    fn min_set_contains_min(a: Vec<(usize, char)>) -> bool {
        let result_set = a.iter().min_set();
        if let Some(result_element) = a.iter().min() {
            result_set.contains(&result_element)
        } else {
            result_set.is_empty()
        }
    }

    fn min_set_by_contains_min(a: Vec<(usize, char)>) -> bool {
        let compare = |x: &&(usize, char), y: &&(usize, char)| x.1.cmp(&y.1);
        let result_set = a.iter().min_set_by(compare);
        if let Some(result_element) = a.iter().min_by(compare) {
            result_set.contains(&result_element)
        } else {
            result_set.is_empty()
        }
    }

    fn min_set_by_key_contains_min(a: Vec<(usize, char)>) -> bool {
        let key = |x: &&(usize, char)| x.1;
        let result_set = a.iter().min_set_by_key(&key);
        if let Some(result_element) = a.iter().min_by_key(&key) {
            result_set.contains(&result_element)
        } else {
            result_set.is_empty()
        }
    }

    fn max_set_contains_max(a: Vec<(usize, char)>) -> bool {
        let result_set = a.iter().max_set();
        if let Some(result_element) = a.iter().max() {
            result_set.contains(&result_element)
        } else {
            result_set.is_empty()
        }
    }

    fn max_set_by_contains_max(a: Vec<(usize, char)>) -> bool {
        let compare = |x: &&(usize, char), y: &&(usize, char)| x.1.cmp(&y.1);
        let result_set = a.iter().max_set_by(compare);
        if let Some(result_element) = a.iter().max_by(compare) {
            result_set.contains(&result_element)
        } else {
            result_set.is_empty()
        }
    }

    fn max_set_by_key_contains_max(a: Vec<(usize, char)>) -> bool {
        let key = |x: &&(usize, char)| x.1;
        let result_set = a.iter().max_set_by_key(&key);
        if let Some(result_element) = a.iter().max_by_key(&key) {
            result_set.contains(&result_element)
        } else {
            result_set.is_empty()
        }
    }

    fn tail(v: Vec<i32>, n: u8) -> bool {
        let n = n as usize;
        let result = &v[v.len().saturating_sub(n)..];
        itertools::equal(v.iter().tail(n), result)
            && itertools::equal(v.iter().filter(|_| true).tail(n), result)
    }
}
