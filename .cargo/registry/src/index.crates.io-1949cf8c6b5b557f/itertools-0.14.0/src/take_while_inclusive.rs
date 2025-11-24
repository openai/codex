use core::iter::FusedIterator;
use std::fmt;

/// An iterator adaptor that consumes elements while the given predicate is
/// `true`, including the element for which the predicate first returned
/// `false`.
///
/// See [`.take_while_inclusive()`](crate::Itertools::take_while_inclusive)
/// for more information.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Clone)]
pub struct TakeWhileInclusive<I, F> {
    iter: I,
    predicate: F,
    done: bool,
}

impl<I, F> TakeWhileInclusive<I, F>
where
    I: Iterator,
    F: FnMut(&I::Item) -> bool,
{
    /// Create a new [`TakeWhileInclusive`] from an iterator and a predicate.
    pub(crate) fn new(iter: I, predicate: F) -> Self {
        Self {
            iter,
            predicate,
            done: false,
        }
    }
}

impl<I, F> fmt::Debug for TakeWhileInclusive<I, F>
where
    I: Iterator + fmt::Debug,
{
    debug_fmt_fields!(TakeWhileInclusive, iter, done);
}

impl<I, F> Iterator for TakeWhileInclusive<I, F>
where
    I: Iterator,
    F: FnMut(&I::Item) -> bool,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            None
        } else {
            self.iter.next().map(|item| {
                if !(self.predicate)(&item) {
                    self.done = true;
                }
                item
            })
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.done {
            (0, Some(0))
        } else {
            (0, self.iter.size_hint().1)
        }
    }

    fn fold<B, Fold>(mut self, init: B, mut f: Fold) -> B
    where
        Fold: FnMut(B, Self::Item) -> B,
    {
        if self.done {
            init
        } else {
            let predicate = &mut self.predicate;
            self.iter
                .try_fold(init, |mut acc, item| {
                    let is_ok = predicate(&item);
                    acc = f(acc, item);
                    if is_ok {
                        Ok(acc)
                    } else {
                        Err(acc)
                    }
                })
                .unwrap_or_else(|err| err)
        }
    }
}

impl<I, F> FusedIterator for TakeWhileInclusive<I, F>
where
    I: Iterator,
    F: FnMut(&I::Item) -> bool,
{
}
