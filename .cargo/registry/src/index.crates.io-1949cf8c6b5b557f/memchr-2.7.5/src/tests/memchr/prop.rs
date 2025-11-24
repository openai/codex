/// Defines a host of quickcheck tests for the given memchr searcher.
#[cfg(miri)]
#[macro_export]
macro_rules! define_memchr_quickcheck {
    ($($tt:tt)*) => {};
}

/// Defines a host of quickcheck tests for the given memchr searcher.
#[cfg(not(miri))]
#[macro_export]
macro_rules! define_memchr_quickcheck {
    ($mod:ident) => {
        define_memchr_quickcheck!($mod, new);
    };
    ($mod:ident, $cons:ident) => {
        use alloc::vec::Vec;

        use quickcheck::TestResult;

        use crate::tests::memchr::{
            naive,
            prop::{double_ended_take, naive1_iter, naive2_iter, naive3_iter},
        };

        quickcheck::quickcheck! {
            fn qc_memchr_matches_naive(n1: u8, corpus: Vec<u8>) -> TestResult {
                let expected = naive::memchr(n1, &corpus);
                let got = match $mod::One::$cons(n1) {
                    None => return TestResult::discard(),
                    Some(f) => f.find(&corpus),
                };
                TestResult::from_bool(expected == got)
            }

            fn qc_memrchr_matches_naive(n1: u8, corpus: Vec<u8>) -> TestResult {
                let expected = naive::memrchr(n1, &corpus);
                let got = match $mod::One::$cons(n1) {
                    None => return TestResult::discard(),
                    Some(f) => f.rfind(&corpus),
                };
                TestResult::from_bool(expected == got)
            }

            fn qc_memchr2_matches_naive(n1: u8, n2: u8, corpus: Vec<u8>) -> TestResult {
                let expected = naive::memchr2(n1, n2, &corpus);
                let got = match $mod::Two::$cons(n1, n2) {
                    None => return TestResult::discard(),
                    Some(f) => f.find(&corpus),
                };
                TestResult::from_bool(expected == got)
            }

            fn qc_memrchr2_matches_naive(n1: u8, n2: u8, corpus: Vec<u8>) -> TestResult {
                let expected = naive::memrchr2(n1, n2, &corpus);
                let got = match $mod::Two::$cons(n1, n2) {
                    None => return TestResult::discard(),
                    Some(f) => f.rfind(&corpus),
                };
                TestResult::from_bool(expected == got)
            }

            fn qc_memchr3_matches_naive(
                n1: u8, n2: u8, n3: u8,
                corpus: Vec<u8>
            ) -> TestResult {
                let expected = naive::memchr3(n1, n2, n3, &corpus);
                let got = match $mod::Three::$cons(n1, n2, n3) {
                    None => return TestResult::discard(),
                    Some(f) => f.find(&corpus),
                };
                TestResult::from_bool(expected == got)
            }

            fn qc_memrchr3_matches_naive(
                n1: u8, n2: u8, n3: u8,
                corpus: Vec<u8>
            ) -> TestResult {
                let expected = naive::memrchr3(n1, n2, n3, &corpus);
                let got = match $mod::Three::$cons(n1, n2, n3) {
                    None => return TestResult::discard(),
                    Some(f) => f.rfind(&corpus),
                };
                TestResult::from_bool(expected == got)
            }

            fn qc_memchr_double_ended_iter(
                needle: u8, data: Vec<u8>, take_side: Vec<bool>
            ) -> TestResult {
                // make nonempty
                let mut take_side = take_side;
                if take_side.is_empty() { take_side.push(true) };

                let finder = match $mod::One::$cons(needle) {
                    None => return TestResult::discard(),
                    Some(finder) => finder,
                };
                let iter = finder.iter(&data);
                let got = double_ended_take(
                    iter,
                    take_side.iter().cycle().cloned(),
                );
                let expected = naive1_iter(needle, &data);

                TestResult::from_bool(got.iter().cloned().eq(expected))
            }

            fn qc_memchr2_double_ended_iter(
                needle1: u8, needle2: u8, data: Vec<u8>, take_side: Vec<bool>
            ) -> TestResult {
                // make nonempty
                let mut take_side = take_side;
                if take_side.is_empty() { take_side.push(true) };

                let finder = match $mod::Two::$cons(needle1, needle2) {
                    None => return TestResult::discard(),
                    Some(finder) => finder,
                };
                let iter = finder.iter(&data);
                let got = double_ended_take(
                    iter,
                    take_side.iter().cycle().cloned(),
                );
                let expected = naive2_iter(needle1, needle2, &data);

                TestResult::from_bool(got.iter().cloned().eq(expected))
            }

            fn qc_memchr3_double_ended_iter(
                needle1: u8, needle2: u8, needle3: u8,
                data: Vec<u8>, take_side: Vec<bool>
            ) -> TestResult {
                // make nonempty
                let mut take_side = take_side;
                if take_side.is_empty() { take_side.push(true) };

                let finder = match $mod::Three::$cons(needle1, needle2, needle3) {
                    None => return TestResult::discard(),
                    Some(finder) => finder,
                };
                let iter = finder.iter(&data);
                let got = double_ended_take(
                    iter,
                    take_side.iter().cycle().cloned(),
                );
                let expected = naive3_iter(needle1, needle2, needle3, &data);

                TestResult::from_bool(got.iter().cloned().eq(expected))
            }

            fn qc_memchr1_iter(data: Vec<u8>) -> TestResult {
                let needle = 0;
                let finder = match $mod::One::$cons(needle) {
                    None => return TestResult::discard(),
                    Some(finder) => finder,
                };
                let got = finder.iter(&data);
                let expected = naive1_iter(needle, &data);
                TestResult::from_bool(got.eq(expected))
            }

            fn qc_memchr1_rev_iter(data: Vec<u8>) -> TestResult {
                let needle = 0;

                let finder = match $mod::One::$cons(needle) {
                    None => return TestResult::discard(),
                    Some(finder) => finder,
                };
                let got = finder.iter(&data).rev();
                let expected = naive1_iter(needle, &data).rev();
                TestResult::from_bool(got.eq(expected))
            }

            fn qc_memchr2_iter(data: Vec<u8>) -> TestResult {
                let needle1 = 0;
                let needle2 = 1;

                let finder = match $mod::Two::$cons(needle1, needle2) {
                    None => return TestResult::discard(),
                    Some(finder) => finder,
                };
                let got = finder.iter(&data);
                let expected = naive2_iter(needle1, needle2, &data);
                TestResult::from_bool(got.eq(expected))
            }

            fn qc_memchr2_rev_iter(data: Vec<u8>) -> TestResult {
                let needle1 = 0;
                let needle2 = 1;

                let finder = match $mod::Two::$cons(needle1, needle2) {
                    None => return TestResult::discard(),
                    Some(finder) => finder,
                };
                let got = finder.iter(&data).rev();
                let expected = naive2_iter(needle1, needle2, &data).rev();
                TestResult::from_bool(got.eq(expected))
            }

            fn qc_memchr3_iter(data: Vec<u8>) -> TestResult {
                let needle1 = 0;
                let needle2 = 1;
                let needle3 = 2;

                let finder = match $mod::Three::$cons(needle1, needle2, needle3) {
                    None => return TestResult::discard(),
                    Some(finder) => finder,
                };
                let got = finder.iter(&data);
                let expected = naive3_iter(needle1, needle2, needle3, &data);
                TestResult::from_bool(got.eq(expected))
            }

            fn qc_memchr3_rev_iter(data: Vec<u8>) -> TestResult {
                let needle1 = 0;
                let needle2 = 1;
                let needle3 = 2;

                let finder = match $mod::Three::$cons(needle1, needle2, needle3) {
                    None => return TestResult::discard(),
                    Some(finder) => finder,
                };
                let got = finder.iter(&data).rev();
                let expected = naive3_iter(needle1, needle2, needle3, &data).rev();
                TestResult::from_bool(got.eq(expected))
            }

            fn qc_memchr1_iter_size_hint(data: Vec<u8>) -> TestResult {
                // test that the size hint is within reasonable bounds
                let needle = 0;
                let finder = match $mod::One::$cons(needle) {
                    None => return TestResult::discard(),
                    Some(finder) => finder,
                };
                let mut iter = finder.iter(&data);
                let mut real_count = data
                    .iter()
                    .filter(|&&elt| elt == needle)
                    .count();

                while let Some(index) = iter.next() {
                    real_count -= 1;
                    let (lower, upper) = iter.size_hint();
                    assert!(lower <= real_count);
                    assert!(upper.unwrap() >= real_count);
                    assert!(upper.unwrap() <= data.len() - index);
                }
                TestResult::passed()
            }
        }
    };
}

// take items from a DEI, taking front for each true and back for each false.
// Return a vector with the concatenation of the fronts and the reverse of the
// backs.
#[cfg(not(miri))]
pub(crate) fn double_ended_take<I, J>(
    mut iter: I,
    take_side: J,
) -> alloc::vec::Vec<I::Item>
where
    I: DoubleEndedIterator,
    J: Iterator<Item = bool>,
{
    let mut found_front = alloc::vec![];
    let mut found_back = alloc::vec![];

    for take_front in take_side {
        if take_front {
            if let Some(pos) = iter.next() {
                found_front.push(pos);
            } else {
                break;
            }
        } else {
            if let Some(pos) = iter.next_back() {
                found_back.push(pos);
            } else {
                break;
            }
        };
    }

    let mut all_found = found_front;
    all_found.extend(found_back.into_iter().rev());
    all_found
}

// return an iterator of the 0-based indices of haystack that match the needle
#[cfg(not(miri))]
pub(crate) fn naive1_iter<'a>(
    n1: u8,
    haystack: &'a [u8],
) -> impl DoubleEndedIterator<Item = usize> + 'a {
    haystack.iter().enumerate().filter(move |&(_, &b)| b == n1).map(|t| t.0)
}

#[cfg(not(miri))]
pub(crate) fn naive2_iter<'a>(
    n1: u8,
    n2: u8,
    haystack: &'a [u8],
) -> impl DoubleEndedIterator<Item = usize> + 'a {
    haystack
        .iter()
        .enumerate()
        .filter(move |&(_, &b)| b == n1 || b == n2)
        .map(|t| t.0)
}

#[cfg(not(miri))]
pub(crate) fn naive3_iter<'a>(
    n1: u8,
    n2: u8,
    n3: u8,
    haystack: &'a [u8],
) -> impl DoubleEndedIterator<Item = usize> + 'a {
    haystack
        .iter()
        .enumerate()
        .filter(move |&(_, &b)| b == n1 || b == n2 || b == n3)
        .map(|t| t.0)
}
