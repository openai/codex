//! Middleware that provides a buffered mpsc channel to a service.
//!
//! Sometimes you want to give out multiple handles to a single service, and allow each handle to
//! enqueue requests. That is, you want a [`Service`] to be [`Clone`]. This module allows you to do
//! that by placing the service behind a multi-producer, single-consumer buffering channel. Clients
//! enqueue requests by sending on the channel from any of the handles ([`Buffer`]), and the single
//! service running elsewhere (usually spawned) receives and services the requests one by one. Each
//! request is enqueued alongside a response channel that allows the service to report the result
//! of the request back to the caller.
//!
//! # Examples
//!
//! ```rust
//! # #[cfg(feature = "util")]
//! use tower::buffer::Buffer;
//! # #[cfg(feature = "util")]
//! use tower::{Service, ServiceExt};
//! # #[cfg(feature = "util")]
//! async fn mass_produce<S: Service<usize>>(svc: S)
//! where
//!   S: 'static + Send,
//!   S::Error: Send + Sync + std::error::Error,
//!   S::Future: Send
//! {
//!     let svc = Buffer::new(svc, 10 /* buffer length */);
//!     for _ in 0..10 {
//!         let mut svc = svc.clone();
//!         tokio::spawn(async move {
//!             for i in 0usize.. {
//!                 svc.ready().await.expect("service crashed").call(i).await;
//!             }
//!         });
//!     }
//! }
//! ```
//!
//! [`Service`]: crate::Service

pub mod error;
pub mod future;
mod layer;
mod message;
mod service;
mod worker;

pub use self::layer::BufferLayer;
pub use self::service::Buffer;
