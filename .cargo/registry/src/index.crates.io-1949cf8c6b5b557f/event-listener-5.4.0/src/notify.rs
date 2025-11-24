//! The `Notification` trait for specifying notification.

use crate::sync::atomic::{self, Ordering};
#[cfg(feature = "std")]
use core::fmt;

pub(crate) use __private::Internal;

/// The type of notification to use with an [`Event`].
///
/// This is hidden and sealed to prevent changes to this trait from being breaking.
///
/// [`Event`]: crate::Event
#[doc(hidden)]
pub trait NotificationPrivate {
    /// The tag data associated with a notification.
    type Tag;

    /// Emit a fence to ensure that the notification is visible to the listeners.
    fn fence(&self, internal: Internal);

    /// Whether or not the number of currently waiting listeners should be subtracted from `count()`.
    fn is_additional(&self, internal: Internal) -> bool;

    /// Get the number of listeners to wake.
    fn count(&self, internal: Internal) -> usize;

    /// Get a tag to be associated with a notification.
    ///
    /// This method is expected to be called `count()` times.
    fn next_tag(&mut self, internal: Internal) -> Self::Tag;
}

/// A notification that can be used to notify an [`Event`].
///
/// This type is used by the [`Event::notify()`] function to determine how many listeners to wake up, whether
/// or not to subtract additional listeners, and other properties. The actual internal data is hidden in a
/// private trait and is intentionally not exposed. This means that users cannot manually implement the
/// [`Notification`] trait. However, it also means that changing the underlying trait is not a semver breaking
/// change.
///
/// Users can create types that implement notifications using the combinators on the [`IntoNotification`] type.
/// Typical construction of a [`Notification`] starts with a numeric literal (like `3usize`) and then optionally
/// adding combinators.
///
/// # Example
///
/// ```
/// use event_listener::{Event, IntoNotification, Notification};
///
/// fn notify(ev: &Event, notify: impl Notification<Tag = ()>) {
///     ev.notify(notify);
/// }
///
/// notify(&Event::new(), 1.additional());
/// ```
///
/// [`Event`]: crate::Event
pub trait Notification: NotificationPrivate {}
impl<N: NotificationPrivate + ?Sized> Notification for N {}

/// Notify a given number of unnotifed listeners.
#[derive(Debug, Clone)]
#[doc(hidden)]
pub struct Notify(usize);

impl Notify {
    /// Create a new `Notify` with the given number of listeners to notify.
    fn new(count: usize) -> Self {
        Self(count)
    }
}

impl NotificationPrivate for Notify {
    type Tag = ();

    fn is_additional(&self, _: Internal) -> bool {
        false
    }

    fn fence(&self, _: Internal) {
        full_fence();
    }

    fn count(&self, _: Internal) -> usize {
        self.0
    }

    fn next_tag(&mut self, _: Internal) -> Self::Tag {}
}

/// Make the underlying notification additional.
#[derive(Debug, Clone)]
#[doc(hidden)]
pub struct Additional<N: ?Sized>(N);

impl<N> Additional<N> {
    /// Create a new `Additional` with the given notification.
    fn new(inner: N) -> Self {
        Self(inner)
    }
}

impl<N> NotificationPrivate for Additional<N>
where
    N: Notification + ?Sized,
{
    type Tag = N::Tag;

    fn is_additional(&self, _: Internal) -> bool {
        true
    }

    fn fence(&self, i: Internal) {
        self.0.fence(i);
    }

    fn count(&self, i: Internal) -> usize {
        self.0.count(i)
    }

    fn next_tag(&mut self, i: Internal) -> Self::Tag {
        self.0.next_tag(i)
    }
}

/// Don't emit a fence for this notification.
#[derive(Debug, Clone)]
#[doc(hidden)]
pub struct Relaxed<N: ?Sized>(N);

impl<N> Relaxed<N> {
    /// Create a new `Relaxed` with the given notification.
    fn new(inner: N) -> Self {
        Self(inner)
    }
}

impl<N> NotificationPrivate for Relaxed<N>
where
    N: Notification + ?Sized,
{
    type Tag = N::Tag;

    fn is_additional(&self, i: Internal) -> bool {
        self.0.is_additional(i)
    }

    fn fence(&self, _: Internal) {
        // Don't emit a fence.
    }

    fn count(&self, i: Internal) -> usize {
        self.0.count(i)
    }

    fn next_tag(&mut self, i: Internal) -> Self::Tag {
        self.0.next_tag(i)
    }
}

/// Use a tag to notify listeners.
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
#[doc(hidden)]
pub struct Tag<N: ?Sized, T> {
    tag: T,
    inner: N,
}

#[cfg(feature = "std")]
impl<N: ?Sized, T> Tag<N, T> {
    /// Create a new `Tag` with the given tag and notification.
    fn new(tag: T, inner: N) -> Self
    where
        N: Sized,
    {
        Self { tag, inner }
    }
}

#[cfg(feature = "std")]
impl<N, T> NotificationPrivate for Tag<N, T>
where
    N: Notification + ?Sized,
    T: Clone,
{
    type Tag = T;

    fn is_additional(&self, i: Internal) -> bool {
        self.inner.is_additional(i)
    }

    fn fence(&self, i: Internal) {
        self.inner.fence(i);
    }

    fn count(&self, i: Internal) -> usize {
        self.inner.count(i)
    }

    fn next_tag(&mut self, _: Internal) -> Self::Tag {
        self.tag.clone()
    }
}

/// Use a function to generate a tag to notify listeners.
#[cfg(feature = "std")]
#[doc(hidden)]
pub struct TagWith<N: ?Sized, F> {
    tag: F,
    inner: N,
}

#[cfg(feature = "std")]
impl<N: fmt::Debug, F> fmt::Debug for TagWith<N, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Ellipses;

        impl fmt::Debug for Ellipses {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("..")
            }
        }

        f.debug_struct("TagWith")
            .field("tag", &Ellipses)
            .field("inner", &self.inner)
            .finish()
    }
}

#[cfg(feature = "std")]
impl<N, F> TagWith<N, F> {
    /// Create a new `TagFn` with the given tag function and notification.
    fn new(tag: F, inner: N) -> Self {
        Self { tag, inner }
    }
}

#[cfg(feature = "std")]
impl<N, F, T> NotificationPrivate for TagWith<N, F>
where
    N: Notification + ?Sized,
    F: FnMut() -> T,
{
    type Tag = T;

    fn is_additional(&self, i: Internal) -> bool {
        self.inner.is_additional(i)
    }

    fn fence(&self, i: Internal) {
        self.inner.fence(i);
    }

    fn count(&self, i: Internal) -> usize {
        self.inner.count(i)
    }

    fn next_tag(&mut self, _: Internal) -> Self::Tag {
        (self.tag)()
    }
}

/// A generic notification.
#[derive(Debug)]
pub(crate) struct GenericNotify<F> {
    /// Number of listeners to notify.
    count: usize,

    /// Whether this notification is additional.
    additional: bool,

    /// Generate tags.
    tags: F,
}

impl<T, F: TagProducer<Tag = T>> GenericNotify<F> {
    pub(crate) fn new(count: usize, additional: bool, tags: F) -> Self {
        Self {
            count,
            additional,
            tags,
        }
    }
}

impl<T, F: TagProducer<Tag = T>> NotificationPrivate for GenericNotify<F> {
    type Tag = T;

    fn is_additional(&self, _: Internal) -> bool {
        self.additional
    }

    fn fence(&self, _: Internal) {
        // Don't emit a fence.
    }

    fn count(&self, _: Internal) -> usize {
        self.count
    }

    fn next_tag(&mut self, _: Internal) -> Self::Tag {
        self.tags.next_tag()
    }
}

/// The producer for a generic notification.
pub(crate) trait TagProducer {
    type Tag;

    /// Get the next tag.
    fn next_tag(&mut self) -> Self::Tag;
}

impl<T, F: FnMut() -> T> TagProducer for F {
    type Tag = T;

    fn next_tag(&mut self) -> T {
        (self)()
    }
}

/// A value that can be converted into a [`Notification`].
///
/// This trait adds onto the [`Notification`] trait by providing combinators that can be applied to all
/// notification types as well as numeric literals. This transforms what would normally be:
///
/// ```
/// use event_listener::Event;
///
/// let event = Event::new();
///
/// // Note that each use case needs its own function, leading to bloat.
/// event.notify(1);
/// event.notify_additional(3);
/// event.notify_relaxed(5);
/// event.notify_additional_relaxed(2);
/// ```
///
/// into this:
///
/// ```
/// use event_listener::{Event, IntoNotification, Listener};
///
/// let event = Event::new();
///
/// event.notify(1);
/// event.notify(3.additional());
/// event.notify(5.relaxed());
/// event.notify(2.additional().relaxed());
/// ```
///
/// This trait is implemented for all types that implement [`Notification`], as well as for non-floating-point
/// numeric literals (`usize`, `i32`, etc).
///
/// This function can be thought of as being analogous to [`std::iter::IntoIterator`], but for [`Notification`].
pub trait IntoNotification: __private::Sealed {
    /// The tag data associated with a notification.
    ///
    /// By default, most [`Event`]s will use the unit type, `()`. However, this can be used to pass data along to
    /// the listener.
    type Tag;

    /// The notification type.
    ///
    /// Tells what kind of underlying type that the [`Notification`] is. You probably don't need to worry about
    /// this.
    type Notify: Notification<Tag = Self::Tag>;

    /// Convert this value into a notification.
    ///
    /// This allows the user to convert an [`IntoNotification`] into a [`Notification`].
    ///
    /// # Panics
    ///
    /// This function panics if the value represents a negative number of notifications.
    ///
    /// # Examples
    ///
    /// ```
    /// use event_listener::IntoNotification;
    ///
    /// let _ = 3.into_notification();
    /// ```
    fn into_notification(self) -> Self::Notify;

    /// Convert this value into an additional notification.
    ///
    /// By default, notifications ignore listeners that are already notified. Generally, this happens when there
    /// is an [`EventListener`] that has been woken up, but hasn't been polled to completion or waited on yet.
    /// For instance, if you have three notified listeners and you call `event.notify(5)`, only two listeners
    /// will be woken up.
    ///
    /// This default behavior is generally desired. For instance, if you are writing a `Mutex` implementation
    /// powered by an [`Event`], you usually only want one consumer to be notified at a time. If you notified
    /// a listener when another listener is already notified, you would have unnecessary contention for your
    /// lock, as both listeners fight over the lock. Therefore, you would call `event.notify(1)` to make sure
    /// *at least* one listener is awake.
    ///
    /// Sometimes, this behavior is not desired. For instance, if you are writing an MPMC channel, it is desirable
    /// for multiple listeners to be reading from the underlying queue at once. In this case, you would instead
    /// call `event.notify(1.additional())`.
    ///
    /// # Examples
    ///
    /// ```
    /// use event_listener::{Event, IntoNotification, Listener};
    ///
    /// let event = Event::new();
    ///
    /// let mut l1 = event.listen();
    /// let mut l2 = event.listen();
    ///
    /// // This will only wake up the first listener, as the second call observes that there is already a
    /// // notified listener.
    /// event.notify(1);
    /// event.notify(1);
    ///
    /// // This call wakes up the other listener.
    /// event.notify(1.additional());
    /// ```
    fn additional(self) -> Additional<Self::Notify>
    where
        Self: Sized,
    {
        Additional::new(self.into_notification())
    }

    /// Don't emit a fence for this notification.
    ///
    /// Usually, notifications emit a `SeqCst` atomic fence before any listeners are woken up. This ensures
    /// that notification state isn't inconsistent before any wakers are woken up. However, it may be
    /// desirable to omit this fence in certain cases.
    ///
    /// - You are running the [`Event`] on a single thread, where no synchronization needs to occur.
    /// - You are emitting the `SeqCst` fence yourself.
    ///
    /// In these cases, `relaxed()` can be used to avoid emitting the `SeqCst` fence.
    ///
    /// # Examples
    ///
    /// ```
    /// use event_listener::{Event, IntoNotification, Listener};
    /// use std::sync::atomic::{self, Ordering};
    ///
    /// let event = Event::new();
    ///
    /// let listener1 = event.listen();
    /// let listener2 = event.listen();
    /// let listener3 = event.listen();
    ///
    /// // We should emit a fence manually when using relaxed notifications.
    /// atomic::fence(Ordering::SeqCst);
    ///
    /// // Notifies two listeners.
    /// //
    /// // Listener queueing is fair, which means `listener1` and `listener2`
    /// // get notified here since they start listening before `listener3`.
    /// event.notify(1.relaxed());
    /// event.notify(1.relaxed());
    /// ```
    fn relaxed(self) -> Relaxed<Self::Notify>
    where
        Self: Sized,
    {
        Relaxed::new(self.into_notification())
    }

    /// Use a tag with this notification.
    ///
    /// In many cases, it is desired to send additional information to the listener of the [`Event`]. For instance,
    /// it is possible to optimize a `Mutex` implementation by locking directly on the next listener, without
    /// needing to ever unlock the mutex at all.
    ///
    /// The tag provided is cloned to provide the tag for all listeners. In cases where this is not flexible
    /// enough, use [`IntoNotification::with_tag()`] instead.
    ///
    /// Tagging functions cannot be implemented efficiently for `no_std`, so this is only available
    /// when the `std` feature is enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use event_listener::{IntoNotification, Listener, Event};
    ///
    /// let event = Event::<bool>::with_tag();
    ///
    /// let mut listener1 = event.listen();
    /// let mut listener2 = event.listen();
    ///
    /// // Notify with `true` then `false`.
    /// event.notify(1.additional().tag(true));
    /// event.notify(1.additional().tag(false));
    ///
    /// assert_eq!(listener1.wait(), true);
    /// assert_eq!(listener2.wait(), false);
    /// ```
    #[cfg(feature = "std")]
    fn tag<T: Clone>(self, tag: T) -> Tag<Self::Notify, T>
    where
        Self: Sized + IntoNotification<Tag = ()>,
    {
        Tag::new(tag, self.into_notification())
    }

    /// Use a function to generate a tag with this notification.
    ///
    /// In many cases, it is desired to send additional information to the listener of the [`Event`]. For instance,
    /// it is possible to optimize a `Mutex` implementation by locking directly on the next listener, without
    /// needing to ever unlock the mutex at all.
    ///
    /// Tagging functions cannot be implemented efficiently for `no_std`, so this is only available
    /// when the `std` feature is enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use event_listener::{IntoNotification, Listener, Event};
    ///
    /// let event = Event::<bool>::with_tag();
    ///
    /// let mut listener1 = event.listen();
    /// let mut listener2 = event.listen();
    ///
    /// // Notify with `true` then `false`.
    /// event.notify(1.additional().tag_with(|| true));
    /// event.notify(1.additional().tag_with(|| false));
    ///
    /// assert_eq!(listener1.wait(), true);
    /// assert_eq!(listener2.wait(), false);
    /// ```
    #[cfg(feature = "std")]
    fn tag_with<T, F>(self, tag: F) -> TagWith<Self::Notify, F>
    where
        Self: Sized + IntoNotification<Tag = ()>,
        F: FnMut() -> T,
    {
        TagWith::new(tag, self.into_notification())
    }
}

impl<N: Notification> IntoNotification for N {
    type Tag = N::Tag;
    type Notify = N;

    fn into_notification(self) -> Self::Notify {
        self
    }
}

macro_rules! impl_for_numeric_types {
    ($($ty:ty)*) => {$(
        impl IntoNotification for $ty {
            type Tag = ();
            type Notify = Notify;

            #[allow(unused_comparisons)]
            fn into_notification(self) -> Self::Notify {
                if self < 0 {
                    panic!("negative notification count");
                }

                Notify::new(self.try_into().expect("overflow"))
            }
        }

        impl __private::Sealed for $ty {}
    )*};
}

impl_for_numeric_types! { usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 }

/// Equivalent to `atomic::fence(Ordering::SeqCst)`, but in some cases faster.
#[inline]
pub(super) fn full_fence() {
    #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), not(miri), not(loom)))]
    {
        use core::{arch::asm, cell::UnsafeCell};
        // HACK(stjepang): On x86 architectures there are two different ways of executing
        // a `SeqCst` fence.
        //
        // 1. `atomic::fence(SeqCst)`, which compiles into a `mfence` instruction.
        // 2. A `lock <op>` instruction.
        //
        // Both instructions have the effect of a full barrier, but empirical benchmarks have shown
        // that the second one is sometimes a bit faster.
        let a = UnsafeCell::new(0_usize);
        // It is common to use `lock or` here, but when using a local variable, `lock not`, which
        // does not change the flag, should be slightly more efficient.
        // Refs: https://www.felixcloutier.com/x86/not
        unsafe {
            #[cfg(target_pointer_width = "64")]
            asm!("lock not qword ptr [{0}]", in(reg) a.get(), options(nostack, preserves_flags));
            #[cfg(target_pointer_width = "32")]
            asm!("lock not dword ptr [{0:e}]", in(reg) a.get(), options(nostack, preserves_flags));
        }
        return;
    }
    #[allow(unreachable_code)]
    {
        atomic::fence(Ordering::SeqCst);
    }
}

mod __private {
    /// Make sure the NotificationPrivate trait can't be implemented outside of this crate.
    #[doc(hidden)]
    #[derive(Debug)]
    pub struct Internal(());

    impl Internal {
        pub(crate) fn new() -> Self {
            Self(())
        }
    }

    #[doc(hidden)]
    pub trait Sealed {}
    impl<N: super::NotificationPrivate + ?Sized> Sealed for N {}
}
