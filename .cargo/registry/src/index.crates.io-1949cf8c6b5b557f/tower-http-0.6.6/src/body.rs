//! Body types.
//!
//! All these are wrappers around other body types. You shouldn't have to use them in your code.
//! Use `http-body-util` instead.
//!
//! They exist because we don't want to expose types from `http-body-util` in `tower-http`s public
//! API.

#![allow(missing_docs)]

use std::convert::Infallible;

use bytes::{Buf, Bytes};
use http_body::Body;
use pin_project_lite::pin_project;

use crate::BoxError;

macro_rules! body_methods {
    () => {
        #[inline]
        fn poll_frame(
            self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
            self.project().inner.poll_frame(cx)
        }

        #[inline]
        fn is_end_stream(&self) -> bool {
            Body::is_end_stream(&self.inner)
        }

        #[inline]
        fn size_hint(&self) -> http_body::SizeHint {
            Body::size_hint(&self.inner)
        }
    };
}

pin_project! {
    #[derive(Default)]
    pub struct Full {
        #[pin]
        pub(crate) inner: http_body_util::Full<Bytes>
    }
}

impl Full {
    #[allow(dead_code)]
    pub(crate) fn new(inner: http_body_util::Full<Bytes>) -> Self {
        Self { inner }
    }
}

impl Body for Full {
    type Data = Bytes;
    type Error = Infallible;

    body_methods!();
}

pin_project! {
    pub struct Limited<B> {
        #[pin]
        pub(crate) inner: http_body_util::Limited<B>
    }
}

impl<B> Limited<B> {
    #[allow(dead_code)]
    pub(crate) fn new(inner: http_body_util::Limited<B>) -> Self {
        Self { inner }
    }
}

impl<B> Body for Limited<B>
where
    B: Body,
    B::Error: Into<BoxError>,
{
    type Data = B::Data;
    type Error = BoxError;

    body_methods!();
}

pin_project! {
    pub struct UnsyncBoxBody<D, E> {
        #[pin]
        pub(crate) inner: http_body_util::combinators::UnsyncBoxBody<D, E>
    }
}

impl<D, E> Default for UnsyncBoxBody<D, E>
where
    D: Buf + 'static,
{
    fn default() -> Self {
        Self {
            inner: Default::default(),
        }
    }
}

impl<D, E> UnsyncBoxBody<D, E> {
    #[allow(dead_code)]
    pub(crate) fn new(inner: http_body_util::combinators::UnsyncBoxBody<D, E>) -> Self {
        Self { inner }
    }
}

impl<D, E> Body for UnsyncBoxBody<D, E>
where
    D: Buf,
{
    type Data = D;
    type Error = E;

    body_methods!();
}
