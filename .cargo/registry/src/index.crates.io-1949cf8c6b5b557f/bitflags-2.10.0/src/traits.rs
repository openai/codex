use core::{
    fmt,
    ops::{BitAnd, BitOr, BitXor, Not},
};

use crate::{
    iter,
    parser::{ParseError, ParseHex, WriteHex},
};

/**
A defined flags value that may be named or unnamed.
*/
#[derive(Debug)]
pub struct Flag<B> {
    name: &'static str,
    value: B,
}

impl<B> Flag<B> {
    /**
    Define a flag.

    If `name` is non-empty then the flag is named, otherwise it's unnamed.
    */
    pub const fn new(name: &'static str, value: B) -> Self {
        Flag { name, value }
    }

    /**
    Get the name of this flag.

    If the flag is unnamed then the returned string will be empty.
    */
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /**
    Get the flags value of this flag.
    */
    pub const fn value(&self) -> &B {
        &self.value
    }

    /**
    Whether the flag is named.

    If [`Flag::name`] returns a non-empty string then this method will return `true`.
    */
    pub const fn is_named(&self) -> bool {
        !self.name.is_empty()
    }

    /**
    Whether the flag is unnamed.

    If [`Flag::name`] returns a non-empty string then this method will return `false`.
    */
    pub const fn is_unnamed(&self) -> bool {
        self.name.is_empty()
    }
}

/**
A set of defined flags using a bits type as storage.

## Implementing `Flags`

This trait is implemented by the [`bitflags`](macro.bitflags.html) macro:

```
use bitflags::bitflags;

bitflags! {
    struct MyFlags: u8 {
        const A = 1;
        const B = 1 << 1;
    }
}
```

It can also be implemented manually:

```
use bitflags::{Flag, Flags};

struct MyFlags(u8);

impl Flags for MyFlags {
    const FLAGS: &'static [Flag<Self>] = &[
        Flag::new("A", MyFlags(1)),
        Flag::new("B", MyFlags(1 << 1)),
    ];

    type Bits = u8;

    fn from_bits_retain(bits: Self::Bits) -> Self {
        MyFlags(bits)
    }

    fn bits(&self) -> Self::Bits {
        self.0
    }
}
```

## Using `Flags`

The `Flags` trait can be used generically to work with any flags types. In this example,
we can count the number of defined named flags:

```
# use bitflags::{bitflags, Flags};
fn defined_flags<F: Flags>() -> usize {
    F::FLAGS.iter().filter(|f| f.is_named()).count()
}

bitflags! {
    struct MyFlags: u8 {
        const A = 1;
        const B = 1 << 1;
        const C = 1 << 2;

        const _ = !0;
    }
}

assert_eq!(3, defined_flags::<MyFlags>());
```
*/
pub trait Flags: Sized + 'static {
    /// The set of defined flags.
    const FLAGS: &'static [Flag<Self>];

    /// The underlying bits type.
    type Bits: Bits;

    /// Get a flags value with all bits unset.
    fn empty() -> Self {
        Self::from_bits_retain(Self::Bits::EMPTY)
    }

    /// Get a flags value with all known bits set.
    fn all() -> Self {
        let mut truncated = Self::Bits::EMPTY;

        for flag in Self::FLAGS.iter() {
            truncated = truncated | flag.value().bits();
        }

        Self::from_bits_retain(truncated)
    }

    /// This method will return `true` if any unknown bits are set.
    fn contains_unknown_bits(&self) -> bool {
        Self::all().bits() & self.bits() != self.bits()
    }

    /// Get the underlying bits value.
    ///
    /// The returned value is exactly the bits set in this flags value.
    fn bits(&self) -> Self::Bits;

    /// Convert from a bits value.
    ///
    /// This method will return `None` if any unknown bits are set.
    fn from_bits(bits: Self::Bits) -> Option<Self> {
        let truncated = Self::from_bits_truncate(bits);

        if truncated.bits() == bits {
            Some(truncated)
        } else {
            None
        }
    }

    /// Convert from a bits value, unsetting any unknown bits.
    fn from_bits_truncate(bits: Self::Bits) -> Self {
        Self::from_bits_retain(bits & Self::all().bits())
    }

    /// Convert from a bits value exactly.
    fn from_bits_retain(bits: Self::Bits) -> Self;

    /// Get a flags value with the bits of a flag with the given name set.
    ///
    /// This method will return `None` if `name` is empty or doesn't
    /// correspond to any named flag.
    fn from_name(name: &str) -> Option<Self> {
        // Don't parse empty names as empty flags
        if name.is_empty() {
            return None;
        }

        for flag in Self::FLAGS {
            if flag.name() == name {
                return Some(Self::from_bits_retain(flag.value().bits()));
            }
        }

        None
    }

    /// Yield a set of contained flags values.
    ///
    /// Each yielded flags value will correspond to a defined named flag. Any unknown bits
    /// will be yielded together as a final flags value.
    fn iter(&self) -> iter::Iter<Self> {
        iter::Iter::new(self)
    }

    /// Yield a set of contained named flags values.
    ///
    /// This method is like [`Flags::iter`], except only yields bits in contained named flags.
    /// Any unknown bits, or bits not corresponding to a contained flag will not be yielded.
    fn iter_names(&self) -> iter::IterNames<Self> {
        iter::IterNames::new(self)
    }

    /// Yield a set of all named flags defined by [`Self::FLAGS`].
    fn iter_defined_names() -> iter::IterDefinedNames<Self> {
        iter::IterDefinedNames::new()
    }

    /// Whether all bits in this flags value are unset.
    fn is_empty(&self) -> bool {
        self.bits() == Self::Bits::EMPTY
    }

    /// Whether all known bits in this flags value are set.
    fn is_all(&self) -> bool {
        // NOTE: We check against `Self::all` here, not `Self::Bits::ALL`
        // because the set of all flags may not use all bits
        Self::all().bits() | self.bits() == self.bits()
    }

    /// Whether any set bits in a source flags value are also set in a target flags value.
    fn intersects(&self, other: Self) -> bool
    where
        Self: Sized,
    {
        self.bits() & other.bits() != Self::Bits::EMPTY
    }

    /// Whether all set bits in a source flags value are also set in a target flags value.
    fn contains(&self, other: Self) -> bool
    where
        Self: Sized,
    {
        self.bits() & other.bits() == other.bits()
    }

    /// Remove any unknown bits from the flags.
    fn truncate(&mut self)
    where
        Self: Sized,
    {
        *self = Self::from_bits_truncate(self.bits());
    }

    /// The bitwise or (`|`) of the bits in two flags values.
    fn insert(&mut self, other: Self)
    where
        Self: Sized,
    {
        *self = Self::from_bits_retain(self.bits()).union(other);
    }

    /// The intersection of a source flags value with the complement of a target flags value (`&!`).
    ///
    /// This method is not equivalent to `self & !other` when `other` has unknown bits set.
    /// `remove` won't truncate `other`, but the `!` operator will.
    fn remove(&mut self, other: Self)
    where
        Self: Sized,
    {
        *self = Self::from_bits_retain(self.bits()).difference(other);
    }

    /// The bitwise exclusive-or (`^`) of the bits in two flags values.
    fn toggle(&mut self, other: Self)
    where
        Self: Sized,
    {
        *self = Self::from_bits_retain(self.bits()).symmetric_difference(other);
    }

    /// Call [`Flags::insert`] when `value` is `true` or [`Flags::remove`] when `value` is `false`.
    fn set(&mut self, other: Self, value: bool)
    where
        Self: Sized,
    {
        if value {
            self.insert(other);
        } else {
            self.remove(other);
        }
    }

    /// Unsets all bits in the flags.
    fn clear(&mut self)
    where
        Self: Sized,
    {
        *self = Self::empty();
    }

    /// The bitwise and (`&`) of the bits in two flags values.
    #[must_use]
    fn intersection(self, other: Self) -> Self {
        Self::from_bits_retain(self.bits() & other.bits())
    }

    /// The bitwise or (`|`) of the bits in two flags values.
    #[must_use]
    fn union(self, other: Self) -> Self {
        Self::from_bits_retain(self.bits() | other.bits())
    }

    /// The intersection of a source flags value with the complement of a target flags value (`&!`).
    ///
    /// This method is not equivalent to `self & !other` when `other` has unknown bits set.
    /// `difference` won't truncate `other`, but the `!` operator will.
    #[must_use]
    fn difference(self, other: Self) -> Self {
        Self::from_bits_retain(self.bits() & !other.bits())
    }

    /// The bitwise exclusive-or (`^`) of the bits in two flags values.
    #[must_use]
    fn symmetric_difference(self, other: Self) -> Self {
        Self::from_bits_retain(self.bits() ^ other.bits())
    }

    /// The bitwise negation (`!`) of the bits in a flags value, truncating the result.
    #[must_use]
    fn complement(self) -> Self {
        Self::from_bits_truncate(!self.bits())
    }
}

/**
A bits type that can be used as storage for a flags type.
*/
pub trait Bits:
    Clone
    + Copy
    + PartialEq
    + BitAnd<Output = Self>
    + BitOr<Output = Self>
    + BitXor<Output = Self>
    + Not<Output = Self>
    + Sized
    + 'static
{
    /// A value with all bits unset.
    const EMPTY: Self;

    /// A value with all bits set.
    const ALL: Self;
}

// Not re-exported: prevent custom `Bits` impls being used in the `bitflags!` macro,
// or they may fail to compile based on crate features
pub trait Primitive {}

macro_rules! impl_bits {
    ($($u:ty, $i:ty,)*) => {
        $(
            impl Bits for $u {
                const EMPTY: $u = 0;
                const ALL: $u = <$u>::MAX;
            }

            impl Bits for $i {
                const EMPTY: $i = 0;
                const ALL: $i = <$u>::MAX as $i;
            }

            impl ParseHex for $u {
                fn parse_hex(input: &str) -> Result<Self, ParseError> {
                    <$u>::from_str_radix(input, 16).map_err(|_| ParseError::invalid_hex_flag(input))
                }
            }

            impl ParseHex for $i {
                fn parse_hex(input: &str) -> Result<Self, ParseError> {
                    <$i>::from_str_radix(input, 16).map_err(|_| ParseError::invalid_hex_flag(input))
                }
            }

            impl WriteHex for $u {
                fn write_hex<W: fmt::Write>(&self, mut writer: W) -> fmt::Result {
                    write!(writer, "{:x}", self)
                }
            }

            impl WriteHex for $i {
                fn write_hex<W: fmt::Write>(&self, mut writer: W) -> fmt::Result {
                    write!(writer, "{:x}", self)
                }
            }

            impl Primitive for $i {}
            impl Primitive for $u {}
        )*
    }
}

impl_bits! {
    u8, i8,
    u16, i16,
    u32, i32,
    u64, i64,
    u128, i128,
    usize, isize,
}

/// A trait for referencing the `bitflags`-owned internal type
/// without exposing it publicly.
pub trait PublicFlags {
    /// The type of the underlying storage.
    type Primitive: Primitive;

    /// The type of the internal field on the generated flags type.
    type Internal;
}

#[doc(hidden)]
#[deprecated(note = "use the `Flags` trait instead")]
pub trait BitFlags: ImplementedByBitFlagsMacro + Flags {
    /// An iterator over enabled flags in an instance of the type.
    type Iter: Iterator<Item = Self>;

    /// An iterator over the raw names and bits for enabled flags in an instance of the type.
    type IterNames: Iterator<Item = (&'static str, Self)>;
}

#[allow(deprecated)]
impl<B: Flags> BitFlags for B {
    type Iter = iter::Iter<Self>;
    type IterNames = iter::IterNames<Self>;
}

impl<B: Flags> ImplementedByBitFlagsMacro for B {}

/// A marker trait that signals that an implementation of `BitFlags` came from the `bitflags!` macro.
///
/// There's nothing stopping an end-user from implementing this trait, but we don't guarantee their
/// manual implementations won't break between non-breaking releases.
#[doc(hidden)]
pub trait ImplementedByBitFlagsMacro {}

pub(crate) mod __private {
    pub use super::{ImplementedByBitFlagsMacro, PublicFlags};
}
