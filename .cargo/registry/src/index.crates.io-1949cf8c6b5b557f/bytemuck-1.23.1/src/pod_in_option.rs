use super::*;

// Note(Lokathor): This is the neat part!!
unsafe impl<T: PodInOption> Pod for Option<T> {}

/// Trait for types which are [Pod](Pod) when wrapped in
/// [Option](core::option::Option).
///
/// ## Safety
///
/// * `Option<T>` must uphold the same invariants as [Pod](Pod).
/// * **Reminder:** pointers are **not** pod! **Do not** mix this trait with a
///   newtype over [NonNull](core::ptr::NonNull).
pub unsafe trait PodInOption: ZeroableInOption + Copy + 'static {}

unsafe impl PodInOption for NonZeroI8 {}
unsafe impl PodInOption for NonZeroI16 {}
unsafe impl PodInOption for NonZeroI32 {}
unsafe impl PodInOption for NonZeroI64 {}
unsafe impl PodInOption for NonZeroI128 {}
unsafe impl PodInOption for NonZeroIsize {}
unsafe impl PodInOption for NonZeroU8 {}
unsafe impl PodInOption for NonZeroU16 {}
unsafe impl PodInOption for NonZeroU32 {}
unsafe impl PodInOption for NonZeroU64 {}
unsafe impl PodInOption for NonZeroU128 {}
unsafe impl PodInOption for NonZeroUsize {}
