use std::fmt;
use std::iter::FusedIterator;

use crate::size_hint;

#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct CoalesceBy<I, F, T>
where
    I: Iterator,
{
    iter: I,
    last: Option<T>,
    f: F,
}

impl<I: Clone, F: Clone, T: Clone> Clone for CoalesceBy<I, F, T>
where
    I: Iterator,
{
    clone_fields!(last, iter, f);
}

impl<I, F, T> fmt::Debug for CoalesceBy<I, F, T>
where
    I: Iterator + fmt::Debug,
    T: fmt::Debug,
{
    debug_fmt_fields!(CoalesceBy, iter);
}

pub trait CoalescePredicate<Item, T> {
    fn coalesce_pair(&mut self, t: T, item: Item) -> Result<T, (T, T)>;
}

impl<I, F, T> Iterator for CoalesceBy<I, F, T>
where
    I: Iterator,
    F: CoalescePredicate<I::Item, T>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        // this fuses the iterator
        let last = self.last.take()?;

        let self_last = &mut self.last;
        let self_f = &mut self.f;
        Some(
            self.iter
                .try_fold(last, |last, next| match self_f.coalesce_pair(last, next) {
                    Ok(joined) => Ok(joined),
                    Err((last_, next_)) => {
                        *self_last = Some(next_);
                        Err(last_)
                    }
                })
                .unwrap_or_else(|x| x),
        )
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (low, hi) = size_hint::add_scalar(self.iter.size_hint(), self.last.is_some() as usize);
        ((low > 0) as usize, hi)
    }

    fn fold<Acc, FnAcc>(self, acc: Acc, mut fn_acc: FnAcc) -> Acc
    where
        FnAcc: FnMut(Acc, Self::Item) -> Acc,
    {
        if let Some(last) = self.last {
            let mut f = self.f;
            let (last, acc) = self.iter.fold((last, acc), |(last, acc), elt| {
                match f.coalesce_pair(last, elt) {
                    Ok(joined) => (joined, acc),
                    Err((last_, next_)) => (next_, fn_acc(acc, last_)),
                }
            });
            fn_acc(acc, last)
        } else {
            acc
        }
    }
}

impl<I: Iterator, F: CoalescePredicate<I::Item, T>, T> FusedIterator for CoalesceBy<I, F, T> {}

/// An iterator adaptor that may join together adjacent elements.
///
/// See [`.coalesce()`](crate::Itertools::coalesce) for more information.
pub type Coalesce<I, F> = CoalesceBy<I, F, <I as Iterator>::Item>;

impl<F, Item, T> CoalescePredicate<Item, T> for F
where
    F: FnMut(T, Item) -> Result<T, (T, T)>,
{
    fn coalesce_pair(&mut self, t: T, item: Item) -> Result<T, (T, T)> {
        self(t, item)
    }
}

/// Create a new `Coalesce`.
pub fn coalesce<I, F>(mut iter: I, f: F) -> Coalesce<I, F>
where
    I: Iterator,
{
    Coalesce {
        last: iter.next(),
        iter,
        f,
    }
}

/// An iterator adaptor that removes repeated duplicates, determining equality using a comparison function.
///
/// See [`.dedup_by()`](crate::Itertools::dedup_by) or [`.dedup()`](crate::Itertools::dedup) for more information.
pub type DedupBy<I, Pred> = CoalesceBy<I, DedupPred2CoalescePred<Pred>, <I as Iterator>::Item>;

#[derive(Clone)]
pub struct DedupPred2CoalescePred<DP>(DP);

impl<DP> fmt::Debug for DedupPred2CoalescePred<DP> {
    debug_fmt_fields!(DedupPred2CoalescePred,);
}

pub trait DedupPredicate<T> {
    // TODO replace by Fn(&T, &T)->bool once Rust supports it
    fn dedup_pair(&mut self, a: &T, b: &T) -> bool;
}

impl<DP, T> CoalescePredicate<T, T> for DedupPred2CoalescePred<DP>
where
    DP: DedupPredicate<T>,
{
    fn coalesce_pair(&mut self, t: T, item: T) -> Result<T, (T, T)> {
        if self.0.dedup_pair(&t, &item) {
            Ok(t)
        } else {
            Err((t, item))
        }
    }
}

#[derive(Clone, Debug)]
pub struct DedupEq;

impl<T: PartialEq> DedupPredicate<T> for DedupEq {
    fn dedup_pair(&mut self, a: &T, b: &T) -> bool {
        a == b
    }
}

impl<T, F: FnMut(&T, &T) -> bool> DedupPredicate<T> for F {
    fn dedup_pair(&mut self, a: &T, b: &T) -> bool {
        self(a, b)
    }
}

/// Create a new `DedupBy`.
pub fn dedup_by<I, Pred>(mut iter: I, dedup_pred: Pred) -> DedupBy<I, Pred>
where
    I: Iterator,
{
    DedupBy {
        last: iter.next(),
        iter,
        f: DedupPred2CoalescePred(dedup_pred),
    }
}

/// An iterator adaptor that removes repeated duplicates.
///
/// See [`.dedup()`](crate::Itertools::dedup) for more information.
pub type Dedup<I> = DedupBy<I, DedupEq>;

/// Create a new `Dedup`.
pub fn dedup<I>(iter: I) -> Dedup<I>
where
    I: Iterator,
{
    dedup_by(iter, DedupEq)
}

/// An iterator adaptor that removes repeated duplicates, while keeping a count of how many
/// repeated elements were present. This will determine equality using a comparison function.
///
/// See [`.dedup_by_with_count()`](crate::Itertools::dedup_by_with_count) or
/// [`.dedup_with_count()`](crate::Itertools::dedup_with_count) for more information.
pub type DedupByWithCount<I, Pred> =
    CoalesceBy<I, DedupPredWithCount2CoalescePred<Pred>, (usize, <I as Iterator>::Item)>;

#[derive(Clone, Debug)]
pub struct DedupPredWithCount2CoalescePred<DP>(DP);

impl<DP, T> CoalescePredicate<T, (usize, T)> for DedupPredWithCount2CoalescePred<DP>
where
    DP: DedupPredicate<T>,
{
    fn coalesce_pair(
        &mut self,
        (c, t): (usize, T),
        item: T,
    ) -> Result<(usize, T), ((usize, T), (usize, T))> {
        if self.0.dedup_pair(&t, &item) {
            Ok((c + 1, t))
        } else {
            Err(((c, t), (1, item)))
        }
    }
}

/// An iterator adaptor that removes repeated duplicates, while keeping a count of how many
/// repeated elements were present.
///
/// See [`.dedup_with_count()`](crate::Itertools::dedup_with_count) for more information.
pub type DedupWithCount<I> = DedupByWithCount<I, DedupEq>;

/// Create a new `DedupByWithCount`.
pub fn dedup_by_with_count<I, Pred>(mut iter: I, dedup_pred: Pred) -> DedupByWithCount<I, Pred>
where
    I: Iterator,
{
    DedupByWithCount {
        last: iter.next().map(|v| (1, v)),
        iter,
        f: DedupPredWithCount2CoalescePred(dedup_pred),
    }
}

/// Create a new `DedupWithCount`.
pub fn dedup_with_count<I>(iter: I) -> DedupWithCount<I>
where
    I: Iterator,
{
    dedup_by_with_count(iter, DedupEq)
}
