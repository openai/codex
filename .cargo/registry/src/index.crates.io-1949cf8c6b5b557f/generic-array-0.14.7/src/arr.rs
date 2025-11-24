//! Implementation for `arr!` macro.

use super::ArrayLength;
use core::ops::Add;
use typenum::U1;

/// Helper trait for `arr!` macro
pub trait AddLength<T, N: ArrayLength<T>>: ArrayLength<T> {
    /// Resulting length
    type Output: ArrayLength<T>;
}

impl<T, N1, N2> AddLength<T, N2> for N1
where
    N1: ArrayLength<T> + Add<N2>,
    N2: ArrayLength<T>,
    <N1 as Add<N2>>::Output: ArrayLength<T>,
{
    type Output = <N1 as Add<N2>>::Output;
}

/// Helper type for `arr!` macro
pub type Inc<T, U> = <U as AddLength<T, U1>>::Output;

#[doc(hidden)]
#[macro_export]
macro_rules! arr_impl {
    (@replace_expr $e:expr) => { 1 };
    ($T:ty; $N:ty, [$($x:expr),*], []) => ({
        const __ARR_LENGTH: usize = 0 $(+ $crate::arr_impl!(@replace_expr $x) )*;

        #[inline(always)]
        fn __do_transmute<T, N: $crate::ArrayLength<T>>(arr: [T; __ARR_LENGTH]) -> $crate::GenericArray<T, N> {
            unsafe { $crate::transmute(arr) }
        }

        let _: [(); <$N as $crate::typenum::Unsigned>::USIZE] = [(); __ARR_LENGTH];

        __do_transmute::<$T, $N>([$($x as $T),*])
    });
    ($T:ty; $N:ty, [], [$x1:expr]) => (
        $crate::arr_impl!($T; $crate::arr::Inc<$T, $N>, [$x1], [])
    );
    ($T:ty; $N:ty, [], [$x1:expr, $($x:expr),+]) => (
        $crate::arr_impl!($T; $crate::arr::Inc<$T, $N>, [$x1], [$($x),+])
    );
    ($T:ty; $N:ty, [$($y:expr),+], [$x1:expr]) => (
        $crate::arr_impl!($T; $crate::arr::Inc<$T, $N>, [$($y),+, $x1], [])
    );
    ($T:ty; $N:ty, [$($y:expr),+], [$x1:expr, $($x:expr),+]) => (
        $crate::arr_impl!($T; $crate::arr::Inc<$T, $N>, [$($y),+, $x1], [$($x),+])
    );
}

/// Macro allowing for easy generation of Generic Arrays.
/// Example: `let test = arr![u32; 1, 2, 3];`
#[macro_export]
macro_rules! arr {
    ($T:ty; $(,)*) => ({
        unsafe { $crate::transmute::<[$T; 0], $crate::GenericArray<$T, $crate::typenum::U0>>([]) }
    });
    ($T:ty; $($x:expr),* $(,)*) => (
        $crate::arr_impl!($T; $crate::typenum::U0, [], [$($x),*])
    );
    ($($x:expr,)+) => (arr![$($x),+]);
    () => ("""Macro requires a type, e.g. `let array = arr![u32; 1, 2, 3];`")
}

mod doctests_only {
    ///
    /// # With ellision
    ///
    /// Testing that lifetimes aren't transmuted when they're ellided.
    ///
    /// ```compile_fail
    /// #[macro_use] extern crate generic_array;
    /// fn main() {
    ///    fn unsound_lifetime_extension<'a, A>(a: &'a A) -> &'static A {
    ///        arr![&A; a][0]
    ///    }
    /// }
    /// ```
    ///
    /// ```rust
    /// #[macro_use] extern crate generic_array;
    /// fn main() {
    ///    fn unsound_lifetime_extension<'a, A>(a: &'a A) -> &'a A {
    ///        arr![&A; a][0]
    ///    }
    /// }
    /// ```
    ///
    /// # Without ellision
    ///
    /// Testing that lifetimes aren't transmuted when they're specified explicitly.
    ///
    /// ```compile_fail
    /// #[macro_use] extern crate generic_array;
    /// fn main() {
    ///    fn unsound_lifetime_extension<'a, A>(a: &'a A) -> &'static A {
    ///        arr![&'a A; a][0]
    ///    }
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// #[macro_use] extern crate generic_array;
    /// fn main() {
    ///    fn unsound_lifetime_extension<'a, A>(a: &'a A) -> &'static A {
    ///        arr![&'static A; a][0]
    ///    }
    /// }
    /// ```
    ///
    /// ```rust
    /// #[macro_use] extern crate generic_array;
    /// fn main() {
    ///    fn unsound_lifetime_extension<'a, A>(a: &'a A) -> &'a A {
    ///        arr![&'a A; a][0]
    ///    }
    /// }
    /// ```
    #[allow(dead_code)]
    pub enum DocTests {}
}
