use std::fmt;
use std::iter::FusedIterator;

use crate::size_hint;

#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct CoalesceBy<I, F, C>
where
    I: Iterator,
    C: CountItem<I::Item>,
{
    iter: I,
    /// `last` is `None` while no item have been taken out of `iter` (at definition).
    /// Then `last` will be `Some(Some(item))` until `iter` is exhausted,
    /// in which case `last` will be `Some(None)`.
    last: Option<Option<C::CItem>>,
    f: F,
}

impl<I, F, C> Clone for CoalesceBy<I, F, C>
where
    I: Clone + Iterator,
    F: Clone,
    C: CountItem<I::Item>,
    C::CItem: Clone,
{
    clone_fields!(last, iter, f);
}

impl<I, F, C> fmt::Debug for CoalesceBy<I, F, C>
where
    I: Iterator + fmt::Debug,
    C: CountItem<I::Item>,
    C::CItem: fmt::Debug,
{
    debug_fmt_fields!(CoalesceBy, iter, last);
}

pub trait CoalescePredicate<Item, T> {
    fn coalesce_pair(&mut self, t: T, item: Item) -> Result<T, (T, T)>;
}

impl<I, F, C> Iterator for CoalesceBy<I, F, C>
where
    I: Iterator,
    F: CoalescePredicate<I::Item, C::CItem>,
    C: CountItem<I::Item>,
{
    type Item = C::CItem;

    fn next(&mut self) -> Option<Self::Item> {
        let Self { iter, last, f } = self;
        // this fuses the iterator
        let init = match last {
            Some(elt) => elt.take(),
            None => {
                *last = Some(None);
                iter.next().map(C::new)
            }
        }?;

        Some(
            iter.try_fold(init, |accum, next| match f.coalesce_pair(accum, next) {
                Ok(joined) => Ok(joined),
                Err((last_, next_)) => {
                    *last = Some(Some(next_));
                    Err(last_)
                }
            })
            .unwrap_or_else(|x| x),
        )
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (low, hi) = size_hint::add_scalar(
            self.iter.size_hint(),
            matches!(self.last, Some(Some(_))) as usize,
        );
        ((low > 0) as usize, hi)
    }

    fn fold<Acc, FnAcc>(self, acc: Acc, mut fn_acc: FnAcc) -> Acc
    where
        FnAcc: FnMut(Acc, Self::Item) -> Acc,
    {
        let Self {
            mut iter,
            last,
            mut f,
        } = self;
        if let Some(last) = last.unwrap_or_else(|| iter.next().map(C::new)) {
            let (last, acc) = iter.fold((last, acc), |(last, acc), elt| {
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

impl<I, F, C> FusedIterator for CoalesceBy<I, F, C>
where
    I: Iterator,
    F: CoalescePredicate<I::Item, C::CItem>,
    C: CountItem<I::Item>,
{
}

pub struct NoCount;

pub struct WithCount;

pub trait CountItem<T> {
    type CItem;
    fn new(t: T) -> Self::CItem;
}

impl<T> CountItem<T> for NoCount {
    type CItem = T;
    #[inline(always)]
    fn new(t: T) -> T {
        t
    }
}

impl<T> CountItem<T> for WithCount {
    type CItem = (usize, T);
    #[inline(always)]
    fn new(t: T) -> (usize, T) {
        (1, t)
    }
}

/// An iterator adaptor that may join together adjacent elements.
///
/// See [`.coalesce()`](crate::Itertools::coalesce) for more information.
pub type Coalesce<I, F> = CoalesceBy<I, F, NoCount>;

impl<F, Item, T> CoalescePredicate<Item, T> for F
where
    F: FnMut(T, Item) -> Result<T, (T, T)>,
{
    fn coalesce_pair(&mut self, t: T, item: Item) -> Result<T, (T, T)> {
        self(t, item)
    }
}

/// Create a new `Coalesce`.
pub fn coalesce<I, F>(iter: I, f: F) -> Coalesce<I, F>
where
    I: Iterator,
{
    Coalesce {
        last: None,
        iter,
        f,
    }
}

/// An iterator adaptor that removes repeated duplicates, determining equality using a comparison function.
///
/// See [`.dedup_by()`](crate::Itertools::dedup_by) or [`.dedup()`](crate::Itertools::dedup) for more information.
pub type DedupBy<I, Pred> = CoalesceBy<I, DedupPred2CoalescePred<Pred>, NoCount>;

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
pub fn dedup_by<I, Pred>(iter: I, dedup_pred: Pred) -> DedupBy<I, Pred>
where
    I: Iterator,
{
    DedupBy {
        last: None,
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
    CoalesceBy<I, DedupPredWithCount2CoalescePred<Pred>, WithCount>;

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
pub fn dedup_by_with_count<I, Pred>(iter: I, dedup_pred: Pred) -> DedupByWithCount<I, Pred>
where
    I: Iterator,
{
    DedupByWithCount {
        last: None,
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
