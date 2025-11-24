/// Marker trait for types that do not contain any lifetime parameters. Such
/// types are safe to cast from non-static type parameters if their types are
/// equal.
///
/// This trait is used by [`cast!`](crate::cast) to determine what casts are legal on values
/// without a `'static` type constraint.
///
/// # Safety
///
/// When implementing this trait for a type, you must ensure that the type is
/// free of any lifetime parameters. Failure to meet **all** of the requirements
/// below may result in undefined behavior.
///
/// - The type must be `'static`.
/// - The type must be free of lifetime parameters. In other words, the type
///   must be an "owned" type and not contain *any* lifetime parameters.
/// - All contained fields must also be `LifetimeFree`.
///
/// # Examples
///
/// ```
/// use castaway::LifetimeFree;
///
/// struct Container<T>(T);
///
/// // UNDEFINED BEHAVIOR!!
/// // unsafe impl LifetimeFree for Container<&'static str> {}
///
/// // UNDEFINED BEHAVIOR!!
/// // unsafe impl<T> LifetimeFree for Container<T> {}
///
/// // This is safe.
/// unsafe impl<T: LifetimeFree> LifetimeFree for Container<T> {}
///
/// struct PlainOldData {
///     foo: u8,
///     bar: bool,
/// }
///
/// // This is also safe, since all fields are known to be `LifetimeFree`.
/// unsafe impl LifetimeFree for PlainOldData {}
/// ```
pub unsafe trait LifetimeFree {}

unsafe impl LifetimeFree for () {}
unsafe impl LifetimeFree for bool {}
unsafe impl LifetimeFree for char {}
unsafe impl LifetimeFree for f32 {}
unsafe impl LifetimeFree for f64 {}
unsafe impl LifetimeFree for i8 {}
unsafe impl LifetimeFree for i16 {}
unsafe impl LifetimeFree for i32 {}
unsafe impl LifetimeFree for i64 {}
unsafe impl LifetimeFree for i128 {}
unsafe impl LifetimeFree for isize {}
unsafe impl LifetimeFree for str {}
unsafe impl LifetimeFree for u8 {}
unsafe impl LifetimeFree for u16 {}
unsafe impl LifetimeFree for u32 {}
unsafe impl LifetimeFree for u64 {}
unsafe impl LifetimeFree for u128 {}
unsafe impl LifetimeFree for usize {}

unsafe impl LifetimeFree for core::num::NonZeroI8 {}
unsafe impl LifetimeFree for core::num::NonZeroI16 {}
unsafe impl LifetimeFree for core::num::NonZeroI32 {}
unsafe impl LifetimeFree for core::num::NonZeroI64 {}
unsafe impl LifetimeFree for core::num::NonZeroI128 {}
unsafe impl LifetimeFree for core::num::NonZeroIsize {}
unsafe impl LifetimeFree for core::num::NonZeroU8 {}
unsafe impl LifetimeFree for core::num::NonZeroU16 {}
unsafe impl LifetimeFree for core::num::NonZeroU32 {}
unsafe impl LifetimeFree for core::num::NonZeroU64 {}
unsafe impl LifetimeFree for core::num::NonZeroU128 {}
unsafe impl LifetimeFree for core::num::NonZeroUsize {}

unsafe impl<T: LifetimeFree> LifetimeFree for [T] {}
#[rustversion::since(1.51)]
unsafe impl<T: LifetimeFree, const SIZE: usize> LifetimeFree for [T; SIZE] {}
unsafe impl<T: LifetimeFree> LifetimeFree for Option<T> {}
unsafe impl<T: LifetimeFree, E: LifetimeFree> LifetimeFree for Result<T, E> {}
unsafe impl<T: LifetimeFree> LifetimeFree for core::num::Wrapping<T> {}
unsafe impl<T: LifetimeFree> LifetimeFree for core::cell::Cell<T> {}
unsafe impl<T: LifetimeFree> LifetimeFree for core::cell::RefCell<T> {}

macro_rules! tuple_impls {
    ($( $( $name:ident )+, )+) => {
        $(
            unsafe impl<$($name: LifetimeFree),+> LifetimeFree for ($($name,)+) {}
        )+
    };
}

tuple_impls! {
    T0,
    T0 T1,
    T0 T1 T2,
    T0 T1 T2 T3,
    T0 T1 T2 T3 T4,
    T0 T1 T2 T3 T4 T5,
    T0 T1 T2 T3 T4 T5 T6,
    T0 T1 T2 T3 T4 T5 T6 T7,
    T0 T1 T2 T3 T4 T5 T6 T7 T8,
    T0 T1 T2 T3 T4 T5 T6 T7 T8 T9,
}

#[cfg(feature = "alloc")]
mod alloc_impls {
    use super::LifetimeFree;

    unsafe impl LifetimeFree for alloc::string::String {}

    unsafe impl<T: LifetimeFree> LifetimeFree for alloc::boxed::Box<T> {}
    unsafe impl<T: LifetimeFree> LifetimeFree for alloc::vec::Vec<T> {}

    #[rustversion::attr(since(1.60), cfg(target_has_atomic = "ptr"))]
    unsafe impl<T: LifetimeFree> LifetimeFree for alloc::sync::Arc<T> {}
}
