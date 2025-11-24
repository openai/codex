// Copyright 2018-2024 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! `IteratorRandom`

use super::coin_flipper::CoinFlipper;
#[allow(unused)]
use super::IndexedRandom;
use crate::Rng;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

/// Extension trait on iterators, providing random sampling methods.
///
/// This trait is implemented on all iterators `I` where `I: Iterator + Sized`
/// and provides methods for
/// choosing one or more elements. You must `use` this trait:
///
/// ```
/// use rand::seq::IteratorRandom;
///
/// let faces = "😀😎😐😕😠😢";
/// println!("I am {}!", faces.chars().choose(&mut rand::rng()).unwrap());
/// ```
/// Example output (non-deterministic):
/// ```none
/// I am 😀!
/// ```
pub trait IteratorRandom: Iterator + Sized {
    /// Uniformly sample one element
    ///
    /// Assuming that the [`Iterator::size_hint`] is correct, this method
    /// returns one uniformly-sampled random element of the slice, or `None`
    /// only if the slice is empty. Incorrect bounds on the `size_hint` may
    /// cause this method to incorrectly return `None` if fewer elements than
    /// the advertised `lower` bound are present and may prevent sampling of
    /// elements beyond an advertised `upper` bound (i.e. incorrect `size_hint`
    /// is memory-safe, but may result in unexpected `None` result and
    /// non-uniform distribution).
    ///
    /// With an accurate [`Iterator::size_hint`] and where [`Iterator::nth`] is
    /// a constant-time operation, this method can offer `O(1)` performance.
    /// Where no size hint is
    /// available, complexity is `O(n)` where `n` is the iterator length.
    /// Partial hints (where `lower > 0`) also improve performance.
    ///
    /// Note further that [`Iterator::size_hint`] may affect the number of RNG
    /// samples used as well as the result (while remaining uniform sampling).
    /// Consider instead using [`IteratorRandom::choose_stable`] to avoid
    /// [`Iterator`] combinators which only change size hints from affecting the
    /// results.
    ///
    /// # Example
    ///
    /// ```
    /// use rand::seq::IteratorRandom;
    ///
    /// let words = "Mary had a little lamb".split(' ');
    /// println!("{}", words.choose(&mut rand::rng()).unwrap());
    /// ```
    fn choose<R>(mut self, rng: &mut R) -> Option<Self::Item>
    where
        R: Rng + ?Sized,
    {
        let (mut lower, mut upper) = self.size_hint();
        let mut result = None;

        // Handling for this condition outside the loop allows the optimizer to eliminate the loop
        // when the Iterator is an ExactSizeIterator. This has a large performance impact on e.g.
        // seq_iter_choose_from_1000.
        if upper == Some(lower) {
            return match lower {
                0 => None,
                1 => self.next(),
                _ => self.nth(rng.random_range(..lower)),
            };
        }

        let mut coin_flipper = CoinFlipper::new(rng);
        let mut consumed = 0;

        // Continue until the iterator is exhausted
        loop {
            if lower > 1 {
                let ix = coin_flipper.rng.random_range(..lower + consumed);
                let skip = if ix < lower {
                    result = self.nth(ix);
                    lower - (ix + 1)
                } else {
                    lower
                };
                if upper == Some(lower) {
                    return result;
                }
                consumed += lower;
                if skip > 0 {
                    self.nth(skip - 1);
                }
            } else {
                let elem = self.next();
                if elem.is_none() {
                    return result;
                }
                consumed += 1;
                if coin_flipper.random_ratio_one_over(consumed) {
                    result = elem;
                }
            }

            let hint = self.size_hint();
            lower = hint.0;
            upper = hint.1;
        }
    }

    /// Uniformly sample one element (stable)
    ///
    /// This method is very similar to [`choose`] except that the result
    /// only depends on the length of the iterator and the values produced by
    /// `rng`. Notably for any iterator of a given length this will make the
    /// same requests to `rng` and if the same sequence of values are produced
    /// the same index will be selected from `self`. This may be useful if you
    /// need consistent results no matter what type of iterator you are working
    /// with. If you do not need this stability prefer [`choose`].
    ///
    /// Note that this method still uses [`Iterator::size_hint`] to skip
    /// constructing elements where possible, however the selection and `rng`
    /// calls are the same in the face of this optimization. If you want to
    /// force every element to be created regardless call `.inspect(|e| ())`.
    ///
    /// [`choose`]: IteratorRandom::choose
    //
    // Clippy is wrong here: we need to iterate over all entries with the RNG to
    // ensure that choosing is *stable*.
    // "allow(unknown_lints)" can be removed when switching to at least
    // rust-version 1.86.0, see:
    // https://rust-lang.github.io/rust-clippy/master/index.html#double_ended_iterator_last
    #[allow(unknown_lints)]
    #[allow(clippy::double_ended_iterator_last)]
    fn choose_stable<R>(mut self, rng: &mut R) -> Option<Self::Item>
    where
        R: Rng + ?Sized,
    {
        let mut consumed = 0;
        let mut result = None;
        let mut coin_flipper = CoinFlipper::new(rng);

        loop {
            // Currently the only way to skip elements is `nth()`. So we need to
            // store what index to access next here.
            // This should be replaced by `advance_by()` once it is stable:
            // https://github.com/rust-lang/rust/issues/77404
            let mut next = 0;

            let (lower, _) = self.size_hint();
            if lower >= 2 {
                let highest_selected = (0..lower)
                    .filter(|ix| coin_flipper.random_ratio_one_over(consumed + ix + 1))
                    .last();

                consumed += lower;
                next = lower;

                if let Some(ix) = highest_selected {
                    result = self.nth(ix);
                    next -= ix + 1;
                    debug_assert!(result.is_some(), "iterator shorter than size_hint().0");
                }
            }

            let elem = self.nth(next);
            if elem.is_none() {
                return result;
            }

            if coin_flipper.random_ratio_one_over(consumed + 1) {
                result = elem;
            }
            consumed += 1;
        }
    }

    /// Uniformly sample `amount` distinct elements into a buffer
    ///
    /// Collects values at random from the iterator into a supplied buffer
    /// until that buffer is filled.
    ///
    /// Although the elements are selected randomly, the order of elements in
    /// the buffer is neither stable nor fully random. If random ordering is
    /// desired, shuffle the result.
    ///
    /// Returns the number of elements added to the buffer. This equals the length
    /// of the buffer unless the iterator contains insufficient elements, in which
    /// case this equals the number of elements available.
    ///
    /// Complexity is `O(n)` where `n` is the length of the iterator.
    /// For slices, prefer [`IndexedRandom::choose_multiple`].
    fn choose_multiple_fill<R>(mut self, rng: &mut R, buf: &mut [Self::Item]) -> usize
    where
        R: Rng + ?Sized,
    {
        let amount = buf.len();
        let mut len = 0;
        while len < amount {
            if let Some(elem) = self.next() {
                buf[len] = elem;
                len += 1;
            } else {
                // Iterator exhausted; stop early
                return len;
            }
        }

        // Continue, since the iterator was not exhausted
        for (i, elem) in self.enumerate() {
            let k = rng.random_range(..i + 1 + amount);
            if let Some(slot) = buf.get_mut(k) {
                *slot = elem;
            }
        }
        len
    }

    /// Uniformly sample `amount` distinct elements into a [`Vec`]
    ///
    /// This is equivalent to `choose_multiple_fill` except for the result type.
    ///
    /// Although the elements are selected randomly, the order of elements in
    /// the buffer is neither stable nor fully random. If random ordering is
    /// desired, shuffle the result.
    ///
    /// The length of the returned vector equals `amount` unless the iterator
    /// contains insufficient elements, in which case it equals the number of
    /// elements available.
    ///
    /// Complexity is `O(n)` where `n` is the length of the iterator.
    /// For slices, prefer [`IndexedRandom::choose_multiple`].
    #[cfg(feature = "alloc")]
    fn choose_multiple<R>(mut self, rng: &mut R, amount: usize) -> Vec<Self::Item>
    where
        R: Rng + ?Sized,
    {
        let mut reservoir = Vec::with_capacity(amount);
        reservoir.extend(self.by_ref().take(amount));

        // Continue unless the iterator was exhausted
        //
        // note: this prevents iterators that "restart" from causing problems.
        // If the iterator stops once, then so do we.
        if reservoir.len() == amount {
            for (i, elem) in self.enumerate() {
                let k = rng.random_range(..i + 1 + amount);
                if let Some(slot) = reservoir.get_mut(k) {
                    *slot = elem;
                }
            }
        } else {
            // Don't hang onto extra memory. There is a corner case where
            // `amount` was much less than `self.len()`.
            reservoir.shrink_to_fit();
        }
        reservoir
    }
}

impl<I> IteratorRandom for I where I: Iterator + Sized {}

#[cfg(test)]
mod test {
    use super::*;
    #[cfg(all(feature = "alloc", not(feature = "std")))]
    use alloc::vec::Vec;

    #[derive(Clone)]
    struct UnhintedIterator<I: Iterator + Clone> {
        iter: I,
    }
    impl<I: Iterator + Clone> Iterator for UnhintedIterator<I> {
        type Item = I::Item;

        fn next(&mut self) -> Option<Self::Item> {
            self.iter.next()
        }
    }

    #[derive(Clone)]
    struct ChunkHintedIterator<I: ExactSizeIterator + Iterator + Clone> {
        iter: I,
        chunk_remaining: usize,
        chunk_size: usize,
        hint_total_size: bool,
    }
    impl<I: ExactSizeIterator + Iterator + Clone> Iterator for ChunkHintedIterator<I> {
        type Item = I::Item;

        fn next(&mut self) -> Option<Self::Item> {
            if self.chunk_remaining == 0 {
                self.chunk_remaining = core::cmp::min(self.chunk_size, self.iter.len());
            }
            self.chunk_remaining = self.chunk_remaining.saturating_sub(1);

            self.iter.next()
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            (
                self.chunk_remaining,
                if self.hint_total_size {
                    Some(self.iter.len())
                } else {
                    None
                },
            )
        }
    }

    #[derive(Clone)]
    struct WindowHintedIterator<I: ExactSizeIterator + Iterator + Clone> {
        iter: I,
        window_size: usize,
        hint_total_size: bool,
    }
    impl<I: ExactSizeIterator + Iterator + Clone> Iterator for WindowHintedIterator<I> {
        type Item = I::Item;

        fn next(&mut self) -> Option<Self::Item> {
            self.iter.next()
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            (
                core::cmp::min(self.iter.len(), self.window_size),
                if self.hint_total_size {
                    Some(self.iter.len())
                } else {
                    None
                },
            )
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)] // Miri is too slow
    fn test_iterator_choose() {
        let r = &mut crate::test::rng(109);
        fn test_iter<R: Rng + ?Sized, Iter: Iterator<Item = usize> + Clone>(r: &mut R, iter: Iter) {
            let mut chosen = [0i32; 9];
            for _ in 0..1000 {
                let picked = iter.clone().choose(r).unwrap();
                chosen[picked] += 1;
            }
            for count in chosen.iter() {
                // Samples should follow Binomial(1000, 1/9)
                // Octave: binopdf(x, 1000, 1/9) gives the prob of *count == x
                // Note: have seen 153, which is unlikely but not impossible.
                assert!(
                    72 < *count && *count < 154,
                    "count not close to 1000/9: {}",
                    count
                );
            }
        }

        test_iter(r, 0..9);
        test_iter(r, [0, 1, 2, 3, 4, 5, 6, 7, 8].iter().cloned());
        #[cfg(feature = "alloc")]
        test_iter(r, (0..9).collect::<Vec<_>>().into_iter());
        test_iter(r, UnhintedIterator { iter: 0..9 });
        test_iter(
            r,
            ChunkHintedIterator {
                iter: 0..9,
                chunk_size: 4,
                chunk_remaining: 4,
                hint_total_size: false,
            },
        );
        test_iter(
            r,
            ChunkHintedIterator {
                iter: 0..9,
                chunk_size: 4,
                chunk_remaining: 4,
                hint_total_size: true,
            },
        );
        test_iter(
            r,
            WindowHintedIterator {
                iter: 0..9,
                window_size: 2,
                hint_total_size: false,
            },
        );
        test_iter(
            r,
            WindowHintedIterator {
                iter: 0..9,
                window_size: 2,
                hint_total_size: true,
            },
        );

        assert_eq!((0..0).choose(r), None);
        assert_eq!(UnhintedIterator { iter: 0..0 }.choose(r), None);
    }

    #[test]
    #[cfg_attr(miri, ignore)] // Miri is too slow
    fn test_iterator_choose_stable() {
        let r = &mut crate::test::rng(109);
        fn test_iter<R: Rng + ?Sized, Iter: Iterator<Item = usize> + Clone>(r: &mut R, iter: Iter) {
            let mut chosen = [0i32; 9];
            for _ in 0..1000 {
                let picked = iter.clone().choose_stable(r).unwrap();
                chosen[picked] += 1;
            }
            for count in chosen.iter() {
                // Samples should follow Binomial(1000, 1/9)
                // Octave: binopdf(x, 1000, 1/9) gives the prob of *count == x
                // Note: have seen 153, which is unlikely but not impossible.
                assert!(
                    72 < *count && *count < 154,
                    "count not close to 1000/9: {}",
                    count
                );
            }
        }

        test_iter(r, 0..9);
        test_iter(r, [0, 1, 2, 3, 4, 5, 6, 7, 8].iter().cloned());
        #[cfg(feature = "alloc")]
        test_iter(r, (0..9).collect::<Vec<_>>().into_iter());
        test_iter(r, UnhintedIterator { iter: 0..9 });
        test_iter(
            r,
            ChunkHintedIterator {
                iter: 0..9,
                chunk_size: 4,
                chunk_remaining: 4,
                hint_total_size: false,
            },
        );
        test_iter(
            r,
            ChunkHintedIterator {
                iter: 0..9,
                chunk_size: 4,
                chunk_remaining: 4,
                hint_total_size: true,
            },
        );
        test_iter(
            r,
            WindowHintedIterator {
                iter: 0..9,
                window_size: 2,
                hint_total_size: false,
            },
        );
        test_iter(
            r,
            WindowHintedIterator {
                iter: 0..9,
                window_size: 2,
                hint_total_size: true,
            },
        );

        assert_eq!((0..0).choose(r), None);
        assert_eq!(UnhintedIterator { iter: 0..0 }.choose(r), None);
    }

    #[test]
    #[cfg_attr(miri, ignore)] // Miri is too slow
    fn test_iterator_choose_stable_stability() {
        fn test_iter(iter: impl Iterator<Item = usize> + Clone) -> [i32; 9] {
            let r = &mut crate::test::rng(109);
            let mut chosen = [0i32; 9];
            for _ in 0..1000 {
                let picked = iter.clone().choose_stable(r).unwrap();
                chosen[picked] += 1;
            }
            chosen
        }

        let reference = test_iter(0..9);
        assert_eq!(
            test_iter([0, 1, 2, 3, 4, 5, 6, 7, 8].iter().cloned()),
            reference
        );

        #[cfg(feature = "alloc")]
        assert_eq!(test_iter((0..9).collect::<Vec<_>>().into_iter()), reference);
        assert_eq!(test_iter(UnhintedIterator { iter: 0..9 }), reference);
        assert_eq!(
            test_iter(ChunkHintedIterator {
                iter: 0..9,
                chunk_size: 4,
                chunk_remaining: 4,
                hint_total_size: false,
            }),
            reference
        );
        assert_eq!(
            test_iter(ChunkHintedIterator {
                iter: 0..9,
                chunk_size: 4,
                chunk_remaining: 4,
                hint_total_size: true,
            }),
            reference
        );
        assert_eq!(
            test_iter(WindowHintedIterator {
                iter: 0..9,
                window_size: 2,
                hint_total_size: false,
            }),
            reference
        );
        assert_eq!(
            test_iter(WindowHintedIterator {
                iter: 0..9,
                window_size: 2,
                hint_total_size: true,
            }),
            reference
        );
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_sample_iter() {
        let min_val = 1;
        let max_val = 100;

        let mut r = crate::test::rng(401);
        let vals = (min_val..max_val).collect::<Vec<i32>>();
        let small_sample = vals.iter().choose_multiple(&mut r, 5);
        let large_sample = vals.iter().choose_multiple(&mut r, vals.len() + 5);

        assert_eq!(small_sample.len(), 5);
        assert_eq!(large_sample.len(), vals.len());
        // no randomization happens when amount >= len
        assert_eq!(large_sample, vals.iter().collect::<Vec<_>>());

        assert!(small_sample
            .iter()
            .all(|e| { **e >= min_val && **e <= max_val }));
    }

    #[test]
    fn value_stability_choose() {
        fn choose<I: Iterator<Item = u32>>(iter: I) -> Option<u32> {
            let mut rng = crate::test::rng(411);
            iter.choose(&mut rng)
        }

        assert_eq!(choose([].iter().cloned()), None);
        assert_eq!(choose(0..100), Some(33));
        assert_eq!(choose(UnhintedIterator { iter: 0..100 }), Some(27));
        assert_eq!(
            choose(ChunkHintedIterator {
                iter: 0..100,
                chunk_size: 32,
                chunk_remaining: 32,
                hint_total_size: false,
            }),
            Some(91)
        );
        assert_eq!(
            choose(ChunkHintedIterator {
                iter: 0..100,
                chunk_size: 32,
                chunk_remaining: 32,
                hint_total_size: true,
            }),
            Some(91)
        );
        assert_eq!(
            choose(WindowHintedIterator {
                iter: 0..100,
                window_size: 32,
                hint_total_size: false,
            }),
            Some(34)
        );
        assert_eq!(
            choose(WindowHintedIterator {
                iter: 0..100,
                window_size: 32,
                hint_total_size: true,
            }),
            Some(34)
        );
    }

    #[test]
    fn value_stability_choose_stable() {
        fn choose<I: Iterator<Item = u32>>(iter: I) -> Option<u32> {
            let mut rng = crate::test::rng(411);
            iter.choose_stable(&mut rng)
        }

        assert_eq!(choose([].iter().cloned()), None);
        assert_eq!(choose(0..100), Some(27));
        assert_eq!(choose(UnhintedIterator { iter: 0..100 }), Some(27));
        assert_eq!(
            choose(ChunkHintedIterator {
                iter: 0..100,
                chunk_size: 32,
                chunk_remaining: 32,
                hint_total_size: false,
            }),
            Some(27)
        );
        assert_eq!(
            choose(ChunkHintedIterator {
                iter: 0..100,
                chunk_size: 32,
                chunk_remaining: 32,
                hint_total_size: true,
            }),
            Some(27)
        );
        assert_eq!(
            choose(WindowHintedIterator {
                iter: 0..100,
                window_size: 32,
                hint_total_size: false,
            }),
            Some(27)
        );
        assert_eq!(
            choose(WindowHintedIterator {
                iter: 0..100,
                window_size: 32,
                hint_total_size: true,
            }),
            Some(27)
        );
    }

    #[test]
    fn value_stability_choose_multiple() {
        fn do_test<I: Clone + Iterator<Item = u32>>(iter: I, v: &[u32]) {
            let mut rng = crate::test::rng(412);
            let mut buf = [0u32; 8];
            assert_eq!(
                iter.clone().choose_multiple_fill(&mut rng, &mut buf),
                v.len()
            );
            assert_eq!(&buf[0..v.len()], v);

            #[cfg(feature = "alloc")]
            {
                let mut rng = crate::test::rng(412);
                assert_eq!(iter.choose_multiple(&mut rng, v.len()), v);
            }
        }

        do_test(0..4, &[0, 1, 2, 3]);
        do_test(0..8, &[0, 1, 2, 3, 4, 5, 6, 7]);
        do_test(0..100, &[77, 95, 38, 23, 25, 8, 58, 40]);
    }
}
