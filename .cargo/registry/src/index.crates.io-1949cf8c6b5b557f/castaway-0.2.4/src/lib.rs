//! Safe, zero-cost downcasting for limited compile-time specialization.
//!
//! This crate works fully on stable Rust, and also does not require the
//! standard library. To disable references to the standard library, you must
//! opt-out of the `std` feature using `default-features = false` in your
//! `Cargo.toml` file. When in no-std mode, a separate `alloc` feature flag
//! is available to support casting to several [`alloc`] types not included
//! in [`core`].
//!
//! Castaway provides the following key macros:
//!
//! - [`cast`]: Attempt to cast the result of an expression into a given
//!   concrete type.
//! - [`match_type`]: Match the result of an expression against multiple
//!   concrete types.

#![no_std]

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "alloc")]
extern crate alloc;

#[doc(hidden)]
pub mod internal;
mod lifetime_free;
mod utils;

pub use lifetime_free::LifetimeFree;

/// Attempt to cast the result of an expression into a given concrete type.
///
/// If the expression is in fact of the given type, an [`Ok`] is returned
/// containing the result of the expression as that type. If the types do not
/// match, the value is returned in an [`Err`] unchanged.
///
/// This macro is designed to work inside a generic context, and allows you to
/// downcast generic types to their concrete types or to another generic type at
/// compile time. If you are looking for the ability to downcast values at
/// runtime, you should use [`Any`](core::any::Any) instead.
///
/// This macro does not perform any sort of type _conversion_ (such as
/// re-interpreting `i32` as `u32` and so on), it only resolves generic types to
/// concrete types if the instantiated generic type is exactly the same as the
/// type you specify. If you are looking to reinterpret the bits of a value as a
/// type other than the one it actually is, then you should look for a different
/// library.
///
/// Invoking this macro is zero-cost, meaning after normal compiler optimization
/// steps there will be no code generated in the final binary for performing a
/// cast. In debug builds some glue code may be present with a small runtime
/// cost.
///
/// # Restrictions
///
/// Attempting to perform an illegal or unsupported cast that can never be
/// successful, such as casting to a value with a longer lifetime than the
/// expression, will produce a compile-time error.
///
/// Due to language limitations with lifetime bounds, this macro is more
/// restrictive than what is theoretically possible and rejects some legal
/// casts. This is to ensure safety and correctness around lifetime handling.
/// Examples include the following:
///
/// - Casting an expression by value with a non-`'static` lifetime is not
///   allowed. For example, you cannot attempt to cast a `T: 'a` to `Foo<'a>`.
/// - Casting to a reference with a non-`'static` lifetime is not allowed if the
///   expression type is not required to be a reference. For example, you can
///   attempt to cast a `&T` to `&String`, but you can't attempt to cast a `T`
///   to `&String` because `T` may or may not be a reference. You can, however,
///   attempt to cast a `T: 'static` to `&'static String`.
/// - You cannot cast references whose target itself may contain non-`'static`
///   references. For example, you can attempt to cast a `&'a T: 'static` to
///   `&'a Foo<'static>`, but you can't attempt to cast a `&'a T: 'b` to `&'a
///   Foo<'b>`.
/// - You can cast generic slices as long as the item type is `'static` and
///   `Sized`, but you cannot cast a generic reference to a slice or vice versa.
///
/// Some exceptions are made to the above restrictions for certain types which
/// are known to be _lifetime-free_. You can cast a generic type to any
/// lifetime-free type by value or by reference, even if the generic type is not
/// `'static`.
///
/// A type is considered lifetime-free if it contains no generic lifetime
/// bounds, ensuring that all possible instantiations of the type are always
/// `'static`. To mark a type as being lifetime-free and enable it to be casted
/// to in this manner by this macro it must implement the [`LifetimeFree`]
/// trait. This is implemented automatically for all primitive types and for
/// several [`core`] types. If you enable the `std` crate feature, then it will
/// also be implemented for several [`std`] types as well. If you enable the
/// `alloc` crate feature, then it will be implemented for several [`alloc`]
/// types without linking to the standard library as the `std` feature would.
///
/// # Examples
///
/// The above restrictions are admittedly complex and can be tricky to reason
/// about, so it is recommended to read the following examples to get a feel for
/// what is, and what is not, supported.
///
/// Performing trivial casts:
///
/// ```
/// use castaway::cast;
///
/// let value: u8 = 0;
/// assert_eq!(cast!(value, u8), Ok(0));
///
/// let slice: &[u8] = &[value];
/// assert_eq!(cast!(slice, &[u8]), Ok(slice));
/// ```
///
/// Performing a cast in a generic context:
///
/// ```
/// use castaway::cast;
///
/// fn is_this_a_u8<T: 'static>(value: T) -> bool {
///     cast!(value, u8).is_ok()
/// }
///
/// assert!(is_this_a_u8(0u8));
/// assert!(!is_this_a_u8(0u16));
///
/// // Note that we can also implement this without the `'static` type bound
/// // because the only type(s) we care about casting to all implement
/// // `LifetimeFree`:
///
/// fn is_this_a_u8_non_static<T>(value: T) -> bool {
///     cast!(value, u8).is_ok()
/// }
///
/// assert!(is_this_a_u8_non_static(0u8));
/// assert!(!is_this_a_u8_non_static(0u16));
/// ```
///
/// Specialization in a blanket trait implementation:
///
/// ```
/// # #[cfg(feature = "std")] {
/// use std::fmt::Display;
/// use castaway::cast;
///
/// /// Like `std::string::ToString`, but with an optimization when `Self` is
/// /// already a `String`.
/// ///
/// /// Since the standard library is allowed to use unstable features,
/// /// `ToString` already has this optimization using the `specialization`
/// /// feature, but this isn't something normal crates can do.
/// pub trait FastToString {
///     fn fast_to_string(&self) -> String;
/// }
///
/// impl<T: Display> FastToString for T {
///     fn fast_to_string<'local>(&'local self) -> String {
///         // If `T` is already a string, then take a different code path.
///         // After monomorphization, this check will be completely optimized
///         // away.
///         //
///         // Note we can cast a `&'local self` to a `&'local String` as `String`
///         // implements `LifetimeFree`.
///         if let Ok(string) = cast!(self, &String) {
///             // Don't invoke the std::fmt machinery, just clone the string.
///             string.to_owned()
///         } else {
///             // Make use of `Display` for any other `T`.
///             format!("{}", self)
///         }
///     }
/// }
///
/// println!("specialized: {}", String::from("hello").fast_to_string());
/// println!("default: {}", "hello".fast_to_string());
/// # }
/// ```
#[macro_export]
macro_rules! cast {
    ($value:expr, $T:ty) => {{
        #[allow(unused_imports)]
        use $crate::internal::*;

        // Here we are using an _autoderef specialization_ technique, which
        // exploits method resolution autoderefs to select different cast
        // implementations based on the type of expression passed in. The traits
        // imported above are all in scope and all have the potential to be
        // chosen to resolve the method name `try_cast` based on their generic
        // constraints.
        //
        // To support casting references with non-static lifetimes, the traits
        // limited to reference types require less dereferencing to invoke and
        // thus are preferred by the compiler if applicable.
        let value = $value;
        let src_token = CastToken::of_val(&value);
        let dest_token = CastToken::<$T>::of();

        // Note: The number of references added here must be kept in sync with
        // the largest number of references used by any trait implementation in
        // the internal module.
        let result: ::core::result::Result<$T, _> = (&&&&&&&(src_token, dest_token)).try_cast(value);

        result
    }};

    ($value:expr) => {
        $crate::cast!($value, _)
    };
}

/// Match the result of an expression against multiple concrete types.
///
/// You can write multiple match arms in the following syntax:
///
/// ```no_compile
/// TYPE as name => { /* expression */ }
/// ```
///
/// If the concrete type matches the given type, then the value will be cast to
/// that type and bound to the given variable name. The expression on the
/// right-hand side of the match is then executed and returned as the result of
/// the entire match expression.
///
/// The name following the `as` keyword can be any [irrefutable
/// pattern](https://doc.rust-lang.org/stable/reference/patterns.html#refutability).
/// Like `match` or `let` expressions, you can use an underscore to prevent
/// warnings if you don't use the casted value, such as `_value` or just `_`.
///
/// Since it would be impossible to exhaustively list all possible types of an
/// expression, you **must** include a final default match arm. The default
/// match arm does not specify a type:
///
/// ```no_compile
/// name => { /* expression */ }
/// ```
///
/// The original expression will be bound to the given variable name without
/// being casted. If you don't care about the original value, the default arm
/// can be:
///
/// ```no_compile
/// _ => { /* expression */ }
/// ```
///
/// This macro has all the same rules and restrictions around type casting as
/// [`cast`].
///
/// # Examples
///
/// ```
/// use std::fmt::Display;
/// use castaway::match_type;
///
/// fn to_string<T: Display + 'static>(value: T) -> String {
///     match_type!(value, {
///         String as s => s,
///         &str as s => s.to_string(),
///         s => s.to_string(),
///     })
/// }
///
/// println!("{}", to_string("foo"));
/// ```
#[macro_export]
macro_rules! match_type {
    ($value:expr, {
        $T:ty as $pat:pat => $branch:expr,
        $($tail:tt)+
    }) => {
        match $crate::cast!($value, $T) {
            Ok(value) => {
                let $pat = value;
                $branch
            },
            Err(value) => $crate::match_type!(value, {
                $($tail)*
            })
        }
    };

    ($value:expr, {
        $pat:pat => $branch:expr $(,)?
    }) => {{
        let $pat = $value;
        $branch
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "alloc")]
    use alloc::string::String;

    #[test]
    fn cast() {
        assert_eq!(cast!(0u8, u16), Err(0u8));
        assert_eq!(cast!(1u8, u8), Ok(1u8));
        assert_eq!(cast!(2u8, &'static u8), Err(2u8));
        assert_eq!(cast!(2u8, &u8), Err(2u8)); // 'static is inferred

        static VALUE: u8 = 2u8;
        assert_eq!(cast!(&VALUE, &u8), Ok(&2u8));
        assert_eq!(cast!(&VALUE, &'static u8), Ok(&2u8));
        assert_eq!(cast!(&VALUE, &u16), Err(&2u8));
        assert_eq!(cast!(&VALUE, &i8), Err(&2u8));

        let value = 2u8;

        fn inner<'a>(value: &'a u8) {
            assert_eq!(cast!(value, &u8), Ok(&2u8));
            assert_eq!(cast!(value, &'a u8), Ok(&2u8));
            assert_eq!(cast!(value, &u16), Err(&2u8));
            assert_eq!(cast!(value, &i8), Err(&2u8));
        }

        inner(&value);

        let mut slice = [1u8; 2];

        fn inner2<'a>(value: &'a [u8]) {
            assert_eq!(cast!(value, &[u8]), Ok(&[1, 1][..]));
            assert_eq!(cast!(value, &'a [u8]), Ok(&[1, 1][..]));
            assert_eq!(cast!(value, &'a [u16]), Err(&[1, 1][..]));
            assert_eq!(cast!(value, &'a [i8]), Err(&[1, 1][..]));
        }

        inner2(&slice);

        #[allow(clippy::needless_lifetimes)]
        fn inner3<'a>(value: &'a mut [u8]) {
            assert_eq!(cast!(value, &mut [u8]), Ok(&mut [1, 1][..]));
        }
        inner3(&mut slice);
    }

    #[test]
    fn cast_with_type_inference() {
        let result: Result<u8, u8> = cast!(0u8);
        assert_eq!(result, Ok(0u8));

        let result: Result<u8, u16> = cast!(0u16);
        assert_eq!(result, Err(0u16));
    }

    #[test]
    fn match_type() {
        let v = 42i32;

        assert!(match_type!(v, {
            u32 as _ => false,
            i32 as _ => true,
            _ => false,
        }));
    }

    #[test]
    fn cast_lifetime_free_unsized_ref() {
        fn can_cast<T>(value: &[T]) -> bool {
            cast!(value, &[u8]).is_ok()
        }

        let value = 42i32;
        assert!(can_cast(&[1_u8, 2, 3, 4]));
        assert!(!can_cast(&[1_i8, 2, 3, 4]));
        assert!(!can_cast(&[&value, &value]));
    }

    #[test]
    fn cast_lifetime_free_unsized_mut() {
        fn can_cast<T>(value: &mut [T]) -> bool {
            cast!(value, &mut [u8]).is_ok()
        }

        let value = 42i32;
        assert!(can_cast(&mut [1_u8, 2, 3, 4]));
        assert!(!can_cast(&mut [1_i8, 2, 3, 4]));
        assert!(!can_cast(&mut [&value, &value]));
    }

    macro_rules! test_lifetime_free_cast {
        () => {};

        (
            $(#[$meta:meta])*
            for $TARGET:ty as $name:ident {
                $(
                    $value:expr => $matches:pat $(if $guard:expr)?,
                )+
            }
            $($tail:tt)*
        ) => {
            paste::paste! {
                $(#[$meta])*
                #[test]
                #[allow(non_snake_case)]
                fn [<cast_lifetime_free_ $name>]() {
                    fn do_cast<T>(value: T) -> Result<$TARGET, T> {
                        cast!(value, $TARGET)
                    }

                    $(
                        assert!(match do_cast($value) {
                            $matches $(if $guard)* => true,
                            _ => false,
                        });
                    )*
                }

                $(#[$meta])*
                #[test]
                #[allow(non_snake_case)]
                fn [<cast_lifetime_free_ref_ $name>]() {
                    fn do_cast<T>(value: &T) -> Result<&$TARGET, &T> {
                        cast!(value, &$TARGET)
                    }

                    $(
                        assert!(match do_cast(&$value).map(|t| t.clone()).map_err(|e| e.clone()) {
                            $matches $(if $guard)* => true,
                            _ => false,
                        });
                    )*
                }

                $(#[$meta])*
                #[test]
                #[allow(non_snake_case)]
                fn [<cast_lifetime_free_mut_ $name>]() {
                    fn do_cast<T>(value: &mut T) -> Result<&mut $TARGET, &mut T> {
                        cast!(value, &mut $TARGET)
                    }

                    $(
                        assert!(match do_cast(&mut $value).map(|t| t.clone()).map_err(|e| e.clone()) {
                            $matches $(if $guard)* => true,
                            _ => false,
                        });
                    )*
                }
            }

            test_lifetime_free_cast! {
                $($tail)*
            }
        };

        (
            $(#[$meta:meta])*
            for $TARGET:ty {
                $(
                    $value:expr => $matches:pat $(if $guard:expr)?,
                )+
            }
            $($tail:tt)*
        ) => {
            paste::paste! {
                $(#[$meta])*
                #[test]
                #[allow(non_snake_case)]
                fn [<cast_lifetime_free_ $TARGET>]() {
                    fn do_cast<T>(value: T) -> Result<$TARGET, T> {
                        cast!(value, $TARGET)
                    }

                    $(
                        assert!(match do_cast($value) {
                            $matches $(if $guard)* => true,
                            _ => false,
                        });
                    )*
                }

                $(#[$meta])*
                #[test]
                #[allow(non_snake_case)]
                fn [<cast_lifetime_free_ref_ $TARGET>]() {
                    fn do_cast<T>(value: &T) -> Result<&$TARGET, &T> {
                        cast!(value, &$TARGET)
                    }

                    $(
                        assert!(match do_cast(&$value).map(|t| t.clone()).map_err(|e| e.clone()) {
                            $matches $(if $guard)* => true,
                            _ => false,
                        });
                    )*
                }

                $(#[$meta])*
                #[test]
                #[allow(non_snake_case)]
                fn [<cast_lifetime_free_mut_ $TARGET>]() {
                    fn do_cast<T>(value: &mut T) -> Result<&mut $TARGET, &mut T> {
                        cast!(value, &mut $TARGET)
                    }

                    $(
                        assert!(match do_cast(&mut $value).map(|t| t.clone()).map_err(|e| e.clone()) {
                            $matches $(if $guard)* => true,
                            _ => false,
                        });
                    )*
                }
            }

            test_lifetime_free_cast! {
                $($tail)*
            }
        };
    }

    test_lifetime_free_cast! {
        for bool {
            0u8 => Err(_),
            true => Ok(true),
        }

        for u8 {
            0u8 => Ok(0u8),
            1u16 => Err(1u16),
            42u8 => Ok(42u8),
        }

        for f32 {
            3.2f32 => Ok(v) if v == 3.2,
            3.2f64 => Err(v) if v == 3.2f64,
        }

        #[cfg(feature = "alloc")]
        for String {
            String::from("hello world") => Ok(ref v) if v.as_str() == "hello world",
            "hello world" => Err("hello world"),
        }

        for Option<u8> as Option_u8 {
            0u8 => Err(0u8),
            Some(42u8) => Ok(Some(42u8)),
        }

        // See https://github.com/rust-lang/rust/issues/127286 for details.
        for (u8, u8) as TupleU8U8 {
            (1u8, 2u8) => Ok((1u8, 2u8)),
            1u8 => Err(1u8),
        }
        for (bool, u16) as TupleBoolU16 {
            (false, 2u16) => Ok((false, 2u16)),
            true => Err(true),
        }
    }
}
