use crate::Stream;

use core::future::Future;
use core::marker::PhantomPinned;
use core::mem;
use core::pin::Pin;
use core::task::{ready, Context, Poll};
use pin_project_lite::pin_project;

// Do not export this struct until `FromStream` can be unsealed.
pin_project! {
    /// Future returned by the [`collect`](super::StreamExt::collect) method.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    #[derive(Debug)]
    pub struct Collect<T, U>
    where
        T: Stream,
        U: FromStream<T::Item>,
    {
        #[pin]
        stream: T,
        collection: U::InternalCollection,
        // Make this future `!Unpin` for compatibility with async trait methods.
        #[pin]
        _pin: PhantomPinned,
    }
}

/// Convert from a [`Stream`].
///
/// This trait is not intended to be used directly. Instead, call
/// [`StreamExt::collect()`](super::StreamExt::collect).
///
/// # Implementing
///
/// Currently, this trait may not be implemented by third parties. The trait is
/// sealed in order to make changes in the future. Stabilization is pending
/// enhancements to the Rust language.
pub trait FromStream<T>: sealed::FromStreamPriv<T> {}

impl<T, U> Collect<T, U>
where
    T: Stream,
    U: FromStream<T::Item>,
{
    pub(super) fn new(stream: T) -> Collect<T, U> {
        let (lower, upper) = stream.size_hint();
        let collection = U::initialize(sealed::Internal, lower, upper);

        Collect {
            stream,
            collection,
            _pin: PhantomPinned,
        }
    }
}

impl<T, U> Future for Collect<T, U>
where
    T: Stream,
    U: FromStream<T::Item>,
{
    type Output = U;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<U> {
        use Poll::Ready;

        loop {
            let me = self.as_mut().project();

            let item = match ready!(me.stream.poll_next(cx)) {
                Some(item) => item,
                None => {
                    return Ready(U::finalize(sealed::Internal, me.collection));
                }
            };

            if !U::extend(sealed::Internal, me.collection, item) {
                return Ready(U::finalize(sealed::Internal, me.collection));
            }
        }
    }
}

// ===== FromStream implementations

impl FromStream<()> for () {}

impl sealed::FromStreamPriv<()> for () {
    type InternalCollection = ();

    fn initialize(_: sealed::Internal, _lower: usize, _upper: Option<usize>) {}

    fn extend(_: sealed::Internal, _collection: &mut (), _item: ()) -> bool {
        true
    }

    fn finalize(_: sealed::Internal, _collection: &mut ()) {}
}

impl<T: AsRef<str>> FromStream<T> for String {}

impl<T: AsRef<str>> sealed::FromStreamPriv<T> for String {
    type InternalCollection = String;

    fn initialize(_: sealed::Internal, _lower: usize, _upper: Option<usize>) -> String {
        String::new()
    }

    fn extend(_: sealed::Internal, collection: &mut String, item: T) -> bool {
        collection.push_str(item.as_ref());
        true
    }

    fn finalize(_: sealed::Internal, collection: &mut String) -> String {
        mem::take(collection)
    }
}

impl<T> FromStream<T> for Vec<T> {}

impl<T> sealed::FromStreamPriv<T> for Vec<T> {
    type InternalCollection = Vec<T>;

    fn initialize(_: sealed::Internal, lower: usize, _upper: Option<usize>) -> Vec<T> {
        Vec::with_capacity(lower)
    }

    fn extend(_: sealed::Internal, collection: &mut Vec<T>, item: T) -> bool {
        collection.push(item);
        true
    }

    fn finalize(_: sealed::Internal, collection: &mut Vec<T>) -> Vec<T> {
        mem::take(collection)
    }
}

impl<T> FromStream<T> for Box<[T]> {}

impl<T> sealed::FromStreamPriv<T> for Box<[T]> {
    type InternalCollection = Vec<T>;

    fn initialize(_: sealed::Internal, lower: usize, upper: Option<usize>) -> Vec<T> {
        <Vec<T> as sealed::FromStreamPriv<T>>::initialize(sealed::Internal, lower, upper)
    }

    fn extend(_: sealed::Internal, collection: &mut Vec<T>, item: T) -> bool {
        <Vec<T> as sealed::FromStreamPriv<T>>::extend(sealed::Internal, collection, item)
    }

    fn finalize(_: sealed::Internal, collection: &mut Vec<T>) -> Box<[T]> {
        <Vec<T> as sealed::FromStreamPriv<T>>::finalize(sealed::Internal, collection)
            .into_boxed_slice()
    }
}

impl<T, U, E> FromStream<Result<T, E>> for Result<U, E> where U: FromStream<T> {}

impl<T, U, E> sealed::FromStreamPriv<Result<T, E>> for Result<U, E>
where
    U: FromStream<T>,
{
    type InternalCollection = Result<U::InternalCollection, E>;

    fn initialize(
        _: sealed::Internal,
        lower: usize,
        upper: Option<usize>,
    ) -> Result<U::InternalCollection, E> {
        Ok(U::initialize(sealed::Internal, lower, upper))
    }

    fn extend(
        _: sealed::Internal,
        collection: &mut Self::InternalCollection,
        item: Result<T, E>,
    ) -> bool {
        assert!(collection.is_ok());
        match item {
            Ok(item) => {
                let collection = collection.as_mut().ok().expect("invalid state");
                U::extend(sealed::Internal, collection, item)
            }
            Err(err) => {
                *collection = Err(err);
                false
            }
        }
    }

    fn finalize(_: sealed::Internal, collection: &mut Self::InternalCollection) -> Result<U, E> {
        if let Ok(collection) = collection.as_mut() {
            Ok(U::finalize(sealed::Internal, collection))
        } else {
            let res = mem::replace(collection, Ok(U::initialize(sealed::Internal, 0, Some(0))));

            Err(res.map(drop).unwrap_err())
        }
    }
}

pub(crate) mod sealed {
    #[doc(hidden)]
    pub trait FromStreamPriv<T> {
        /// Intermediate type used during collection process
        ///
        /// The name of this type is internal and cannot be relied upon.
        type InternalCollection;

        /// Initialize the collection
        fn initialize(
            internal: Internal,
            lower: usize,
            upper: Option<usize>,
        ) -> Self::InternalCollection;

        /// Extend the collection with the received item
        ///
        /// Return `true` to continue streaming, `false` complete collection.
        fn extend(internal: Internal, collection: &mut Self::InternalCollection, item: T) -> bool;

        /// Finalize collection into target type.
        fn finalize(internal: Internal, collection: &mut Self::InternalCollection) -> Self;
    }

    #[allow(missing_debug_implementations)]
    pub struct Internal;
}
