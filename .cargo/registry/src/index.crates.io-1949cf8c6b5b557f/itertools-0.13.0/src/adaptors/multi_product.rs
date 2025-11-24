#![cfg(feature = "use_alloc")]
use Option::{self as State, None as ProductEnded, Some as ProductInProgress};
use Option::{self as CurrentItems, None as NotYetPopulated, Some as Populated};

use alloc::vec::Vec;

use crate::size_hint;

#[derive(Clone)]
/// An iterator adaptor that iterates over the cartesian product of
/// multiple iterators of type `I`.
///
/// An iterator element type is `Vec<I::Item>`.
///
/// See [`.multi_cartesian_product()`](crate::Itertools::multi_cartesian_product)
/// for more information.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct MultiProduct<I>(State<MultiProductInner<I>>)
where
    I: Iterator + Clone,
    I::Item: Clone;

#[derive(Clone)]
/// Internals for `MultiProduct`.
struct MultiProductInner<I>
where
    I: Iterator + Clone,
    I::Item: Clone,
{
    /// Holds the iterators.
    iters: Vec<MultiProductIter<I>>,
    /// Not populated at the beginning then it holds the current item of each iterator.
    cur: CurrentItems<Vec<I::Item>>,
}

impl<I> std::fmt::Debug for MultiProduct<I>
where
    I: Iterator + Clone + std::fmt::Debug,
    I::Item: Clone + std::fmt::Debug,
{
    debug_fmt_fields!(MultiProduct, 0);
}

impl<I> std::fmt::Debug for MultiProductInner<I>
where
    I: Iterator + Clone + std::fmt::Debug,
    I::Item: Clone + std::fmt::Debug,
{
    debug_fmt_fields!(MultiProductInner, iters, cur);
}

/// Create a new cartesian product iterator over an arbitrary number
/// of iterators of the same type.
///
/// Iterator element is of type `Vec<H::Item::Item>`.
pub fn multi_cartesian_product<H>(iters: H) -> MultiProduct<<H::Item as IntoIterator>::IntoIter>
where
    H: Iterator,
    H::Item: IntoIterator,
    <H::Item as IntoIterator>::IntoIter: Clone,
    <H::Item as IntoIterator>::Item: Clone,
{
    let inner = MultiProductInner {
        iters: iters
            .map(|i| MultiProductIter::new(i.into_iter()))
            .collect(),
        cur: NotYetPopulated,
    };
    MultiProduct(ProductInProgress(inner))
}

#[derive(Clone, Debug)]
/// Holds the state of a single iterator within a `MultiProduct`.
struct MultiProductIter<I>
where
    I: Iterator + Clone,
    I::Item: Clone,
{
    iter: I,
    iter_orig: I,
}

impl<I> MultiProductIter<I>
where
    I: Iterator + Clone,
    I::Item: Clone,
{
    fn new(iter: I) -> Self {
        Self {
            iter: iter.clone(),
            iter_orig: iter,
        }
    }
}

impl<I> Iterator for MultiProduct<I>
where
    I: Iterator + Clone,
    I::Item: Clone,
{
    type Item = Vec<I::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        // This fuses the iterator.
        let inner = self.0.as_mut()?;
        match &mut inner.cur {
            Populated(values) => {
                debug_assert!(!inner.iters.is_empty());
                // Find (from the right) a non-finished iterator and
                // reset the finished ones encountered.
                for (iter, item) in inner.iters.iter_mut().zip(values.iter_mut()).rev() {
                    if let Some(new) = iter.iter.next() {
                        *item = new;
                        return Some(values.clone());
                    } else {
                        iter.iter = iter.iter_orig.clone();
                        // `cur` is populated so the untouched `iter_orig` can not be empty.
                        *item = iter.iter.next().unwrap();
                    }
                }
                self.0 = ProductEnded;
                None
            }
            // Only the first time.
            NotYetPopulated => {
                let next: Option<Vec<_>> = inner.iters.iter_mut().map(|i| i.iter.next()).collect();
                if next.is_none() || inner.iters.is_empty() {
                    // This cartesian product had at most one item to generate and now ends.
                    self.0 = ProductEnded;
                } else {
                    inner.cur.clone_from(&next);
                }
                next
            }
        }
    }

    fn count(self) -> usize {
        match self.0 {
            ProductEnded => 0,
            // The iterator is fresh so the count is the product of the length of each iterator:
            // - If one of them is empty, stop counting.
            // - Less `count()` calls than the general case.
            ProductInProgress(MultiProductInner {
                iters,
                cur: NotYetPopulated,
            }) => iters
                .into_iter()
                .map(|iter| iter.iter_orig.count())
                .try_fold(1, |product, count| {
                    if count == 0 {
                        None
                    } else {
                        Some(product * count)
                    }
                })
                .unwrap_or_default(),
            // The general case.
            ProductInProgress(MultiProductInner {
                iters,
                cur: Populated(_),
            }) => iters.into_iter().fold(0, |mut acc, iter| {
                if acc != 0 {
                    acc *= iter.iter_orig.count();
                }
                acc + iter.iter.count()
            }),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.0 {
            ProductEnded => (0, Some(0)),
            ProductInProgress(MultiProductInner {
                iters,
                cur: NotYetPopulated,
            }) => iters
                .iter()
                .map(|iter| iter.iter_orig.size_hint())
                .fold((1, Some(1)), size_hint::mul),
            ProductInProgress(MultiProductInner {
                iters,
                cur: Populated(_),
            }) => {
                if let [first, tail @ ..] = &iters[..] {
                    tail.iter().fold(first.iter.size_hint(), |mut sh, iter| {
                        sh = size_hint::mul(sh, iter.iter_orig.size_hint());
                        size_hint::add(sh, iter.iter.size_hint())
                    })
                } else {
                    // Since it is populated, this cartesian product has started so `iters` is not empty.
                    unreachable!()
                }
            }
        }
    }

    fn last(self) -> Option<Self::Item> {
        let MultiProductInner { iters, cur } = self.0?;
        // Collect the last item of each iterator of the product.
        if let Populated(values) = cur {
            let mut count = iters.len();
            let last = iters
                .into_iter()
                .zip(values)
                .map(|(i, value)| {
                    i.iter.last().unwrap_or_else(|| {
                        // The iterator is empty, use its current `value`.
                        count -= 1;
                        value
                    })
                })
                .collect();
            if count == 0 {
                // `values` was the last item.
                None
            } else {
                Some(last)
            }
        } else {
            iters.into_iter().map(|i| i.iter.last()).collect()
        }
    }
}

impl<I> std::iter::FusedIterator for MultiProduct<I>
where
    I: Iterator + Clone,
    I::Item: Clone,
{
}
