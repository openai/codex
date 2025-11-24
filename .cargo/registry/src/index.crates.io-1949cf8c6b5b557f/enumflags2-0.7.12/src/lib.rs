//! # Enum Flags
//! `enumflags2` implements the classic bitflags datastructure. Annotate an enum
//! with `#[bitflags]`, and `BitFlags<YourEnum>` will be able to hold arbitrary combinations
//! of your enum within the space of a single integer.
//!
//! Unlike other crates, `enumflags2` makes the type-level distinction between
//! a single flag (`YourEnum`) and a set of flags (`BitFlags<YourEnum>`).
//! This allows idiomatic handling of bitflags, such as with `match` and `iter`.
//!
//! ## Example
//! ```
//! use enumflags2::{bitflags, make_bitflags, BitFlags};
//!
//! #[bitflags]
//! #[repr(u8)]
//! #[derive(Copy, Clone, Debug, PartialEq)]
//! enum Test {
//!     A = 0b0001,
//!     B = 0b0010,
//!     C, // unspecified variants pick unused bits automatically
//!     D = 0b1000,
//! }
//!
//! // Flags can be combined with |, this creates a BitFlags of your type:
//! let a_b: BitFlags<Test> = Test::A | Test::B;
//! let a_c = Test::A | Test::C;
//! let b_c_d = make_bitflags!(Test::{B | C | D});
//!
//! // The debug output lets you inspect both the numeric value and
//! // the actual flags:
//! assert_eq!(format!("{:?}", a_b), "BitFlags<Test>(0b11, A | B)");
//!
//! // But if you'd rather see only one of those, that's available too:
//! assert_eq!(format!("{}", a_b), "A | B");
//! assert_eq!(format!("{:04b}", a_b), "0011");
//!
//! // Iterate over the flags like a normal set
//! assert_eq!(a_b.iter().collect::<Vec<_>>(), &[Test::A, Test::B]);
//!
//! // Query the contents with contains and intersects
//! assert!(a_b.contains(Test::A));
//! assert!(b_c_d.contains(Test::B | Test::C));
//! assert!(!(b_c_d.contains(a_b)));
//!
//! assert!(a_b.intersects(a_c));
//! assert!(!(a_b.intersects(Test::C | Test::D)));
//! ```
//!
//! ## Optional Feature Flags
//!
//! - [`serde`](https://serde.rs/) implements `Serialize` and `Deserialize`
//!   for `BitFlags<T>`.
//! - `std` implements `std::error::Error` for `FromBitsError`.
//!
//! ## `const fn`-compatible APIs
//!
//! **Background:** The subset of `const fn` features currently stabilized is pretty limited.
//! Most notably, [const traits are still at the RFC stage][const-trait-rfc],
//! which makes it impossible to use any overloaded operators in a const
//! context.
//!
//! **Naming convention:** If a separate, more limited function is provided
//! for usage in a `const fn`, the name is suffixed with `_c`.
//!
//! Apart from functions whose name ends with `_c`, the [`make_bitflags!`] macro
//! is often useful for many `const` and `const fn` usecases.
//!
//! **Blanket implementations:** If you attempt to write a `const fn` ranging
//! over `T: BitFlag`, you will be met with an error explaining that currently,
//! the only allowed trait bound for a `const fn` is `?Sized`. You will probably
//! want to write a separate implementation for `BitFlags<T, u8>`,
//! `BitFlags<T, u16>`, etc — best accomplished by a simple macro.
//!
//! **Documentation considerations:** The strategy described above is often used
//! by `enumflags2` itself. To avoid clutter in the auto-generated documentation,
//! the implementations for widths other than `u8` are marked with `#[doc(hidden)]`.
//!
//! ## Customizing `Default`
//!
//! By default, creating an instance of `BitFlags<T>` with `Default` will result in an empty
//! set. If that's undesirable, you may customize this:
//!
//! ```
//! # use enumflags2::{BitFlags, bitflags};
//! #[bitflags(default = B | C)]
//! #[repr(u8)]
//! #[derive(Copy, Clone, Debug, PartialEq)]
//! enum Test {
//!     A = 0b0001,
//!     B = 0b0010,
//!     C = 0b0100,
//!     D = 0b1000,
//! }
//!
//! assert_eq!(BitFlags::default(), Test::B | Test::C);
//! ```
//!
//! [const-trait-rfc]: https://github.com/rust-lang/rfcs/pull/2632
#![warn(missing_docs)]
#![cfg_attr(all(not(test), not(feature = "std")), no_std)]

use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::{cmp, ops};

#[allow(unused_imports)]
#[macro_use]
extern crate enumflags2_derive;

#[doc(hidden)]
pub use enumflags2_derive::bitflags_internal as bitflags;

// Internal macro: expand into a separate copy for each supported numeric type.
macro_rules! for_each_uint {
    ( $d:tt $tyvar:ident $dd:tt $docattr:ident => $($input:tt)* ) => {
        macro_rules! implement {
            ( $d $tyvar:ty => $d($d $docattr:meta)? ) => {
                $($input)*
            }
        }

        implement! { u8 => }
        implement! { u16 => doc(hidden) }
        implement! { u32 => doc(hidden) }
        implement! { u64 => doc(hidden) }
        implement! { u128 => doc(hidden) }
    }
}

/// A trait automatically implemented by `#[bitflags]` to make the enum
/// a valid type parameter for `BitFlags<T>`.
pub trait BitFlag: Copy + Clone + 'static + _internal::RawBitFlags {
    /// Create a `BitFlags` with no flags set (in other words, with a value of 0).
    ///
    /// This is a convenience reexport of [`BitFlags::empty`]. It can be called with
    /// `MyFlag::empty()`, thus bypassing the need for type hints in some situations.
    ///
    /// ```
    /// # use enumflags2::{bitflags, BitFlags};
    /// #[bitflags]
    /// #[repr(u8)]
    /// #[derive(Clone, Copy, PartialEq, Eq)]
    /// enum MyFlag {
    ///     One = 1 << 0,
    ///     Two = 1 << 1,
    ///     Three = 1 << 2,
    /// }
    ///
    /// use enumflags2::BitFlag;
    ///
    /// let empty = MyFlag::empty();
    /// assert!(empty.is_empty());
    /// assert_eq!(empty.contains(MyFlag::One), false);
    /// assert_eq!(empty.contains(MyFlag::Two), false);
    /// assert_eq!(empty.contains(MyFlag::Three), false);
    /// ```
    #[inline]
    fn empty() -> BitFlags<Self> {
        BitFlags::empty()
    }

    /// Create a `BitFlags` with all flags set.
    ///
    /// This is a convenience reexport of [`BitFlags::all`]. It can be called with
    /// `MyFlag::all()`, thus bypassing the need for type hints in some situations.
    ///
    /// ```
    /// # use enumflags2::{bitflags, BitFlags};
    /// #[bitflags]
    /// #[repr(u8)]
    /// #[derive(Clone, Copy, PartialEq, Eq)]
    /// enum MyFlag {
    ///     One = 1 << 0,
    ///     Two = 1 << 1,
    ///     Three = 1 << 2,
    /// }
    ///
    /// use enumflags2::BitFlag;
    ///
    /// let all = MyFlag::all();
    /// assert!(all.is_all());
    /// assert_eq!(all.contains(MyFlag::One), true);
    /// assert_eq!(all.contains(MyFlag::Two), true);
    /// assert_eq!(all.contains(MyFlag::Three), true);
    /// ```
    #[inline]
    fn all() -> BitFlags<Self> {
        BitFlags::all()
    }

    /// Create a `BitFlags` if the raw value provided does not contain
    /// any illegal flags.
    ///
    /// This is a convenience reexport of [`BitFlags::from_bits`]. It can be called
    /// with `MyFlag::from_bits(bits)`, thus bypassing the need for type hints in
    /// some situations.
    ///
    /// ```
    /// # use enumflags2::{bitflags, BitFlags};
    /// #[bitflags]
    /// #[repr(u8)]
    /// #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    /// enum MyFlag {
    ///     One = 1 << 0,
    ///     Two = 1 << 1,
    ///     Three = 1 << 2,
    /// }
    ///
    /// use enumflags2::BitFlag;
    ///
    /// let flags = MyFlag::from_bits(0b11).unwrap();
    /// assert_eq!(flags.contains(MyFlag::One), true);
    /// assert_eq!(flags.contains(MyFlag::Two), true);
    /// assert_eq!(flags.contains(MyFlag::Three), false);
    /// let invalid = MyFlag::from_bits(1 << 3);
    /// assert!(invalid.is_err());
    /// ```
    #[inline]
    fn from_bits(bits: Self::Numeric) -> Result<BitFlags<Self>, FromBitsError<Self>> {
        BitFlags::from_bits(bits)
    }

    /// Create a `BitFlags` from an underlying bitwise value. If any
    /// invalid bits are set, ignore them.
    ///
    /// This is a convenience reexport of [`BitFlags::from_bits_truncate`]. It can be
    /// called with `MyFlag::from_bits_truncate(bits)`, thus bypassing the need for
    /// type hints in some situations.
    ///
    /// ```
    /// # use enumflags2::{bitflags, BitFlags};
    /// #[bitflags]
    /// #[repr(u8)]
    /// #[derive(Clone, Copy, PartialEq, Eq)]
    /// enum MyFlag {
    ///     One = 1 << 0,
    ///     Two = 1 << 1,
    ///     Three = 1 << 2,
    /// }
    ///
    /// use enumflags2::BitFlag;
    ///
    /// let flags = MyFlag::from_bits_truncate(0b1_1011);
    /// assert_eq!(flags.contains(MyFlag::One), true);
    /// assert_eq!(flags.contains(MyFlag::Two), true);
    /// assert_eq!(flags.contains(MyFlag::Three), false);
    /// ```
    #[inline]
    fn from_bits_truncate(bits: Self::Numeric) -> BitFlags<Self> {
        BitFlags::from_bits_truncate(bits)
    }

    /// Create a `BitFlags` unsafely, without checking if the bits form
    /// a valid bit pattern for the type.
    ///
    /// Consider using [`from_bits`][BitFlag::from_bits]
    /// or [`from_bits_truncate`][BitFlag::from_bits_truncate] instead.
    ///
    /// # Safety
    ///
    /// All bits set in `val` must correspond to a value of the enum.
    ///
    /// # Example 
    ///
    /// This is a convenience reexport of [`BitFlags::from_bits_unchecked`]. It can be
    /// called with `MyFlag::from_bits_unchecked(bits)`, thus bypassing the need for
    /// type hints in some situations.
    ///
    /// ```
    /// # use enumflags2::{bitflags, BitFlags};
    /// #[bitflags]
    /// #[repr(u8)]
    /// #[derive(Clone, Copy, PartialEq, Eq)]
    /// enum MyFlag {
    ///     One = 1 << 0,
    ///     Two = 1 << 1,
    ///     Three = 1 << 2,
    /// }
    ///
    /// use enumflags2::BitFlag;
    ///
    /// let flags = unsafe {
    ///     MyFlag::from_bits_unchecked(0b011)
    /// };
    ///
    /// assert_eq!(flags.contains(MyFlag::One), true);
    /// assert_eq!(flags.contains(MyFlag::Two), true);
    /// assert_eq!(flags.contains(MyFlag::Three), false);
    /// ```
    #[inline]
    unsafe fn from_bits_unchecked(bits: Self::Numeric) -> BitFlags<Self> {
        BitFlags::from_bits_unchecked(bits)
    }
}

/// While the module is public, this is only the case because it needs to be
/// accessed by the macro. Do not use this directly. Stability guarantees
/// don't apply.
#[doc(hidden)]
pub mod _internal {
    /// A trait automatically implemented by `#[bitflags]` to make the enum
    /// a valid type parameter for `BitFlags<T>`.
    ///
    /// # Safety
    ///
    /// The values should reflect reality, like they do if the implementation
    /// is generated by the procmacro.
    ///
    /// `bits` must return the same value as
    /// [`transmute_copy`][std::mem::transmute_copy].
    ///
    /// Representations for all values of `T` must have exactly one bit set.
    pub unsafe trait RawBitFlags: Copy + Clone + 'static {
        /// The underlying integer type.
        type Numeric: BitFlagNum;

        /// A value with no bits set.
        const EMPTY: Self::Numeric;

        /// The value used by the Default implementation. Equivalent to EMPTY, unless
        /// customized.
        const DEFAULT: Self::Numeric;

        /// A value with all flag bits set.
        const ALL_BITS: Self::Numeric;

        /// The name of the type for debug formatting purposes.
        ///
        /// This is typically `BitFlags<EnumName>`
        const BITFLAGS_TYPE_NAME: &'static str;

        /// Return the bits as a number type.
        fn bits(self) -> Self::Numeric;
    }

    use ::core::fmt;
    use ::core::ops::{BitAnd, BitOr, BitXor, Not, Sub};
    use ::core::hash::Hash;

    pub trait BitFlagNum:
        Default
        + BitOr<Self, Output = Self>
        + BitAnd<Self, Output = Self>
        + BitXor<Self, Output = Self>
        + Sub<Self, Output = Self>
        + Not<Output = Self>
        + PartialOrd<Self>
        + Ord
        + Hash
        + fmt::Debug
        + fmt::Binary
        + Copy
        + Clone
    {
        const ONE: Self;

        fn is_power_of_two(self) -> bool;
        fn count_ones(self) -> u32;
        fn wrapping_neg(self) -> Self;
    }

    for_each_uint! { $ty $hide_docs =>
        impl BitFlagNum for $ty {
            const ONE: Self = 1;

            fn is_power_of_two(self) -> bool {
                <$ty>::is_power_of_two(self)
            }

            fn count_ones(self) -> u32 {
                <$ty>::count_ones(self)
            }

            fn wrapping_neg(self) -> Self {
                <$ty>::wrapping_neg(self)
            }
        }
    }

    // Re-export libcore so the macro doesn't inject "extern crate" downstream.
    pub mod core {
        pub use core::{convert, ops, option};
    }

    pub struct AssertionSucceeded;
    pub struct AssertionFailed;
    pub trait ExactlyOneBitSet {
        type X;
    }
    impl ExactlyOneBitSet for AssertionSucceeded {
        type X = ();
    }

    pub trait AssertionHelper {
        type Status;
    }

    impl AssertionHelper for [(); 1] {
        type Status = AssertionSucceeded;
    }

    impl AssertionHelper for [(); 0] {
        type Status = AssertionFailed;
    }

    pub const fn next_bit(x: u128) -> u128 {
        1 << x.trailing_ones()
    }
}

use _internal::BitFlagNum;

// Internal debug formatting implementations
mod formatting;

// impl TryFrom<T::Numeric> for BitFlags<T>
mod fallible;
pub use crate::fallible::FromBitsError;

mod iter;
pub use crate::iter::Iter;

mod const_api;
pub use crate::const_api::ConstToken;

/// Represents a set of flags of some type `T`.
/// `T` must have the `#[bitflags]` attribute applied.
///
/// A `BitFlags<T>` is as large as the `T` itself,
/// and stores one flag per bit.
///
/// ## Comparison operators, [`PartialOrd`] and [`Ord`]
///
/// To make it possible to use `BitFlags` as the key of a
/// [`BTreeMap`][std::collections::BTreeMap], `BitFlags` implements
/// [`Ord`]. There is no meaningful total order for bitflags,
/// so the implementation simply compares the integer values of the bits.
///
/// Unfortunately, this means that comparing `BitFlags` with an operator
/// like `<=` will compile, and return values that are probably useless
/// and not what you expect. In particular, `<=` does *not* check whether
/// one value is a subset of the other. Use [`BitFlags::contains`] for that.
///
/// ## Customizing `Default`
///
/// By default, creating an instance of `BitFlags<T>` with `Default` will result
/// in an empty set. If that's undesirable, you may customize this:
///
/// ```
/// # use enumflags2::{BitFlags, bitflags};
/// #[bitflags(default = B | C)]
/// #[repr(u8)]
/// #[derive(Copy, Clone, Debug, PartialEq)]
/// enum MyFlag {
///     A = 0b0001,
///     B = 0b0010,
///     C = 0b0100,
///     D = 0b1000,
/// }
///
/// assert_eq!(BitFlags::default(), MyFlag::B | MyFlag::C);
/// ```
///
/// ## Memory layout
///
/// `BitFlags<T>` is marked with the `#[repr(transparent)]` trait, meaning
/// it can be safely transmuted into the corresponding numeric type.
///
/// Usually, the same can be achieved by using [`BitFlags::bits`] in one
/// direction, and [`BitFlags::from_bits`], [`BitFlags::from_bits_truncate`],
/// or [`BitFlags::from_bits_unchecked`] in the other direction. However,
/// transmuting might still be useful if, for example, you're dealing with
/// an entire array of `BitFlags`.
///
/// When transmuting *into* a `BitFlags`, make sure that each set bit
/// corresponds to an existing flag
/// (cf. [`from_bits_unchecked`][BitFlags::from_bits_unchecked]).
///
/// For example:
///
/// ```
/// # use enumflags2::{BitFlags, bitflags};
/// #[bitflags]
/// #[repr(u8)] // <-- the repr determines the numeric type
/// #[derive(Copy, Clone)]
/// enum TransmuteMe {
///     One = 1 << 0,
///     Two = 1 << 1,
/// }
///
/// # use std::slice;
/// // NOTE: we use a small, self-contained function to handle the slice
/// // conversion to make sure the lifetimes are right.
/// fn transmute_slice<'a>(input: &'a [BitFlags<TransmuteMe>]) -> &'a [u8] {
///     unsafe {
///         slice::from_raw_parts(input.as_ptr() as *const u8, input.len())
///     }
/// }
///
/// let many_flags = &[
///     TransmuteMe::One.into(),
///     TransmuteMe::One | TransmuteMe::Two,
/// ];
///
/// let as_nums = transmute_slice(many_flags);
/// assert_eq!(as_nums, &[0b01, 0b11]);
/// ```
///
/// ## Implementation notes
///
/// You might expect this struct to be defined as
///
/// ```ignore
/// struct BitFlags<T: BitFlag> {
///     value: T::Numeric
/// }
/// ```
///
/// Ideally, that would be the case. However, because `const fn`s cannot
/// have trait bounds in current Rust, this would prevent us from providing
/// most `const fn` APIs. As a workaround, we define `BitFlags` with two
/// type parameters, with a default for the second one:
///
/// ```ignore
/// struct BitFlags<T, N = <T as BitFlag>::Numeric> {
///     value: N,
///     marker: PhantomData<T>,
/// }
/// ```
///
/// Manually providing a type for the `N` type parameter shouldn't ever
/// be necessary.
///
/// The types substituted for `T` and `N` must always match, creating a
/// `BitFlags` value where that isn't the case is only possible with
/// incorrect unsafe code.
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct BitFlags<T, N = <T as _internal::RawBitFlags>::Numeric> {
    val: N,
    marker: PhantomData<T>,
}

/// `make_bitflags!` provides a succint syntax for creating instances of
/// `BitFlags<T>`. Instead of repeating the name of your type for each flag
/// you want to add, try `make_bitflags!(Flags::{Foo | Bar})`.
/// ```
/// # use enumflags2::{bitflags, BitFlags, make_bitflags};
/// # #[bitflags]
/// # #[repr(u8)]
/// # #[derive(Clone, Copy, Debug)]
/// # enum Test {
/// #     A = 1 << 0,
/// #     B = 1 << 1,
/// #     C = 1 << 2,
/// # }
/// let x = make_bitflags!(Test::{A | C});
/// assert_eq!(x, Test::A | Test::C);
///
/// // Also works in const contexts:
/// const X: BitFlags<Test> = make_bitflags!(Test::A);
/// ```
#[macro_export]
macro_rules! make_bitflags {
    ( $enum:ident ::{ $($variant:ident)|* } ) => {
        {
            let mut n = 0;
            $(
                {
                    let flag: $enum = $enum::$variant;
                    n |= flag as <$enum as $crate::_internal::RawBitFlags>::Numeric;
                }
            )*
            // SAFETY: The value has been created from numeric values of the underlying
            // enum, so only valid bits are set.
            unsafe { $crate::BitFlags::<$enum>::from_bits_unchecked_c(
                    n, $crate::BitFlags::CONST_TOKEN) }
        }
    };
    ( $enum:ident :: $variant:ident ) => {
        {
            let flag: $enum = $enum::$variant;
            let n = flag as <$enum as $crate::_internal::RawBitFlags>::Numeric;
            // SAFETY: The value has been created from the numeric value of
            // the underlying enum, so only valid bits are set.
            unsafe { $crate::BitFlags::<$enum>::from_bits_unchecked_c(
                    n, $crate::BitFlags::CONST_TOKEN) }
        }
    };
}

/// The default value returned is one with all flags unset, i. e. [`empty`][Self::empty],
/// unless [customized](index.html#customizing-default).
impl<T> Default for BitFlags<T>
where
    T: BitFlag,
{
    #[inline(always)]
    fn default() -> Self {
        BitFlags {
            val: T::DEFAULT,
            marker: PhantomData,
        }
    }
}

impl<T: BitFlag> From<T> for BitFlags<T> {
    #[inline(always)]
    fn from(t: T) -> BitFlags<T> {
        Self::from_flag(t)
    }
}

impl<T> BitFlags<T>
where
    T: BitFlag,
{
    /// Create a `BitFlags` if the raw value provided does not contain
    /// any illegal flags.
    ///
    /// See also: [a convenience re-export in the `BitFlag` trait][BitFlag::from_bits],
    /// which can help avoid the need for type hints.
    ///
    /// ```
    /// # use enumflags2::{bitflags, BitFlags};
    /// #[bitflags]
    /// #[repr(u8)]
    /// #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    /// enum MyFlag {
    ///     One = 1 << 0,
    ///     Two = 1 << 1,
    ///     Three = 1 << 2,
    /// }
    ///
    /// let flags: BitFlags<MyFlag> = BitFlags::from_bits(0b11).unwrap();
    /// assert_eq!(flags.contains(MyFlag::One), true);
    /// assert_eq!(flags.contains(MyFlag::Two), true);
    /// assert_eq!(flags.contains(MyFlag::Three), false);
    /// let invalid = BitFlags::<MyFlag>::from_bits(1 << 3);
    /// assert!(invalid.is_err());
    /// ```
    #[inline]
    pub fn from_bits(bits: T::Numeric) -> Result<Self, FromBitsError<T>> {
        let flags = Self::from_bits_truncate(bits);
        if flags.bits() == bits {
            Ok(flags)
        } else {
            Err(FromBitsError {
                flags,
                invalid: bits & !flags.bits(),
            })
        }
    }

    /// Create a `BitFlags` from an underlying bitwise value. If any
    /// invalid bits are set, ignore them.
    ///
    /// See also: [a convenience re-export in the `BitFlag` trait][BitFlag::from_bits_truncate],
    /// which can help avoid the need for type hints.
    ///
    /// ```
    /// # use enumflags2::{bitflags, BitFlags};
    /// #[bitflags]
    /// #[repr(u8)]
    /// #[derive(Clone, Copy, PartialEq, Eq)]
    /// enum MyFlag {
    ///     One = 1 << 0,
    ///     Two = 1 << 1,
    ///     Three = 1 << 2,
    /// }
    ///
    /// let flags: BitFlags<MyFlag> = BitFlags::from_bits_truncate(0b1_1011);
    /// assert_eq!(flags.contains(MyFlag::One), true);
    /// assert_eq!(flags.contains(MyFlag::Two), true);
    /// assert_eq!(flags.contains(MyFlag::Three), false);
    /// ```
    #[must_use]
    #[inline(always)]
    pub fn from_bits_truncate(bits: T::Numeric) -> Self {
        // SAFETY: We're truncating out all the invalid bits, so the remaining
        // ones must be valid.
        unsafe { BitFlags::from_bits_unchecked(bits & T::ALL_BITS) }
    }

    /// Create a new BitFlags unsafely, without checking if the bits form
    /// a valid bit pattern for the type.
    ///
    /// Consider using [`from_bits`][BitFlags::from_bits]
    /// or [`from_bits_truncate`][BitFlags::from_bits_truncate] instead.
    ///
    /// # Safety
    ///
    /// All bits set in `val` must correspond to a value of the enum.
    ///
    /// # Example
    ///
    /// ```
    /// # use enumflags2::{bitflags, BitFlags};
    /// #[bitflags]
    /// #[repr(u8)]
    /// #[derive(Clone, Copy, PartialEq, Eq)]
    /// enum MyFlag {
    ///     One = 1 << 0,
    ///     Two = 1 << 1,
    ///     Three = 1 << 2,
    /// }
    ///
    /// let flags: BitFlags<MyFlag> = unsafe {
    ///     BitFlags::from_bits_unchecked(0b011)
    /// };
    ///
    /// assert_eq!(flags.contains(MyFlag::One), true);
    /// assert_eq!(flags.contains(MyFlag::Two), true);
    /// assert_eq!(flags.contains(MyFlag::Three), false);
    /// ```
    #[must_use]
    #[inline(always)]
    pub unsafe fn from_bits_unchecked(val: T::Numeric) -> Self {
        BitFlags {
            val,
            marker: PhantomData,
        }
    }

    /// Turn a `T` into a `BitFlags<T>`. Also available as `flag.into()`.
    #[must_use]
    #[inline(always)]
    pub fn from_flag(flag: T) -> Self {
        // SAFETY: A value of the underlying enum is valid by definition.
        unsafe { Self::from_bits_unchecked(flag.bits()) }
    }

    /// Create a `BitFlags` with no flags set (in other words, with a value of `0`).
    ///
    /// See also: [`BitFlag::empty`], a convenience reexport;
    /// [`BitFlags::EMPTY`], the same functionality available
    /// as a constant for `const fn` code.
    ///
    /// ```
    /// # use enumflags2::{bitflags, BitFlags};
    /// #[bitflags]
    /// #[repr(u8)]
    /// #[derive(Clone, Copy, PartialEq, Eq)]
    /// enum MyFlag {
    ///     One = 1 << 0,
    ///     Two = 1 << 1,
    ///     Three = 1 << 2,
    /// }
    ///
    /// let empty: BitFlags<MyFlag> = BitFlags::empty();
    /// assert!(empty.is_empty());
    /// assert_eq!(empty.contains(MyFlag::One), false);
    /// assert_eq!(empty.contains(MyFlag::Two), false);
    /// assert_eq!(empty.contains(MyFlag::Three), false);
    /// ```
    #[inline(always)]
    pub fn empty() -> Self {
        Self::EMPTY
    }

    /// Create a `BitFlags` with all flags set.
    ///
    /// See also: [`BitFlag::all`], a convenience reexport;
    /// [`BitFlags::ALL`], the same functionality available
    /// as a constant for `const fn` code.
    ///
    /// ```
    /// # use enumflags2::{bitflags, BitFlags};
    /// #[bitflags]
    /// #[repr(u8)]
    /// #[derive(Clone, Copy, PartialEq, Eq)]
    /// enum MyFlag {
    ///     One = 1 << 0,
    ///     Two = 1 << 1,
    ///     Three = 1 << 2,
    /// }
    ///
    /// let empty: BitFlags<MyFlag> = BitFlags::all();
    /// assert!(empty.is_all());
    /// assert_eq!(empty.contains(MyFlag::One), true);
    /// assert_eq!(empty.contains(MyFlag::Two), true);
    /// assert_eq!(empty.contains(MyFlag::Three), true);
    /// ```
    #[inline(always)]
    pub fn all() -> Self {
        Self::ALL
    }

    /// Returns true if all flags are set
    #[inline(always)]
    pub fn is_all(self) -> bool {
        self.val == T::ALL_BITS
    }

    /// Returns true if no flag is set
    #[inline(always)]
    pub fn is_empty(self) -> bool {
        self.val == T::EMPTY
    }

    /// Returns the number of flags set.
    #[inline(always)]
    pub fn len(self) -> usize {
        self.val.count_ones() as usize
    }

    /// If exactly one flag is set, the flag is returned. Otherwise, returns `None`.
    ///
    /// See also [`Itertools::exactly_one`](https://docs.rs/itertools/latest/itertools/trait.Itertools.html#method.exactly_one).
    #[inline(always)]
    pub fn exactly_one(self) -> Option<T> {
        if self.val.is_power_of_two() {
            // SAFETY: By the invariant of the BitFlags type, all bits are valid
            // in isolation for the underlying enum.
            Some(unsafe { core::mem::transmute_copy(&self.val) })
        } else {
            None
        }
    }

    /// Returns the underlying bitwise value.
    ///
    /// ```
    /// # use enumflags2::{bitflags, BitFlags};
    /// #[bitflags]
    /// #[repr(u8)]
    /// #[derive(Clone, Copy)]
    /// enum Flags {
    ///     Foo = 1 << 0,
    ///     Bar = 1 << 1,
    /// }
    ///
    /// let both_flags = Flags::Foo | Flags::Bar;
    /// assert_eq!(both_flags.bits(), 0b11);
    /// ```
    #[inline(always)]
    pub fn bits(self) -> T::Numeric {
        self.val
    }

    /// Returns true if at least one flag is shared.
    #[inline(always)]
    pub fn intersects<B: Into<BitFlags<T>>>(self, other: B) -> bool {
        (self.bits() & other.into().bits()) != Self::EMPTY.val
    }

    /// Returns true if all flags are contained.
    #[inline(always)]
    pub fn contains<B: Into<BitFlags<T>>>(self, other: B) -> bool {
        let other = other.into();
        (self.bits() & other.bits()) == other.bits()
    }

    /// Toggles the matching bits
    #[inline(always)]
    pub fn toggle<B: Into<BitFlags<T>>>(&mut self, other: B) {
        *self ^= other.into();
    }

    /// Inserts the flags into the BitFlag
    #[inline(always)]
    pub fn insert<B: Into<BitFlags<T>>>(&mut self, other: B) {
        *self |= other.into();
    }

    /// Removes the matching flags
    #[inline(always)]
    pub fn remove<B: Into<BitFlags<T>>>(&mut self, other: B) {
        *self &= !other.into();
    }

    /// Inserts if `cond` holds, else removes
    ///
    /// ```
    /// # use enumflags2::bitflags;
    /// #[bitflags]
    /// #[derive(Clone, Copy, PartialEq, Debug)]
    /// #[repr(u8)]
    /// enum MyFlag {
    ///     A = 1 << 0,
    ///     B = 1 << 1,
    ///     C = 1 << 2,
    /// }
    ///
    /// let mut state = MyFlag::A | MyFlag::C;
    /// state.set(MyFlag::A | MyFlag::B, false);
    ///
    /// // Because the condition was false, both
    /// // `A` and `B` are removed from the set
    /// assert_eq!(state, MyFlag::C);
    /// ```
    #[inline(always)]
    pub fn set<B: Into<BitFlags<T>>>(&mut self, other: B, cond: bool) {
        if cond {
            self.insert(other);
        } else {
            self.remove(other);
        }
    }
}

impl<T, N: PartialEq> PartialEq for BitFlags<T, N> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.val == other.val
    }
}

impl<T, N: Eq> Eq for BitFlags<T, N> {}

impl<T, N: PartialOrd> PartialOrd for BitFlags<T, N> {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.val.partial_cmp(&other.val)
    }
}

impl<T, N: Ord> Ord for BitFlags<T, N> {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.val.cmp(&other.val)
    }
}

// Clippy complains when Hash is derived while PartialEq is implemented manually
impl<T, N: Hash> Hash for BitFlags<T, N> {
    #[inline(always)]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.val.hash(state)
    }
}

impl<T> cmp::PartialEq<T> for BitFlags<T>
where
    T: BitFlag,
{
    #[inline(always)]
    fn eq(&self, other: &T) -> bool {
        self.bits() == Into::<Self>::into(*other).bits()
    }
}

impl<T, B> ops::BitOr<B> for BitFlags<T>
where
    T: BitFlag,
    B: Into<BitFlags<T>>,
{
    type Output = BitFlags<T>;
    #[inline(always)]
    fn bitor(self, other: B) -> BitFlags<T> {
        // SAFETY: The two operands are known to be composed of valid bits,
        // and 0 | 0 = 0 in the columns of the invalid bits.
        unsafe { BitFlags::from_bits_unchecked(self.bits() | other.into().bits()) }
    }
}

impl<T, B> ops::BitAnd<B> for BitFlags<T>
where
    T: BitFlag,
    B: Into<BitFlags<T>>,
{
    type Output = BitFlags<T>;
    #[inline(always)]
    fn bitand(self, other: B) -> BitFlags<T> {
        // SAFETY: The two operands are known to be composed of valid bits,
        // and 0 & 0 = 0 in the columns of the invalid bits.
        unsafe { BitFlags::from_bits_unchecked(self.bits() & other.into().bits()) }
    }
}

impl<T, B> ops::BitXor<B> for BitFlags<T>
where
    T: BitFlag,
    B: Into<BitFlags<T>>,
{
    type Output = BitFlags<T>;
    #[inline(always)]
    fn bitxor(self, other: B) -> BitFlags<T> {
        // SAFETY: The two operands are known to be composed of valid bits,
        // and 0 ^ 0 = 0 in the columns of the invalid bits.
        unsafe { BitFlags::from_bits_unchecked(self.bits() ^ other.into().bits()) }
    }
}

impl<T, B> ops::BitOrAssign<B> for BitFlags<T>
where
    T: BitFlag,
    B: Into<BitFlags<T>>,
{
    #[inline(always)]
    fn bitor_assign(&mut self, other: B) {
        *self = *self | other;
    }
}

impl<T, B> ops::BitAndAssign<B> for BitFlags<T>
where
    T: BitFlag,
    B: Into<BitFlags<T>>,
{
    #[inline(always)]
    fn bitand_assign(&mut self, other: B) {
        *self = *self & other;
    }
}
impl<T, B> ops::BitXorAssign<B> for BitFlags<T>
where
    T: BitFlag,
    B: Into<BitFlags<T>>,
{
    #[inline(always)]
    fn bitxor_assign(&mut self, other: B) {
        *self = *self ^ other;
    }
}

impl<T> ops::Not for BitFlags<T>
where
    T: BitFlag,
{
    type Output = BitFlags<T>;
    #[inline(always)]
    fn not(self) -> BitFlags<T> {
        BitFlags::from_bits_truncate(!self.bits())
    }
}

#[cfg(feature = "serde")]
mod impl_serde {
    use super::{BitFlag, BitFlags};
    use serde::de::{Error, Unexpected};
    use serde::{Deserialize, Serialize};

    impl<'a, T> Deserialize<'a> for BitFlags<T>
    where
        T: BitFlag,
        T::Numeric: Deserialize<'a> + Into<u64>,
    {
        fn deserialize<D: serde::Deserializer<'a>>(d: D) -> Result<Self, D::Error> {
            let val = T::Numeric::deserialize(d)?;
            Self::from_bits(val).map_err(|_| {
                D::Error::invalid_value(
                    Unexpected::Unsigned(val.into()),
                    &"valid bit representation",
                )
            })
        }
    }

    impl<T> Serialize for BitFlags<T>
    where
        T: BitFlag,
        T::Numeric: Serialize,
    {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            T::Numeric::serialize(&self.val, s)
        }
    }
}
