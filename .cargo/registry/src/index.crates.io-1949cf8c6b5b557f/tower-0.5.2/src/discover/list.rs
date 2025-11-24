use super::Change;
use futures_core::Stream;
use pin_project_lite::pin_project;
use std::convert::Infallible;
use std::iter::{Enumerate, IntoIterator};
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tower_service::Service;

pin_project! {
    /// Static service discovery based on a predetermined list of services.
    ///
    /// [`ServiceList`] is created with an initial list of services. The discovery
    /// process will yield this list once and do nothing after.
    #[derive(Debug)]
    pub struct ServiceList<T>
    where
        T: IntoIterator,
    {
        inner: Enumerate<T::IntoIter>,
    }
}

impl<T, U> ServiceList<T>
where
    T: IntoIterator<Item = U>,
{
    #[allow(missing_docs)]
    pub fn new<Request>(services: T) -> ServiceList<T>
    where
        U: Service<Request>,
    {
        ServiceList {
            inner: services.into_iter().enumerate(),
        }
    }
}

impl<T, U> Stream for ServiceList<T>
where
    T: IntoIterator<Item = U>,
{
    type Item = Result<Change<usize, U>, Infallible>;

    fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.project().inner.next() {
            Some((i, service)) => Poll::Ready(Some(Ok(Change::Insert(i, service)))),
            None => Poll::Ready(None),
        }
    }
}

// check that List can be directly over collections
#[cfg(test)]
#[allow(dead_code)]
type ListVecTest<T> = ServiceList<Vec<T>>;

#[cfg(test)]
#[allow(dead_code)]
type ListVecIterTest<T> = ServiceList<::std::vec::IntoIter<T>>;
