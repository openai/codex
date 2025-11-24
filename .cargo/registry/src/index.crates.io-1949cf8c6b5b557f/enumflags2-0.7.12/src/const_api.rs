use crate::{BitFlags, BitFlag};
use core::marker::PhantomData;

/// Workaround for `const fn` limitations.
///
/// Some `const fn`s in this crate will need an instance of this type
/// for some type-level information usually provided by traits.
///
/// A token can be obtained from [`BitFlags::CONST_TOKEN`]. The relevant types
/// should be readily inferred from context.
///
/// For an example of usage, see [`not_c`][BitFlags::not_c].
pub struct ConstToken<T, N>(BitFlags<T, N>);

impl<T> BitFlags<T>
where
    T: BitFlag,
{
    /// An empty `BitFlags`. Equivalent to [`empty()`][BitFlags::empty],
    /// but works in a const context.
    pub const EMPTY: Self = BitFlags {
        val: T::EMPTY,
        marker: PhantomData,
    };

    /// A `BitFlags` with all flags set. Equivalent to [`all()`][BitFlags::all],
    /// but works in a const context.
    pub const ALL: Self = BitFlags {
        val: T::ALL_BITS,
        marker: PhantomData,
    };

    /// A [`ConstToken`] for this type of flag.
    pub const CONST_TOKEN: ConstToken<T, T::Numeric> = ConstToken(Self::ALL);
}

for_each_uint! { $ty $hide_docs =>
    impl<T> BitFlags<T, $ty> {
        /// Create a new BitFlags unsafely, without checking if the bits form
        /// a valid bit pattern for the type.
        ///
        /// Const variant of
        /// [`from_bits_unchecked`][BitFlags::from_bits_unchecked].
        ///
        /// Consider using
        /// [`from_bits_truncate_c`][BitFlags::from_bits_truncate_c] instead.
        ///
        /// # Safety
        ///
        /// All bits set in `val` must correspond to a value of the enum.
        #[must_use]
        #[inline(always)]
        $(#[$hide_docs])?
        pub const unsafe fn from_bits_unchecked_c(
            val: $ty, const_token: ConstToken<T, $ty>
        ) -> Self {
            let _ = const_token;
            BitFlags {
                val,
                marker: PhantomData,
            }
        }

        /// Create a `BitFlags<T>` from an underlying bitwise value. If any
        /// invalid bits are set, ignore them.
        ///
        /// ```
        /// # use enumflags2::{bitflags, BitFlags};
        /// #[bitflags]
        /// #[repr(u8)]
        /// #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        /// enum MyFlag {
        ///     One = 1 << 0,
        ///     Two = 1 << 1,
        ///     Three = 1 << 2,
        /// }
        ///
        /// const FLAGS: BitFlags<MyFlag> =
        ///     BitFlags::<MyFlag>::from_bits_truncate_c(0b10101010, BitFlags::CONST_TOKEN);
        /// assert_eq!(FLAGS, MyFlag::Two);
        /// ```
        #[must_use]
        #[inline(always)]
        $(#[$hide_docs])?
        pub const fn from_bits_truncate_c(
            bits: $ty, const_token: ConstToken<T, $ty>
        ) -> Self {
            BitFlags {
                val: bits & const_token.0.val,
                marker: PhantomData,
            }
        }

        /// Bitwise OR — return value contains flag if either argument does.
        ///
        /// Also available as `a | b`, but operator overloads are not usable
        /// in `const fn`s at the moment.
        #[must_use]
        #[inline(always)]
        $(#[$hide_docs])?
        pub const fn union_c(self, other: Self) -> Self {
            BitFlags {
                val: self.val | other.val,
                marker: PhantomData,
            }
        }

        /// Bitwise AND — return value contains flag if both arguments do.
        ///
        /// Also available as `a & b`, but operator overloads are not usable
        /// in `const fn`s at the moment.
        #[must_use]
        #[inline(always)]
        $(#[$hide_docs])?
        pub const fn intersection_c(self, other: Self) -> Self {
            BitFlags {
                val: self.val & other.val,
                marker: PhantomData,
            }
        }

        /// Bitwise NOT — return value contains flag if argument doesn't.
        ///
        /// Also available as `!a`, but operator overloads are not usable
        /// in `const fn`s at the moment.
        ///
        /// Moreover, due to `const fn` limitations, `not_c` needs a
        /// [`ConstToken`] as an argument.
        ///
        /// ```
        /// # use enumflags2::{bitflags, BitFlags, make_bitflags};
        /// #[bitflags]
        /// #[repr(u8)]
        /// #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        /// enum MyFlag {
        ///     One = 1 << 0,
        ///     Two = 1 << 1,
        ///     Three = 1 << 2,
        /// }
        ///
        /// const FLAGS: BitFlags<MyFlag> = make_bitflags!(MyFlag::{One | Two});
        /// const NEGATED: BitFlags<MyFlag> = FLAGS.not_c(BitFlags::CONST_TOKEN);
        /// assert_eq!(NEGATED, MyFlag::Three);
        /// ```
        #[must_use]
        #[inline(always)]
        $(#[$hide_docs])?
        pub const fn not_c(self, const_token: ConstToken<T, $ty>) -> Self {
            BitFlags {
                val: !self.val & const_token.0.val,
                marker: PhantomData,
            }
        }

        /// Returns the underlying bitwise value.
        ///
        /// `const` variant of [`bits`][BitFlags::bits].
        #[inline(always)]
        $(#[$hide_docs])?
        pub const fn bits_c(self) -> $ty {
            self.val
        }
    }
}
