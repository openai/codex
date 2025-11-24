use super::*;

// Note(Lokathor): This is the neat part!!
unsafe impl<T: ZeroableInOption> Zeroable for Option<T> {}

/// Trait for types which are [Zeroable](Zeroable) when wrapped in
/// [Option](core::option::Option).
///
/// ## Safety
///
/// * `Option<YourType>` must uphold the same invariants as
///   [Zeroable](Zeroable).
pub unsafe trait ZeroableInOption: Sized {}

unsafe impl ZeroableInOption for NonZeroI8 {}
unsafe impl ZeroableInOption for NonZeroI16 {}
unsafe impl ZeroableInOption for NonZeroI32 {}
unsafe impl ZeroableInOption for NonZeroI64 {}
unsafe impl ZeroableInOption for NonZeroI128 {}
unsafe impl ZeroableInOption for NonZeroIsize {}
unsafe impl ZeroableInOption for NonZeroU8 {}
unsafe impl ZeroableInOption for NonZeroU16 {}
unsafe impl ZeroableInOption for NonZeroU32 {}
unsafe impl ZeroableInOption for NonZeroU64 {}
unsafe impl ZeroableInOption for NonZeroU128 {}
unsafe impl ZeroableInOption for NonZeroUsize {}

// Note: this does not create NULL vtable because we get `None` anyway.
unsafe impl<T: ?Sized> ZeroableInOption for NonNull<T> {}
unsafe impl<T: ?Sized> ZeroableInOption for &'_ T {}
unsafe impl<T: ?Sized> ZeroableInOption for &'_ mut T {}

#[cfg(feature = "extern_crate_alloc")]
#[cfg_attr(feature = "nightly_docs", doc(cfg(feature = "extern_crate_alloc")))]
unsafe impl<T: ?Sized> ZeroableInOption for alloc::boxed::Box<T> {}

#[cfg(feature = "zeroable_unwind_fn")]
macro_rules! impl_for_unwind_fn {
    ($($ArgTy:ident),* $(,)?) => {
        unsafe impl<$($ArgTy,)* R> ZeroableInOption for extern "C-unwind" fn($($ArgTy,)*) -> R {}
        unsafe impl<$($ArgTy,)* R> ZeroableInOption for unsafe extern "C-unwind" fn($($ArgTy,)*) -> R {}
        unsafe impl<$($ArgTy,)* R> ZeroableInOption for extern "system-unwind" fn($($ArgTy,)*) -> R {}
        unsafe impl<$($ArgTy,)* R> ZeroableInOption for unsafe extern "system-unwind" fn($($ArgTy,)*) -> R {}
    };
}

macro_rules! impl_for_fn {
    ($($ArgTy:ident),* $(,)?) => {
        unsafe impl<$($ArgTy,)* R> ZeroableInOption for fn($($ArgTy,)*) -> R {}
        unsafe impl<$($ArgTy,)* R> ZeroableInOption for unsafe fn($($ArgTy,)*) -> R {}
        unsafe impl<$($ArgTy,)* R> ZeroableInOption for extern "C" fn($($ArgTy,)*) -> R {}
        unsafe impl<$($ArgTy,)* R> ZeroableInOption for unsafe extern "C" fn($($ArgTy,)*) -> R {}
        unsafe impl<$($ArgTy,)* R> ZeroableInOption for extern "system" fn($($ArgTy,)*) -> R {}
        unsafe impl<$($ArgTy,)* R> ZeroableInOption for unsafe extern "system" fn($($ArgTy,)*) -> R {}
        #[cfg(target_os="windows")]
        unsafe impl<$($ArgTy,)* R> ZeroableInOption for extern "stdcall" fn($($ArgTy,)*) -> R {}
        #[cfg(target_os="windows")]
        unsafe impl<$($ArgTy,)* R> ZeroableInOption for unsafe extern "stdcall" fn($($ArgTy,)*) -> R {}
        #[cfg(feature = "zeroable_unwind_fn")]
        impl_for_unwind_fn!($($ArgTy),*);
    };
}

impl_for_fn!();
impl_for_fn!(A);
impl_for_fn!(A, B);
impl_for_fn!(A, B, C);
impl_for_fn!(A, B, C, D);
impl_for_fn!(A, B, C, D, E);
impl_for_fn!(A, B, C, D, E, F);
impl_for_fn!(A, B, C, D, E, F, G);
impl_for_fn!(A, B, C, D, E, F, G, H);
impl_for_fn!(A, B, C, D, E, F, G, H, I);
impl_for_fn!(A, B, C, D, E, F, G, H, I, J);
impl_for_fn!(A, B, C, D, E, F, G, H, I, J, K);
impl_for_fn!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_for_fn!(A, B, C, D, E, F, G, H, I, J, K, L, M);
