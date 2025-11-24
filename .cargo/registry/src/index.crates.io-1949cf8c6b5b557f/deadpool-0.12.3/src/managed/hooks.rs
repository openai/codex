//! Hooks allowing to run code when creating and/or recycling objects.

use std::{borrow::Cow, fmt, future::Future, pin::Pin};

use crate::managed::object::ObjectInner;

use super::{Manager, Metrics};

/// The result returned by hooks
pub type HookResult<E> = Result<(), HookError<E>>;

/// The boxed future that should be returned by async hooks
pub type HookFuture<'a, E> = Pin<Box<dyn Future<Output = HookResult<E>> + Send + 'a>>;

/// Function signature for sync callbacks
type SyncFn<M> =
    dyn Fn(&mut <M as Manager>::Type, &Metrics) -> HookResult<<M as Manager>::Error> + Sync + Send;

/// Function siganture for async callbacks
type AsyncFn<M> = dyn for<'a> Fn(&'a mut <M as Manager>::Type, &'a Metrics) -> HookFuture<'a, <M as Manager>::Error>
    + Sync
    + Send;

/// Wrapper for hook functions
pub enum Hook<M: Manager> {
    /// Use a plain function (non-async) as a hook
    Fn(Box<SyncFn<M>>),
    /// Use an async function as a hook
    AsyncFn(Box<AsyncFn<M>>),
}

impl<M: Manager> Hook<M> {
    /// Create Hook from sync function
    pub fn sync_fn(
        f: impl Fn(&mut M::Type, &Metrics) -> HookResult<M::Error> + Sync + Send + 'static,
    ) -> Self {
        Self::Fn(Box::new(f))
    }
    /// Create Hook from async function
    pub fn async_fn(
        f: impl for<'a> Fn(&'a mut M::Type, &'a Metrics) -> HookFuture<'a, M::Error>
            + Sync
            + Send
            + 'static,
    ) -> Self {
        Self::AsyncFn(Box::new(f))
    }
}

impl<M: Manager> fmt::Debug for Hook<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fn(_) => f
                .debug_tuple("Fn")
                //.field(arg0)
                .finish(),
            Self::AsyncFn(_) => f
                .debug_tuple("AsyncFn")
                //.field(arg0)
                .finish(),
        }
    }
}

/// Error which is returned by `pre_create`, `pre_recycle` and
/// `post_recycle` hooks.
#[derive(Debug)]
pub enum HookError<E> {
    /// Hook failed for some other reason.
    Message(Cow<'static, str>),

    /// Error caused by the backend.
    Backend(E),
}

impl<E> HookError<E> {
    /// Convenience constructor function for the `HookError::Message`
    /// variant.
    pub fn message(msg: impl Into<Cow<'static, str>>) -> Self {
        Self::Message(msg.into())
    }
}

impl<E: fmt::Display> fmt::Display for HookError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(msg) => write!(f, "{}", msg),
            Self::Backend(e) => write!(f, "{}", e),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for HookError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Message(_) => None,
            Self::Backend(e) => Some(e),
        }
    }
}

pub(crate) struct HookVec<M: Manager> {
    vec: Vec<Hook<M>>,
}

// Implemented manually to avoid unnecessary trait bound on `M` type parameter.
impl<M: Manager> fmt::Debug for HookVec<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HookVec")
            //.field("fns", &self.fns)
            .finish_non_exhaustive()
    }
}

// Implemented manually to avoid unnecessary trait bound on `M` type parameter.
impl<M: Manager> Default for HookVec<M> {
    fn default() -> Self {
        Self { vec: Vec::new() }
    }
}

impl<M: Manager> HookVec<M> {
    pub(crate) async fn apply(
        &self,
        inner: &mut ObjectInner<M>,
    ) -> Result<(), HookError<M::Error>> {
        for hook in &self.vec {
            match hook {
                Hook::Fn(f) => f(&mut inner.obj, &inner.metrics)?,
                Hook::AsyncFn(f) => f(&mut inner.obj, &inner.metrics).await?,
            };
        }
        Ok(())
    }
    pub(crate) fn push(&mut self, hook: Hook<M>) {
        self.vec.push(hook);
    }
}

/// Collection of all the hooks that can be configured for a [`Pool`].
///
/// [`Pool`]: super::Pool
pub(crate) struct Hooks<M: Manager> {
    pub(crate) post_create: HookVec<M>,
    pub(crate) pre_recycle: HookVec<M>,
    pub(crate) post_recycle: HookVec<M>,
}

// Implemented manually to avoid unnecessary trait bound on `M` type parameter.
impl<M: Manager> fmt::Debug for Hooks<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Hooks")
            .field("post_create", &self.post_create)
            .field("pre_recycle", &self.post_recycle)
            .field("post_recycle", &self.post_recycle)
            .finish()
    }
}

// Implemented manually to avoid unnecessary trait bound on `M` type parameter.
impl<M: Manager> Default for Hooks<M> {
    fn default() -> Self {
        Self {
            pre_recycle: HookVec::default(),
            post_create: HookVec::default(),
            post_recycle: HookVec::default(),
        }
    }
}
