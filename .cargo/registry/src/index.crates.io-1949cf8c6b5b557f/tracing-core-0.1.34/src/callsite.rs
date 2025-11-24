//! Callsites represent the source locations from which spans or events
//! originate.
//!
//! # What Are Callsites?
//!
//! Every span or event in `tracing` is associated with a [`Callsite`]. A
//! callsite is a small `static` value that is responsible for the following:
//!
//! * Storing the span or event's [`Metadata`],
//! * Uniquely [identifying](Identifier) the span or event definition,
//! * Caching the subscriber's [`Interest`][^1] in that span or event, to avoid
//!   re-evaluating filters.
//!
//! # Registering Callsites
//!
//! When a span or event is recorded for the first time, its callsite
//! [`register`]s itself with the global callsite registry. Registering a
//! callsite calls the [`Subscriber::register_callsite`][`register_callsite`]
//! method with that callsite's [`Metadata`] on every currently active
//! subscriber. This serves two primary purposes: informing subscribers of the
//! callsite's existence, and performing static filtering.
//!
//! ## Callsite Existence
//!
//! If a [`Subscriber`] implementation wishes to allocate storage for each
//! unique span/event location in the program, or pre-compute some value
//! that will be used to record that span or event in the future, it can
//! do so in its [`register_callsite`] method.
//!
//! ## Performing Static Filtering
//!
//! The [`register_callsite`] method returns an [`Interest`] value,
//! which indicates that the subscriber either [always] wishes to record
//! that span or event, [sometimes] wishes to record it based on a
//! dynamic filter evaluation, or [never] wishes to record it.
//!
//! When registering a new callsite, the [`Interest`]s returned by every
//! currently active subscriber are combined, and the result is stored at
//! each callsite. This way, when the span or event occurs in the
//! future, the cached [`Interest`] value can be checked efficiently
//! to determine if the span or event should be recorded, without
//! needing to perform expensive filtering (i.e. calling the
//! [`Subscriber::enabled`] method every time a span or event occurs).
//!
//! ### Rebuilding Cached Interest
//!
//! When a new [`Dispatch`] is created (i.e. a new subscriber becomes
//! active), any previously cached [`Interest`] values are re-evaluated
//! for all callsites in the program. This way, if the new subscriber
//! will enable a callsite that was not previously enabled, the
//! [`Interest`] in that callsite is updated. Similarly, when a
//! subscriber is dropped, the interest cache is also re-evaluated, so
//! that any callsites enabled only by that subscriber are disabled.
//!
//! In addition, the [`rebuild_interest_cache`] function in this module can be
//! used to manually invalidate all cached interest and re-register those
//! callsites. This function is useful in situations where a subscriber's
//! interest can change, but it does so relatively infrequently. The subscriber
//! may wish for its interest to be cached most of the time, and return
//! [`Interest::always`][always] or [`Interest::never`][never] in its
//! [`register_callsite`] method, so that its [`Subscriber::enabled`] method
//! doesn't need to be evaluated every time a span or event is recorded.
//! However, when the configuration changes, the subscriber can call
//! [`rebuild_interest_cache`] to re-evaluate the entire interest cache with its
//! new configuration. This is a relatively costly operation, but if the
//! configuration changes infrequently, it may be more efficient than calling
//! [`Subscriber::enabled`] frequently.
//!
//! # Implementing Callsites
//!
//! In most cases, instrumenting code using `tracing` should *not* require
//! implementing the [`Callsite`] trait directly. When using the [`tracing`
//! crate's macros][macros] or the [`#[instrument]` attribute][instrument], a
//! `Callsite` is automatically generated.
//!
//! However, code which provides alternative forms of `tracing` instrumentation
//! may need to interact with the callsite system directly. If
//! instrumentation-side code needs to produce a `Callsite` to emit spans or
//! events, the [`DefaultCallsite`] struct provided in this module is a
//! ready-made `Callsite` implementation that is suitable for most uses. When
//! possible, the use of `DefaultCallsite` should be preferred over implementing
//! [`Callsite`] for user types, as `DefaultCallsite` may benefit from
//! additional performance optimizations.
//!
//! [^1]: Returned by the [`Subscriber::register_callsite`][`register_callsite`]
//!     method.
//!
//! [`Metadata`]: crate::metadata::Metadata
//! [`Interest`]: crate::subscriber::Interest
//! [`Subscriber`]: crate::subscriber::Subscriber
//! [`register_callsite`]: crate::subscriber::Subscriber::register_callsite
//! [`Subscriber::enabled`]: crate::subscriber::Subscriber::enabled
//! [always]: crate::subscriber::Interest::always
//! [sometimes]: crate::subscriber::Interest::sometimes
//! [never]: crate::subscriber::Interest::never
//! [`Dispatch`]: crate::dispatcher::Dispatch
//! [macros]: https://docs.rs/tracing/latest/tracing/#macros
//! [instrument]: https://docs.rs/tracing/latest/tracing/attr.instrument.html
use crate::stdlib::{
    any::TypeId,
    fmt,
    hash::{Hash, Hasher},
    ptr,
    sync::{
        atomic::{AtomicBool, AtomicPtr, AtomicU8, Ordering},
        Mutex,
    },
    vec::Vec,
};
use crate::{
    dispatcher::Dispatch,
    lazy::Lazy,
    metadata::{LevelFilter, Metadata},
    subscriber::Interest,
};

use self::dispatchers::Dispatchers;

/// Trait implemented by callsites.
///
/// These functions are only intended to be called by the callsite registry, which
/// correctly handles determining the common interest between all subscribers.
///
/// See the [module-level documentation](crate::callsite) for details on
/// callsites.
pub trait Callsite: Sync {
    /// Sets the [`Interest`] for this callsite.
    ///
    /// See the [documentation on callsite interest caching][cache-docs] for
    /// details.
    ///
    /// [`Interest`]: super::subscriber::Interest
    /// [cache-docs]: crate::callsite#performing-static-filtering
    fn set_interest(&self, interest: Interest);

    /// Returns the [metadata] associated with the callsite.
    ///
    /// <div class="example-wrap" style="display:inline-block">
    /// <pre class="ignore" style="white-space:normal;font:inherit;">
    ///
    /// **Note:** Implementations of this method should not produce [`Metadata`]
    /// that share the same callsite [`Identifier`] but otherwise differ in any
    /// way (e.g., have different `name`s).
    ///
    /// </pre></div>
    ///
    /// [metadata]: super::metadata::Metadata
    fn metadata(&self) -> &Metadata<'_>;

    /// This method is an *internal implementation detail* of `tracing-core`. It
    /// is *not* intended to be called or overridden from downstream code.
    ///
    /// The `Private` type can only be constructed from within `tracing-core`.
    /// Because this method takes a `Private` as an argument, it cannot be
    /// called from (safe) code external to `tracing-core`. Because it must
    /// *return* a `Private`, the only valid implementation possible outside of
    /// `tracing-core` would have to always unconditionally panic.
    ///
    /// THIS IS BY DESIGN. There is currently no valid reason for code outside
    /// of `tracing-core` to override this method.
    // TODO(eliza): this could be used to implement a public downcasting API
    // for `&dyn Callsite`s in the future.
    #[doc(hidden)]
    #[inline]
    fn private_type_id(&self, _: private::Private<()>) -> private::Private<TypeId>
    where
        Self: 'static,
    {
        private::Private(TypeId::of::<Self>())
    }
}

/// Uniquely identifies a [`Callsite`]
///
/// Two `Identifier`s are equal if they both refer to the same callsite.
///
/// [`Callsite`]: super::callsite::Callsite
#[derive(Clone)]
pub struct Identifier(
    /// **Warning**: The fields on this type are currently `pub` because it must
    /// be able to be constructed statically by macros. However, when `const
    /// fn`s are available on stable Rust, this will no longer be necessary.
    /// Thus, these fields are *not* considered stable public API, and they may
    /// change warning. Do not rely on any fields on `Identifier`. When
    /// constructing new `Identifier`s, use the `identify_callsite!` macro
    /// instead.
    #[doc(hidden)]
    pub &'static dyn Callsite,
);

/// A default [`Callsite`] implementation.
#[derive(Debug)]
pub struct DefaultCallsite {
    interest: AtomicU8,
    registration: AtomicU8,
    meta: &'static Metadata<'static>,
    next: AtomicPtr<Self>,
}

/// Clear and reregister interest on every [`Callsite`]
///
/// This function is intended for runtime reconfiguration of filters on traces
/// when the filter recalculation is much less frequent than trace events are.
/// The alternative is to have the [`Subscriber`] that supports runtime
/// reconfiguration of filters always return [`Interest::sometimes()`] so that
/// [`enabled`] is evaluated for every event.
///
/// This function will also re-compute the global maximum level as determined by
/// the [`max_level_hint`] method. If a [`Subscriber`]
/// implementation changes the value returned by its `max_level_hint`
/// implementation at runtime, then it **must** call this function after that
/// value changes, in order for the change to be reflected.
///
/// See the [documentation on callsite interest caching][cache-docs] for
/// additional information on this function's usage.
///
/// [`max_level_hint`]: super::subscriber::Subscriber::max_level_hint
/// [`Callsite`]: super::callsite::Callsite
/// [`enabled`]: super::subscriber::Subscriber#tymethod.enabled
/// [`Interest::sometimes()`]: super::subscriber::Interest::sometimes
/// [`Subscriber`]: super::subscriber::Subscriber
/// [cache-docs]: crate::callsite#rebuilding-cached-interest
pub fn rebuild_interest_cache() {
    CALLSITES.rebuild_interest(DISPATCHERS.rebuilder());
}

/// Register a new [`Callsite`] with the global registry.
///
/// This should be called once per callsite after the callsite has been
/// constructed.
///
/// See the [documentation on callsite registration][reg-docs] for details
/// on the global callsite registry.
///
/// [`Callsite`]: crate::callsite::Callsite
/// [reg-docs]: crate::callsite#registering-callsites
pub fn register(callsite: &'static dyn Callsite) {
    // Is this a `DefaultCallsite`? If so, use the fancy linked list!
    if callsite.private_type_id(private::Private(())).0 == TypeId::of::<DefaultCallsite>() {
        let callsite = unsafe {
            // Safety: the pointer cast is safe because the type id of the
            // provided callsite matches that of the target type for the cast
            // (`DefaultCallsite`). Because user implementations of `Callsite`
            // cannot override `private_type_id`, we can trust that the callsite
            // is not lying about its type ID.
            &*(callsite as *const dyn Callsite as *const DefaultCallsite)
        };
        CALLSITES.push_default(callsite);
    } else {
        CALLSITES.push_dyn(callsite);
    }

    rebuild_callsite_interest(callsite, &DISPATCHERS.rebuilder());
}

static CALLSITES: Callsites = Callsites {
    list_head: AtomicPtr::new(ptr::null_mut()),
    has_locked_callsites: AtomicBool::new(false),
};

static DISPATCHERS: Dispatchers = Dispatchers::new();

static LOCKED_CALLSITES: Lazy<Mutex<Vec<&'static dyn Callsite>>> = Lazy::new(Default::default);

struct Callsites {
    list_head: AtomicPtr<DefaultCallsite>,
    has_locked_callsites: AtomicBool,
}

// === impl DefaultCallsite ===

impl DefaultCallsite {
    const UNREGISTERED: u8 = 0;
    const REGISTERING: u8 = 1;
    const REGISTERED: u8 = 2;

    const INTEREST_NEVER: u8 = 0;
    const INTEREST_SOMETIMES: u8 = 1;
    const INTEREST_ALWAYS: u8 = 2;

    /// Returns a new `DefaultCallsite` with the specified `Metadata`.
    pub const fn new(meta: &'static Metadata<'static>) -> Self {
        Self {
            interest: AtomicU8::new(0xFF),
            meta,
            next: AtomicPtr::new(ptr::null_mut()),
            registration: AtomicU8::new(Self::UNREGISTERED),
        }
    }

    /// Registers this callsite with the global callsite registry.
    ///
    /// If the callsite is already registered, this does nothing. When using
    /// [`DefaultCallsite`], this method should be preferred over
    /// [`tracing_core::callsite::register`], as it ensures that the callsite is
    /// only registered a single time.
    ///
    /// Other callsite implementations will generally ensure that
    /// callsites are not re-registered through another mechanism.
    ///
    /// See the [documentation on callsite registration][reg-docs] for details
    /// on the global callsite registry.
    ///
    /// [`tracing_core::callsite::register`]: crate::callsite::register
    /// [reg-docs]: crate::callsite#registering-callsites
    #[inline(never)]
    // This only happens once (or if the cached interest value was corrupted).
    #[cold]
    pub fn register(&'static self) -> Interest {
        // Attempt to advance the registration state to `REGISTERING`...
        match self.registration.compare_exchange(
            Self::UNREGISTERED,
            Self::REGISTERING,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                // Okay, we advanced the state, try to register the callsite.
                CALLSITES.push_default(self);
                rebuild_callsite_interest(self, &DISPATCHERS.rebuilder());
                self.registration.store(Self::REGISTERED, Ordering::Release);
            }
            // Great, the callsite is already registered! Just load its
            // previous cached interest.
            Err(Self::REGISTERED) => {}
            // Someone else is registering...
            Err(_state) => {
                debug_assert_eq!(
                    _state,
                    Self::REGISTERING,
                    "weird callsite registration state"
                );
                // Just hit `enabled` this time.
                return Interest::sometimes();
            }
        }

        match self.interest.load(Ordering::Relaxed) {
            Self::INTEREST_NEVER => Interest::never(),
            Self::INTEREST_ALWAYS => Interest::always(),
            _ => Interest::sometimes(),
        }
    }

    /// Returns the callsite's cached `Interest`, or registers it for the
    /// first time if it has not yet been registered.
    #[inline]
    pub fn interest(&'static self) -> Interest {
        match self.interest.load(Ordering::Relaxed) {
            Self::INTEREST_NEVER => Interest::never(),
            Self::INTEREST_SOMETIMES => Interest::sometimes(),
            Self::INTEREST_ALWAYS => Interest::always(),
            _ => self.register(),
        }
    }
}

impl Callsite for DefaultCallsite {
    fn set_interest(&self, interest: Interest) {
        let interest = match () {
            _ if interest.is_never() => Self::INTEREST_NEVER,
            _ if interest.is_always() => Self::INTEREST_ALWAYS,
            _ => Self::INTEREST_SOMETIMES,
        };
        self.interest.store(interest, Ordering::SeqCst);
    }

    #[inline(always)]
    fn metadata(&self) -> &Metadata<'static> {
        self.meta
    }
}

// ===== impl Identifier =====

impl PartialEq for Identifier {
    fn eq(&self, other: &Identifier) -> bool {
        core::ptr::eq(
            self.0 as *const _ as *const (),
            other.0 as *const _ as *const (),
        )
    }
}

impl Eq for Identifier {}

impl fmt::Debug for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Identifier({:p})", self.0)
    }
}

impl Hash for Identifier {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        (self.0 as *const dyn Callsite).hash(state)
    }
}

// === impl Callsites ===

impl Callsites {
    /// Rebuild `Interest`s for all callsites in the registry.
    ///
    /// This also re-computes the max level hint.
    fn rebuild_interest(&self, dispatchers: dispatchers::Rebuilder<'_>) {
        let mut max_level = LevelFilter::OFF;
        dispatchers.for_each(|dispatch| {
            // If the subscriber did not provide a max level hint, assume
            // that it may enable every level.
            let level_hint = dispatch.max_level_hint().unwrap_or(LevelFilter::TRACE);
            if level_hint > max_level {
                max_level = level_hint;
            }
        });

        self.for_each(|callsite| {
            rebuild_callsite_interest(callsite, &dispatchers);
        });
        LevelFilter::set_max(max_level);
    }

    /// Push a `dyn Callsite` trait object to the callsite registry.
    ///
    /// This will attempt to lock the callsites vector.
    fn push_dyn(&self, callsite: &'static dyn Callsite) {
        let mut lock = LOCKED_CALLSITES.lock().unwrap();
        self.has_locked_callsites.store(true, Ordering::Release);
        lock.push(callsite);
    }

    /// Push a `DefaultCallsite` to the callsite registry.
    ///
    /// If we know the callsite being pushed is a `DefaultCallsite`, we can push
    /// it to the linked list without having to acquire a lock.
    fn push_default(&self, callsite: &'static DefaultCallsite) {
        let mut head = self.list_head.load(Ordering::Acquire);

        loop {
            callsite.next.store(head, Ordering::Release);

            assert_ne!(
                callsite as *const _, head,
                "Attempted to register a `DefaultCallsite` that already exists! \
                This will cause an infinite loop when attempting to read from the \
                callsite cache. This is likely a bug! You should only need to call \
                `DefaultCallsite::register` once per `DefaultCallsite`."
            );

            match self.list_head.compare_exchange(
                head,
                callsite as *const _ as *mut _,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    break;
                }
                Err(current) => head = current,
            }
        }
    }

    /// Invokes the provided closure `f` with each callsite in the registry.
    fn for_each(&self, mut f: impl FnMut(&'static dyn Callsite)) {
        let mut head = self.list_head.load(Ordering::Acquire);

        while let Some(cs) = unsafe { head.as_ref() } {
            f(cs);

            head = cs.next.load(Ordering::Acquire);
        }

        if self.has_locked_callsites.load(Ordering::Acquire) {
            let locked = LOCKED_CALLSITES.lock().unwrap();
            for &cs in locked.iter() {
                f(cs);
            }
        }
    }
}

pub(crate) fn register_dispatch(dispatch: &Dispatch) {
    let dispatchers = DISPATCHERS.register_dispatch(dispatch);
    dispatch.subscriber().on_register_dispatch(dispatch);
    CALLSITES.rebuild_interest(dispatchers);
}

fn rebuild_callsite_interest(
    callsite: &'static dyn Callsite,
    dispatchers: &dispatchers::Rebuilder<'_>,
) {
    let meta = callsite.metadata();

    let mut interest = None;
    dispatchers.for_each(|dispatch| {
        let this_interest = dispatch.register_callsite(meta);
        interest = match interest.take() {
            None => Some(this_interest),
            Some(that_interest) => Some(that_interest.and(this_interest)),
        }
    });

    let interest = interest.unwrap_or_else(Interest::never);
    callsite.set_interest(interest)
}

mod private {
    /// Don't call this function, it's private.
    #[allow(missing_debug_implementations)]
    pub struct Private<T>(pub(crate) T);
}

#[cfg(feature = "std")]
mod dispatchers {
    use crate::{dispatcher, lazy::Lazy};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        RwLock, RwLockReadGuard, RwLockWriteGuard,
    };

    pub(super) struct Dispatchers {
        has_just_one: AtomicBool,
    }

    static LOCKED_DISPATCHERS: Lazy<RwLock<Vec<dispatcher::Registrar>>> =
        Lazy::new(Default::default);

    pub(super) enum Rebuilder<'a> {
        JustOne,
        Read(RwLockReadGuard<'a, Vec<dispatcher::Registrar>>),
        Write(RwLockWriteGuard<'a, Vec<dispatcher::Registrar>>),
    }

    impl Dispatchers {
        pub(super) const fn new() -> Self {
            Self {
                has_just_one: AtomicBool::new(true),
            }
        }

        pub(super) fn rebuilder(&self) -> Rebuilder<'_> {
            if self.has_just_one.load(Ordering::SeqCst) {
                return Rebuilder::JustOne;
            }
            Rebuilder::Read(LOCKED_DISPATCHERS.read().unwrap())
        }

        pub(super) fn register_dispatch(&self, dispatch: &dispatcher::Dispatch) -> Rebuilder<'_> {
            let mut dispatchers = LOCKED_DISPATCHERS.write().unwrap();
            dispatchers.retain(|d| d.upgrade().is_some());
            dispatchers.push(dispatch.registrar());
            self.has_just_one
                .store(dispatchers.len() <= 1, Ordering::SeqCst);
            Rebuilder::Write(dispatchers)
        }
    }

    impl Rebuilder<'_> {
        pub(super) fn for_each(&self, mut f: impl FnMut(&dispatcher::Dispatch)) {
            let iter = match self {
                Rebuilder::JustOne => {
                    dispatcher::get_default(f);
                    return;
                }
                Rebuilder::Read(vec) => vec.iter(),
                Rebuilder::Write(vec) => vec.iter(),
            };
            iter.filter_map(dispatcher::Registrar::upgrade)
                .for_each(|dispatch| f(&dispatch))
        }
    }
}

#[cfg(not(feature = "std"))]
mod dispatchers {
    use crate::dispatcher;

    pub(super) struct Dispatchers(());
    pub(super) struct Rebuilder<'a>(Option<&'a dispatcher::Dispatch>);

    impl Dispatchers {
        pub(super) const fn new() -> Self {
            Self(())
        }

        pub(super) fn rebuilder(&self) -> Rebuilder<'_> {
            Rebuilder(None)
        }

        pub(super) fn register_dispatch<'dispatch>(
            &self,
            dispatch: &'dispatch dispatcher::Dispatch,
        ) -> Rebuilder<'dispatch> {
            // nop; on no_std, there can only ever be one dispatcher
            Rebuilder(Some(dispatch))
        }
    }

    impl Rebuilder<'_> {
        #[inline]
        pub(super) fn for_each(&self, mut f: impl FnMut(&dispatcher::Dispatch)) {
            if let Some(dispatch) = self.0 {
                // we are rebuilding the interest cache because a new dispatcher
                // is about to be set. on `no_std`, this should only happen
                // once, because the new dispatcher will be the global default.
                f(dispatch)
            } else {
                // otherwise, we are rebuilding the cache because the subscriber
                // configuration changed, so use the global default.
                // on no_std, there can only ever be one dispatcher
                dispatcher::get_default(f)
            }
        }
    }
}
