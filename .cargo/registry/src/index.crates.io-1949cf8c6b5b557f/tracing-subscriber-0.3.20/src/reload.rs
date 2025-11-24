//! Wrapper for a `Layer` to allow it to be dynamically reloaded.
//!
//! This module provides a [`Layer` type] implementing the [`Layer` trait] or [`Filter` trait]
//! which wraps another type implementing the corresponding trait. This
//! allows the wrapped type to be replaced with another
//! instance of that type at runtime.
//!
//! This can be used in cases where a subset of `Layer` or `Filter` functionality
//! should be dynamically reconfigured, such as when filtering directives may
//! change at runtime. Note that this layer introduces a (relatively small)
//! amount of overhead, and should thus only be used as needed.
//!
//! # Examples
//!
//! Reloading a [global filtering](crate::layer#global-filtering) layer:
//!
//! ```rust
//! # use tracing::info;
//! use tracing_subscriber::{filter, fmt, reload, prelude::*};
//! let filter = filter::LevelFilter::WARN;
//! let (filter, reload_handle) = reload::Layer::new(filter);
//! tracing_subscriber::registry()
//!   .with(filter)
//!   .with(fmt::Layer::default())
//!   .init();
//! #
//! # // specifying the Registry type is required
//! # let _: &reload::Handle<filter::LevelFilter, tracing_subscriber::Registry> = &reload_handle;
//! #
//! info!("This will be ignored");
//! reload_handle.modify(|filter| *filter = filter::LevelFilter::INFO);
//! info!("This will be logged");
//! ```
//!
//! Reloading a [`Filtered`](crate::filter::Filtered) layer:
//!
//! ```rust
//! # use tracing::info;
//! use tracing_subscriber::{filter, fmt, reload, prelude::*};
//! let filtered_layer = fmt::Layer::default().with_filter(filter::LevelFilter::WARN);
//! let (filtered_layer, reload_handle) = reload::Layer::new(filtered_layer);
//! #
//! # // specifying the Registry type is required
//! # let _: &reload::Handle<filter::Filtered<fmt::Layer<tracing_subscriber::Registry>,
//! # filter::LevelFilter, tracing_subscriber::Registry>,tracing_subscriber::Registry>
//! # = &reload_handle;
//! #
//! tracing_subscriber::registry()
//!   .with(filtered_layer)
//!   .init();
//! info!("This will be ignored");
//! reload_handle.modify(|layer| *layer.filter_mut() = filter::LevelFilter::INFO);
//! info!("This will be logged");
//! ```
//!
//! ## Note
//!
//! The [`Layer`] implementation is unable to implement downcasting functionality,
//! so certain [`Layer`] will fail to downcast if wrapped in a `reload::Layer`.
//!
//! If you only want to be able to dynamically change the
//! `Filter` on a layer, prefer wrapping that `Filter` in the `reload::Layer`.
//!
//! [`Filter` trait]: crate::layer::Filter
//! [`Layer` type]: Layer
//! [`Layer` trait]: super::layer::Layer
use crate::layer;
use crate::sync::RwLock;

use core::any::TypeId;
use std::{
    error, fmt,
    marker::PhantomData,
    sync::{Arc, Weak},
};
use tracing_core::{
    callsite, span,
    subscriber::{Interest, Subscriber},
    Dispatch, Event, LevelFilter, Metadata,
};

/// Wraps a `Layer` or `Filter`, allowing it to be reloaded dynamically at runtime.
#[derive(Debug)]
pub struct Layer<L, S> {
    // TODO(eliza): this once used a `crossbeam_util::ShardedRwLock`. We may
    // eventually wish to replace it with a sharded lock implementation on top
    // of our internal `RwLock` wrapper type. If possible, we should profile
    // this first to determine if it's necessary.
    inner: Arc<RwLock<L>>,
    _s: PhantomData<fn(S)>,
}

/// Allows reloading the state of an associated [`Layer`](crate::layer::Layer).
#[derive(Debug)]
pub struct Handle<L, S> {
    inner: Weak<RwLock<L>>,
    _s: PhantomData<fn(S)>,
}

/// Indicates that an error occurred when reloading a layer.
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
}

#[derive(Debug)]
enum ErrorKind {
    SubscriberGone,
    Poisoned,
}

// ===== impl Layer =====

impl<L, S> crate::Layer<S> for Layer<L, S>
where
    L: crate::Layer<S> + 'static,
    S: Subscriber,
{
    fn on_register_dispatch(&self, subscriber: &Dispatch) {
        try_lock!(self.inner.read()).on_register_dispatch(subscriber);
    }

    fn on_layer(&mut self, subscriber: &mut S) {
        try_lock!(self.inner.write(), else return).on_layer(subscriber);
    }

    #[inline]
    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        try_lock!(self.inner.read(), else return Interest::sometimes()).register_callsite(metadata)
    }

    #[inline]
    fn enabled(&self, metadata: &Metadata<'_>, ctx: layer::Context<'_, S>) -> bool {
        try_lock!(self.inner.read(), else return false).enabled(metadata, ctx)
    }

    #[inline]
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: layer::Context<'_, S>) {
        try_lock!(self.inner.read()).on_new_span(attrs, id, ctx)
    }

    #[inline]
    fn on_record(&self, span: &span::Id, values: &span::Record<'_>, ctx: layer::Context<'_, S>) {
        try_lock!(self.inner.read()).on_record(span, values, ctx)
    }

    #[inline]
    fn on_follows_from(&self, span: &span::Id, follows: &span::Id, ctx: layer::Context<'_, S>) {
        try_lock!(self.inner.read()).on_follows_from(span, follows, ctx)
    }

    #[inline]
    fn event_enabled(&self, event: &Event<'_>, ctx: layer::Context<'_, S>) -> bool {
        try_lock!(self.inner.read(), else return false).event_enabled(event, ctx)
    }

    #[inline]
    fn on_event(&self, event: &Event<'_>, ctx: layer::Context<'_, S>) {
        try_lock!(self.inner.read()).on_event(event, ctx)
    }

    #[inline]
    fn on_enter(&self, id: &span::Id, ctx: layer::Context<'_, S>) {
        try_lock!(self.inner.read()).on_enter(id, ctx)
    }

    #[inline]
    fn on_exit(&self, id: &span::Id, ctx: layer::Context<'_, S>) {
        try_lock!(self.inner.read()).on_exit(id, ctx)
    }

    #[inline]
    fn on_close(&self, id: span::Id, ctx: layer::Context<'_, S>) {
        try_lock!(self.inner.read()).on_close(id, ctx)
    }

    #[inline]
    fn on_id_change(&self, old: &span::Id, new: &span::Id, ctx: layer::Context<'_, S>) {
        try_lock!(self.inner.read()).on_id_change(old, new, ctx)
    }

    #[inline]
    fn max_level_hint(&self) -> Option<LevelFilter> {
        try_lock!(self.inner.read(), else return None).max_level_hint()
    }

    #[doc(hidden)]
    unsafe fn downcast_raw(&self, id: TypeId) -> Option<*const ()> {
        // Safety: it is generally unsafe to downcast through a reload, because
        // the pointer can be invalidated after the lock is dropped.
        // `NoneLayerMarker` is a special case because it
        // is never dereferenced.
        //
        // Additionally, even if the marker type *is* dereferenced (which it
        // never will be), the pointer should be valid even if the subscriber
        // is reloaded, because all `NoneLayerMarker` pointers that we return
        // actually point to the global static singleton `NoneLayerMarker`,
        // rather than to a field inside the lock.
        if id == TypeId::of::<layer::NoneLayerMarker>() {
            return try_lock!(self.inner.read(), else return None).downcast_raw(id);
        }

        None
    }
}

// ===== impl Filter =====

#[cfg(all(feature = "registry", feature = "std"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "registry", feature = "std"))))]
impl<S, L> crate::layer::Filter<S> for Layer<L, S>
where
    L: crate::layer::Filter<S> + 'static,
    S: Subscriber,
{
    #[inline]
    fn callsite_enabled(&self, metadata: &'static Metadata<'static>) -> Interest {
        try_lock!(self.inner.read(), else return Interest::sometimes()).callsite_enabled(metadata)
    }

    #[inline]
    fn enabled(&self, metadata: &Metadata<'_>, ctx: &layer::Context<'_, S>) -> bool {
        try_lock!(self.inner.read(), else return false).enabled(metadata, ctx)
    }

    #[inline]
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: layer::Context<'_, S>) {
        try_lock!(self.inner.read()).on_new_span(attrs, id, ctx)
    }

    #[inline]
    fn on_record(&self, span: &span::Id, values: &span::Record<'_>, ctx: layer::Context<'_, S>) {
        try_lock!(self.inner.read()).on_record(span, values, ctx)
    }

    #[inline]
    fn on_enter(&self, id: &span::Id, ctx: layer::Context<'_, S>) {
        try_lock!(self.inner.read()).on_enter(id, ctx)
    }

    #[inline]
    fn on_exit(&self, id: &span::Id, ctx: layer::Context<'_, S>) {
        try_lock!(self.inner.read()).on_exit(id, ctx)
    }

    #[inline]
    fn on_close(&self, id: span::Id, ctx: layer::Context<'_, S>) {
        try_lock!(self.inner.read()).on_close(id, ctx)
    }

    #[inline]
    fn max_level_hint(&self) -> Option<LevelFilter> {
        try_lock!(self.inner.read(), else return None).max_level_hint()
    }
}

impl<L, S> Layer<L, S> {
    /// Wraps the given [`Layer`] or [`Filter`], returning a `reload::Layer`
    /// and a `Handle` that allows the inner value to be modified at runtime.
    ///
    /// [`Layer`]: crate::layer::Layer
    /// [`Filter`]: crate::layer::Filter
    pub fn new(inner: L) -> (Self, Handle<L, S>) {
        let this = Self {
            inner: Arc::new(RwLock::new(inner)),
            _s: PhantomData,
        };
        let handle = this.handle();
        (this, handle)
    }

    /// Returns a `Handle` that can be used to reload the wrapped [`Layer`] or [`Filter`].
    ///
    /// [`Layer`]: crate::layer::Layer
    /// [`Filter`]: crate::layer::Filter
    pub fn handle(&self) -> Handle<L, S> {
        Handle {
            inner: Arc::downgrade(&self.inner),
            _s: PhantomData,
        }
    }
}

// ===== impl Handle =====

impl<L, S> Handle<L, S> {
    /// Replace the current [`Layer`] or [`Filter`] with the provided `new_value`.
    ///
    /// [`Handle::reload`] cannot be used with the [`Filtered`] layer; use
    /// [`Handle::modify`] instead (see [this issue] for additional details).
    ///
    /// However, if the _only_ the [`Filter`]  needs to be modified, use
    /// `reload::Layer` to wrap the `Filter` directly.
    ///
    /// [`Layer`]: crate::layer::Layer
    /// [`Filter`]: crate::layer::Filter
    /// [`Filtered`]: crate::filter::Filtered
    ///
    /// [this issue]: https://github.com/tokio-rs/tracing/issues/1629
    pub fn reload(&self, new_value: impl Into<L>) -> Result<(), Error> {
        self.modify(|layer| {
            *layer = new_value.into();
        })
    }

    /// Invokes a closure with a mutable reference to the current layer or filter,
    /// allowing it to be modified in place.
    pub fn modify(&self, f: impl FnOnce(&mut L)) -> Result<(), Error> {
        let inner = self.inner.upgrade().ok_or(Error {
            kind: ErrorKind::SubscriberGone,
        })?;

        let mut lock = try_lock!(inner.write(), else return Err(Error::poisoned()));
        f(&mut *lock);
        // Release the lock before rebuilding the interest cache, as that
        // function will lock the new layer.
        drop(lock);

        callsite::rebuild_interest_cache();

        // If the `log` crate compatibility feature is in use, set `log`'s max
        // level as well, in case the max `tracing` level changed. We do this
        // *after* rebuilding the interest cache, as that's when the `tracing`
        // max level filter is re-computed.
        #[cfg(feature = "tracing-log")]
        tracing_log::log::set_max_level(tracing_log::AsLog::as_log(
            &crate::filter::LevelFilter::current(),
        ));

        Ok(())
    }

    /// Returns a clone of the layer or filter's current value if it still exists.
    /// Otherwise, if the subscriber has been dropped, returns `None`.
    pub fn clone_current(&self) -> Option<L>
    where
        L: Clone,
    {
        self.with_current(L::clone).ok()
    }

    /// Invokes a closure with a borrowed reference to the current layer or filter,
    /// returning the result (or an error if the subscriber no longer exists).
    pub fn with_current<T>(&self, f: impl FnOnce(&L) -> T) -> Result<T, Error> {
        let inner = self.inner.upgrade().ok_or(Error {
            kind: ErrorKind::SubscriberGone,
        })?;
        let inner = try_lock!(inner.read(), else return Err(Error::poisoned()));
        Ok(f(&*inner))
    }
}

impl<L, S> Clone for Handle<L, S> {
    fn clone(&self) -> Self {
        Handle {
            inner: self.inner.clone(),
            _s: PhantomData,
        }
    }
}

// ===== impl Error =====

impl Error {
    fn poisoned() -> Self {
        Self {
            kind: ErrorKind::Poisoned,
        }
    }

    /// Returns `true` if this error occurred because the layer was poisoned by
    /// a panic on another thread.
    pub fn is_poisoned(&self) -> bool {
        matches!(self.kind, ErrorKind::Poisoned)
    }

    /// Returns `true` if this error occurred because the `Subscriber`
    /// containing the reloadable layer was dropped.
    pub fn is_dropped(&self) -> bool {
        matches!(self.kind, ErrorKind::SubscriberGone)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self.kind {
            ErrorKind::SubscriberGone => "subscriber no longer exists",
            ErrorKind::Poisoned => "lock poisoned",
        };
        f.pad(msg)
    }
}

impl error::Error for Error {}
