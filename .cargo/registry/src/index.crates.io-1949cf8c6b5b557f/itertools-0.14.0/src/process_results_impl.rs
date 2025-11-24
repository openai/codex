#[cfg(doc)]
use crate::Itertools;

/// An iterator that produces only the `T` values as long as the
/// inner iterator produces `Ok(T)`.
///
/// Used by [`process_results`](crate::process_results), see its docs
/// for more information.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Debug)]
pub struct ProcessResults<'a, I, E: 'a> {
    error: &'a mut Result<(), E>,
    iter: I,
}

impl<I, E> ProcessResults<'_, I, E> {
    #[inline(always)]
    fn next_body<T>(&mut self, item: Option<Result<T, E>>) -> Option<T> {
        match item {
            Some(Ok(x)) => Some(x),
            Some(Err(e)) => {
                *self.error = Err(e);
                None
            }
            None => None,
        }
    }
}

impl<I, T, E> Iterator for ProcessResults<'_, I, E>
where
    I: Iterator<Item = Result<T, E>>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.iter.next();
        self.next_body(item)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.iter.size_hint().1)
    }

    fn fold<B, F>(mut self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        let error = self.error;
        self.iter
            .try_fold(init, |acc, opt| match opt {
                Ok(x) => Ok(f(acc, x)),
                Err(e) => {
                    *error = Err(e);
                    Err(acc)
                }
            })
            .unwrap_or_else(|e| e)
    }
}

impl<I, T, E> DoubleEndedIterator for ProcessResults<'_, I, E>
where
    I: Iterator<Item = Result<T, E>>,
    I: DoubleEndedIterator,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        let item = self.iter.next_back();
        self.next_body(item)
    }

    fn rfold<B, F>(mut self, init: B, mut f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        let error = self.error;
        self.iter
            .try_rfold(init, |acc, opt| match opt {
                Ok(x) => Ok(f(acc, x)),
                Err(e) => {
                    *error = Err(e);
                    Err(acc)
                }
            })
            .unwrap_or_else(|e| e)
    }
}

/// “Lift” a function of the values of an iterator so that it can process
/// an iterator of `Result` values instead.
///
/// [`IntoIterator`] enabled version of [`Itertools::process_results`].
pub fn process_results<I, F, T, E, R>(iterable: I, processor: F) -> Result<R, E>
where
    I: IntoIterator<Item = Result<T, E>>,
    F: FnOnce(ProcessResults<I::IntoIter, E>) -> R,
{
    let iter = iterable.into_iter();
    let mut error = Ok(());

    let result = processor(ProcessResults {
        error: &mut error,
        iter,
    });

    error.map(|_| result)
}
