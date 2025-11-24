/// Asserts that _all_ types in a list are equal to each other.
///
/// # Examples
///
/// Often times, type aliases are used to express usage semantics via naming. In
/// some cases, the underlying type may differ based on platform. However, other
/// types like [`c_float`] will always alias the same type.
///
/// ```
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// use std::os::raw::c_float;
///
/// assert_type_eq_all!(c_float, f32);
/// ```
///
/// This macro can also be used to compare types that involve lifetimes! Just
/// use `'static` in that case:
///
/// ```
/// # #[macro_use] extern crate static_assertions;
/// # fn main() {
/// type Buf<'a> = &'a [u8];
///
/// assert_type_eq_all!(Buf<'static>, &'static [u8]);
/// # }
/// ```
///
/// The following example fails to compile because `String` and `str` do not
/// refer to the same type:
///
/// ```compile_fail
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// assert_type_eq_all!(String, str);
/// ```
///
/// This should also work the other way around, regardless of [`Deref`]
/// implementations.
///
/// ```compile_fail
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// assert_type_eq_all!(str, String);
/// ```
///
/// [`c_float`]: https://doc.rust-lang.org/std/os/raw/type.c_float.html
/// [`Deref`]: https://doc.rust-lang.org/std/ops/trait.Deref.html
#[macro_export]
macro_rules! assert_type_eq_all {
    ($x:ty, $($xs:ty),+ $(,)*) => {
        const _: fn() = || { $({
            trait TypeEq {
                type This: ?Sized;
            }

            impl<T: ?Sized> TypeEq for T {
                type This = Self;
            }

            fn assert_type_eq_all<T, U>()
            where
                T: ?Sized + TypeEq<This = U>,
                U: ?Sized,
            {}

            assert_type_eq_all::<$x, $xs>();
        })+ };
    };
}

/// Asserts that _all_ types are **not** equal to each other.
///
/// # Examples
///
/// Rust has all sorts of slices, but they represent different types of data:
///
/// ```
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// assert_type_ne_all!([u8], [u16], str);
/// ```
///
/// The following example fails to compile because [`c_uchar`] is a type alias
/// for [`u8`]:
///
/// ```compile_fail
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// use std::os::raw::c_uchar;
///
/// assert_type_ne_all!(c_uchar, u8, u32);
/// ```
///
/// [`c_uchar`]: https://doc.rust-lang.org/std/os/raw/type.c_uchar.html
/// [`u8`]: https://doc.rust-lang.org/std/primitive.u8.html
#[macro_export]
macro_rules! assert_type_ne_all {
    ($x:ty, $($y:ty),+ $(,)?) => {
        const _: fn() = || {
            trait MutuallyExclusive {}
            impl MutuallyExclusive for $x {}
            $(impl MutuallyExclusive for $y {})+
        };
    };
}
