//! Dispatches trace events to [`Subscriber`]s.
//!
//! The _dispatcher_ is the component of the tracing system which is responsible
//! for forwarding trace data from the instrumentation points that generate it
//! to the subscriber that collects it.
//!
//! # Using the Trace Dispatcher
//!
//! Every thread in a program using `tracing` has a _default subscriber_. When
//! events occur, or spans are created, they are dispatched to the thread's
//! current subscriber.
//!
//! ## Setting the Default Subscriber
//!
//! By default, the current subscriber is an empty implementation that does
//! nothing. To use a subscriber implementation, it must be set as the default.
//! There are two methods for doing so: [`with_default`] and
//! [`set_global_default`]. `with_default` sets the default subscriber for the
//! duration of a scope, while `set_global_default` sets a default subscriber
//! for the entire process.
//!
//! To use either of these functions, we must first wrap our subscriber in a
//! [`Dispatch`], a cloneable, type-erased reference to a subscriber. For
//! example:
//! ```rust
//! # pub struct FooSubscriber;
//! # use tracing_core::{
//! #   dispatcher, Event, Metadata,
//! #   span::{Attributes, Id, Record}
//! # };
//! # impl tracing_core::Subscriber for FooSubscriber {
//! #   fn new_span(&self, _: &Attributes) -> Id { Id::from_u64(0) }
//! #   fn record(&self, _: &Id, _: &Record) {}
//! #   fn event(&self, _: &Event) {}
//! #   fn record_follows_from(&self, _: &Id, _: &Id) {}
//! #   fn enabled(&self, _: &Metadata) -> bool { false }
//! #   fn enter(&self, _: &Id) {}
//! #   fn exit(&self, _: &Id) {}
//! # }
//! # impl FooSubscriber { fn new() -> Self { FooSubscriber } }
//! use dispatcher::Dispatch;
//!
//! let my_subscriber = FooSubscriber::new();
//! let my_dispatch = Dispatch::new(my_subscriber);
//! ```
//! Then, we can use [`with_default`] to set our `Dispatch` as the default for
//! the duration of a block:
//! ```rust
//! # pub struct FooSubscriber;
//! # use tracing_core::{
//! #   dispatcher, Event, Metadata,
//! #   span::{Attributes, Id, Record}
//! # };
//! # impl tracing_core::Subscriber for FooSubscriber {
//! #   fn new_span(&self, _: &Attributes) -> Id { Id::from_u64(0) }
//! #   fn record(&self, _: &Id, _: &Record) {}
//! #   fn event(&self, _: &Event) {}
//! #   fn record_follows_from(&self, _: &Id, _: &Id) {}
//! #   fn enabled(&self, _: &Metadata) -> bool { false }
//! #   fn enter(&self, _: &Id) {}
//! #   fn exit(&self, _: &Id) {}
//! # }
//! # impl FooSubscriber { fn new() -> Self { FooSubscriber } }
//! # let my_subscriber = FooSubscriber::new();
//! # let my_dispatch = dispatcher::Dispatch::new(my_subscriber);
//! // no default subscriber
//!
//! # #[cfg(feature = "std")]
//! dispatcher::with_default(&my_dispatch, || {
//!     // my_subscriber is the default
//! });
//!
//! // no default subscriber again
//! ```
//! It's important to note that `with_default` will not propagate the current
//! thread's default subscriber to any threads spawned within the `with_default`
//! block. To propagate the default subscriber to new threads, either use
//! `with_default` from the new thread, or use `set_global_default`.
//!
//! As an alternative to `with_default`, we can use [`set_global_default`] to
//! set a `Dispatch` as the default for all threads, for the lifetime of the
//! program. For example:
//! ```rust
//! # pub struct FooSubscriber;
//! # use tracing_core::{
//! #   dispatcher, Event, Metadata,
//! #   span::{Attributes, Id, Record}
//! # };
//! # impl tracing_core::Subscriber for FooSubscriber {
//! #   fn new_span(&self, _: &Attributes) -> Id { Id::from_u64(0) }
//! #   fn record(&self, _: &Id, _: &Record) {}
//! #   fn event(&self, _: &Event) {}
//! #   fn record_follows_from(&self, _: &Id, _: &Id) {}
//! #   fn enabled(&self, _: &Metadata) -> bool { false }
//! #   fn enter(&self, _: &Id) {}
//! #   fn exit(&self, _: &Id) {}
//! # }
//! # impl FooSubscriber { fn new() -> Self { FooSubscriber } }
//! # let my_subscriber = FooSubscriber::new();
//! # let my_dispatch = dispatcher::Dispatch::new(my_subscriber);
//! // no default subscriber
//!
//! dispatcher::set_global_default(my_dispatch)
//!     // `set_global_default` will return an error if the global default
//!     // subscriber has already been set.
//!     .expect("global default was already set!");
//!
//! // `my_subscriber` is now the default
//! ```
//!
//! <pre class="ignore" style="white-space:normal;font:inherit;">
//! <strong>Note</strong>: The thread-local scoped dispatcher (<code>with_default</code>)
//! requires the Rust standard library. <code>no_std</code> users should
//! use <a href="fn.set_global_default.html"><code>set_global_default</code></a>
//! instead.
//! </pre>
//!
//! ## Accessing the Default Subscriber
//!
//! A thread's current default subscriber can be accessed using the
//! [`get_default`] function, which executes a closure with a reference to the
//! currently default `Dispatch`. This is used primarily by `tracing`
//! instrumentation.
//!
//! [`Subscriber`]: crate::Subscriber
#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub use tracing_core::dispatcher::set_default;
#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub use tracing_core::dispatcher::with_default;
#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub use tracing_core::dispatcher::DefaultGuard;
pub use tracing_core::dispatcher::{
    get_default, set_global_default, Dispatch, SetGlobalDefaultError, WeakDispatch,
};

/// Private API for internal use by tracing's macros.
///
/// This function is *not* considered part of `tracing`'s public API, and has no
/// stability guarantees. If you use it, and it breaks or disappears entirely,
/// don't say we didn;'t warn you.
#[doc(hidden)]
pub use tracing_core::dispatcher::has_been_set;
