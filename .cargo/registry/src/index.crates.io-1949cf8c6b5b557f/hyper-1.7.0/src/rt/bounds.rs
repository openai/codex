//! Trait aliases
//!
//! Traits in this module ease setting bounds and usually automatically
//! implemented by implementing another trait.

#[cfg(all(feature = "server", feature = "http2"))]
pub use self::h2::Http2ServerConnExec;

#[cfg(all(feature = "client", feature = "http2"))]
pub use self::h2_client::Http2ClientConnExec;

#[cfg(all(feature = "client", feature = "http2"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "client", feature = "http2"))))]
mod h2_client {
    use std::{error::Error, future::Future};

    use crate::rt::{Read, Write};
    use crate::{proto::h2::client::H2ClientFuture, rt::Executor};

    /// An executor to spawn http2 futures for the client.
    ///
    /// This trait is implemented for any type that implements [`Executor`]
    /// trait for any future.
    ///
    /// This trait is sealed and cannot be implemented for types outside this crate.
    ///
    /// [`Executor`]: crate::rt::Executor
    pub trait Http2ClientConnExec<B, T>: sealed_client::Sealed<(B, T)>
    where
        B: http_body::Body,
        B::Error: Into<Box<dyn Error + Send + Sync>>,
        T: Read + Write + Unpin,
    {
        #[doc(hidden)]
        fn execute_h2_future(&mut self, future: H2ClientFuture<B, T>);
    }

    impl<E, B, T> Http2ClientConnExec<B, T> for E
    where
        E: Executor<H2ClientFuture<B, T>>,
        B: http_body::Body + 'static,
        B::Error: Into<Box<dyn Error + Send + Sync>>,
        H2ClientFuture<B, T>: Future<Output = ()>,
        T: Read + Write + Unpin,
    {
        fn execute_h2_future(&mut self, future: H2ClientFuture<B, T>) {
            self.execute(future)
        }
    }

    impl<E, B, T> sealed_client::Sealed<(B, T)> for E
    where
        E: Executor<H2ClientFuture<B, T>>,
        B: http_body::Body + 'static,
        B::Error: Into<Box<dyn Error + Send + Sync>>,
        H2ClientFuture<B, T>: Future<Output = ()>,
        T: Read + Write + Unpin,
    {
    }

    mod sealed_client {
        pub trait Sealed<X> {}
    }
}

#[cfg(all(feature = "server", feature = "http2"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "server", feature = "http2"))))]
mod h2 {
    use crate::{proto::h2::server::H2Stream, rt::Executor};
    use http_body::Body;
    use std::future::Future;

    /// An executor to spawn http2 connections.
    ///
    /// This trait is implemented for any type that implements [`Executor`]
    /// trait for any future.
    ///
    /// This trait is sealed and cannot be implemented for types outside this crate.
    ///
    /// [`Executor`]: crate::rt::Executor
    pub trait Http2ServerConnExec<F, B: Body>: sealed::Sealed<(F, B)> + Clone {
        #[doc(hidden)]
        fn execute_h2stream(&mut self, fut: H2Stream<F, B>);
    }

    #[doc(hidden)]
    impl<E, F, B> Http2ServerConnExec<F, B> for E
    where
        E: Executor<H2Stream<F, B>> + Clone,
        H2Stream<F, B>: Future<Output = ()>,
        B: Body,
    {
        fn execute_h2stream(&mut self, fut: H2Stream<F, B>) {
            self.execute(fut)
        }
    }

    impl<E, F, B> sealed::Sealed<(F, B)> for E
    where
        E: Executor<H2Stream<F, B>> + Clone,
        H2Stream<F, B>: Future<Output = ()>,
        B: Body,
    {
    }

    mod sealed {
        pub trait Sealed<T> {}
    }
}
