//! Types for compile-time and run-time endianness.

use crate::pod::Pod;
use core::fmt::{self, Debug};
use core::marker::PhantomData;

/// A trait for using an endianness specification.
///
/// Provides methods for converting between the specified endianness and
/// the native endianness of the target machine.
///
/// This trait does not require that the endianness is known at compile time.
pub trait Endian: Debug + Default + Clone + Copy + PartialEq + Eq + 'static {
    /// Construct a specification for the endianness of some values.
    ///
    /// Returns `None` if the type does not support specifying the given endianness.
    fn from_big_endian(big_endian: bool) -> Option<Self>;

    /// Construct a specification for the endianness of some values.
    ///
    /// Returns `None` if the type does not support specifying the given endianness.
    fn from_little_endian(little_endian: bool) -> Option<Self> {
        Self::from_big_endian(!little_endian)
    }

    /// Return true for big endian byte order.
    fn is_big_endian(self) -> bool;

    /// Return true for little endian byte order.
    #[inline]
    fn is_little_endian(self) -> bool {
        !self.is_big_endian()
    }

    /// Converts an unsigned 16 bit integer to native endian.
    #[inline]
    fn read_u16(self, n: u16) -> u16 {
        if self.is_big_endian() {
            u16::from_be(n)
        } else {
            u16::from_le(n)
        }
    }

    /// Converts an unsigned 32 bit integer to native endian.
    #[inline]
    fn read_u32(self, n: u32) -> u32 {
        if self.is_big_endian() {
            u32::from_be(n)
        } else {
            u32::from_le(n)
        }
    }

    /// Converts an unsigned 64 bit integer to native endian.
    #[inline]
    fn read_u64(self, n: u64) -> u64 {
        if self.is_big_endian() {
            u64::from_be(n)
        } else {
            u64::from_le(n)
        }
    }

    /// Converts a signed 16 bit integer to native endian.
    #[inline]
    fn read_i16(self, n: i16) -> i16 {
        if self.is_big_endian() {
            i16::from_be(n)
        } else {
            i16::from_le(n)
        }
    }

    /// Converts a signed 32 bit integer to native endian.
    #[inline]
    fn read_i32(self, n: i32) -> i32 {
        if self.is_big_endian() {
            i32::from_be(n)
        } else {
            i32::from_le(n)
        }
    }

    /// Converts a signed 64 bit integer to native endian.
    #[inline]
    fn read_i64(self, n: i64) -> i64 {
        if self.is_big_endian() {
            i64::from_be(n)
        } else {
            i64::from_le(n)
        }
    }

    /// Converts an unaligned unsigned 16 bit integer to native endian.
    #[inline]
    fn read_u16_bytes(self, n: [u8; 2]) -> u16 {
        if self.is_big_endian() {
            u16::from_be_bytes(n)
        } else {
            u16::from_le_bytes(n)
        }
    }

    /// Converts an unaligned unsigned 32 bit integer to native endian.
    #[inline]
    fn read_u32_bytes(self, n: [u8; 4]) -> u32 {
        if self.is_big_endian() {
            u32::from_be_bytes(n)
        } else {
            u32::from_le_bytes(n)
        }
    }

    /// Converts an unaligned unsigned 64 bit integer to native endian.
    #[inline]
    fn read_u64_bytes(self, n: [u8; 8]) -> u64 {
        if self.is_big_endian() {
            u64::from_be_bytes(n)
        } else {
            u64::from_le_bytes(n)
        }
    }

    /// Converts an unaligned signed 16 bit integer to native endian.
    #[inline]
    fn read_i16_bytes(self, n: [u8; 2]) -> i16 {
        if self.is_big_endian() {
            i16::from_be_bytes(n)
        } else {
            i16::from_le_bytes(n)
        }
    }

    /// Converts an unaligned signed 32 bit integer to native endian.
    #[inline]
    fn read_i32_bytes(self, n: [u8; 4]) -> i32 {
        if self.is_big_endian() {
            i32::from_be_bytes(n)
        } else {
            i32::from_le_bytes(n)
        }
    }

    /// Converts an unaligned signed 64 bit integer to native endian.
    #[inline]
    fn read_i64_bytes(self, n: [u8; 8]) -> i64 {
        if self.is_big_endian() {
            i64::from_be_bytes(n)
        } else {
            i64::from_le_bytes(n)
        }
    }

    /// Converts an unsigned 16 bit integer from native endian.
    #[inline]
    fn write_u16(self, n: u16) -> u16 {
        if self.is_big_endian() {
            u16::to_be(n)
        } else {
            u16::to_le(n)
        }
    }

    /// Converts an unsigned 32 bit integer from native endian.
    #[inline]
    fn write_u32(self, n: u32) -> u32 {
        if self.is_big_endian() {
            u32::to_be(n)
        } else {
            u32::to_le(n)
        }
    }

    /// Converts an unsigned 64 bit integer from native endian.
    #[inline]
    fn write_u64(self, n: u64) -> u64 {
        if self.is_big_endian() {
            u64::to_be(n)
        } else {
            u64::to_le(n)
        }
    }

    /// Converts a signed 16 bit integer from native endian.
    #[inline]
    fn write_i16(self, n: i16) -> i16 {
        if self.is_big_endian() {
            i16::to_be(n)
        } else {
            i16::to_le(n)
        }
    }

    /// Converts a signed 32 bit integer from native endian.
    #[inline]
    fn write_i32(self, n: i32) -> i32 {
        if self.is_big_endian() {
            i32::to_be(n)
        } else {
            i32::to_le(n)
        }
    }

    /// Converts a signed 64 bit integer from native endian.
    #[inline]
    fn write_i64(self, n: i64) -> i64 {
        if self.is_big_endian() {
            i64::to_be(n)
        } else {
            i64::to_le(n)
        }
    }

    /// Converts an unaligned unsigned 16 bit integer from native endian.
    #[inline]
    fn write_u16_bytes(self, n: u16) -> [u8; 2] {
        if self.is_big_endian() {
            u16::to_be_bytes(n)
        } else {
            u16::to_le_bytes(n)
        }
    }

    /// Converts an unaligned unsigned 32 bit integer from native endian.
    #[inline]
    fn write_u32_bytes(self, n: u32) -> [u8; 4] {
        if self.is_big_endian() {
            u32::to_be_bytes(n)
        } else {
            u32::to_le_bytes(n)
        }
    }

    /// Converts an unaligned unsigned 64 bit integer from native endian.
    #[inline]
    fn write_u64_bytes(self, n: u64) -> [u8; 8] {
        if self.is_big_endian() {
            u64::to_be_bytes(n)
        } else {
            u64::to_le_bytes(n)
        }
    }

    /// Converts an unaligned signed 16 bit integer from native endian.
    #[inline]
    fn write_i16_bytes(self, n: i16) -> [u8; 2] {
        if self.is_big_endian() {
            i16::to_be_bytes(n)
        } else {
            i16::to_le_bytes(n)
        }
    }

    /// Converts an unaligned signed 32 bit integer from native endian.
    #[inline]
    fn write_i32_bytes(self, n: i32) -> [u8; 4] {
        if self.is_big_endian() {
            i32::to_be_bytes(n)
        } else {
            i32::to_le_bytes(n)
        }
    }

    /// Converts an unaligned signed 64 bit integer from native endian.
    #[inline]
    fn write_i64_bytes(self, n: i64) -> [u8; 8] {
        if self.is_big_endian() {
            i64::to_be_bytes(n)
        } else {
            i64::to_le_bytes(n)
        }
    }
}

/// An endianness that is selectable at run-time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Endianness {
    /// Little endian byte order.
    Little,
    /// Big endian byte order.
    Big,
}

impl Default for Endianness {
    #[cfg(target_endian = "little")]
    #[inline]
    fn default() -> Endianness {
        Endianness::Little
    }

    #[cfg(target_endian = "big")]
    #[inline]
    fn default() -> Endianness {
        Endianness::Big
    }
}

impl Endian for Endianness {
    #[inline]
    fn from_big_endian(big_endian: bool) -> Option<Self> {
        Some(if big_endian {
            Endianness::Big
        } else {
            Endianness::Little
        })
    }

    #[inline]
    fn is_big_endian(self) -> bool {
        self != Endianness::Little
    }
}

/// Compile-time little endian byte order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LittleEndian;

impl Default for LittleEndian {
    #[inline]
    fn default() -> LittleEndian {
        LittleEndian
    }
}

impl Endian for LittleEndian {
    #[inline]
    fn from_big_endian(big_endian: bool) -> Option<Self> {
        if big_endian {
            None
        } else {
            Some(LittleEndian)
        }
    }

    #[inline]
    fn is_big_endian(self) -> bool {
        false
    }
}

/// Compile-time big endian byte order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BigEndian;

impl Default for BigEndian {
    #[inline]
    fn default() -> BigEndian {
        BigEndian
    }
}

impl Endian for BigEndian {
    #[inline]
    fn from_big_endian(big_endian: bool) -> Option<Self> {
        if big_endian {
            Some(BigEndian)
        } else {
            None
        }
    }

    #[inline]
    fn is_big_endian(self) -> bool {
        true
    }
}

/// The native endianness for the target platform.
#[cfg(target_endian = "little")]
pub type NativeEndian = LittleEndian;

#[cfg(target_endian = "little")]
#[allow(non_upper_case_globals)]
#[doc(hidden)]
pub const NativeEndian: LittleEndian = LittleEndian;

/// The native endianness for the target platform.
#[cfg(target_endian = "big")]
pub type NativeEndian = BigEndian;

#[cfg(target_endian = "big")]
#[allow(non_upper_case_globals)]
#[doc(hidden)]
pub const NativeEndian: BigEndian = BigEndian;

macro_rules! unsafe_impl_endian_pod {
    ($($struct_name:ident),+ $(,)?) => {
        $(
            unsafe impl<E: Endian> Pod for $struct_name<E> { }
        )+
    }
}

#[cfg(not(feature = "unaligned"))]
mod aligned {
    use super::{fmt, Endian, PhantomData, Pod};

    /// A `u16` value with an externally specified endianness of type `E`.
    #[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[repr(transparent)]
    pub struct U16<E: Endian>(u16, PhantomData<E>);

    impl<E: Endian> U16<E> {
        /// Construct a new value given bytes that already have the required endianness.
        pub const fn from_bytes(n: [u8; 2]) -> Self {
            Self(u16::from_ne_bytes(n), PhantomData)
        }

        /// Construct a new value given a native endian value.
        pub fn new(e: E, n: u16) -> Self {
            Self(e.write_u16(n), PhantomData)
        }

        /// Return the value as a native endian value.
        pub fn get(self, e: E) -> u16 {
            e.read_u16(self.0)
        }

        /// Set the value given a native endian value.
        pub fn set(&mut self, e: E, n: u16) {
            self.0 = e.write_u16(n);
        }
    }

    /// A `u32` value with an externally specified endianness of type `E`.
    #[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[repr(transparent)]
    pub struct U32<E: Endian>(u32, PhantomData<E>);

    impl<E: Endian> U32<E> {
        /// Construct a new value given bytes that already have the required endianness.
        pub const fn from_bytes(n: [u8; 4]) -> Self {
            Self(u32::from_ne_bytes(n), PhantomData)
        }

        /// Construct a new value given a native endian value.
        pub fn new(e: E, n: u32) -> Self {
            Self(e.write_u32(n), PhantomData)
        }
        /// Return the value as a native endian value.
        pub fn get(self, e: E) -> u32 {
            e.read_u32(self.0)
        }
        /// Set the value given a native endian value.
        pub fn set(&mut self, e: E, n: u32) {
            self.0 = e.write_u32(n);
        }
    }

    /// A `u64` value with an externally specified endianness of type `E`.
    #[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[repr(transparent)]
    pub struct U64<E: Endian>(u64, PhantomData<E>);

    impl<E: Endian> U64<E> {
        /// Construct a new value given bytes that already have the required endianness.
        pub const fn from_bytes(n: [u8; 8]) -> Self {
            Self(u64::from_ne_bytes(n), PhantomData)
        }

        /// Construct a new value given a native endian value.
        pub fn new(e: E, n: u64) -> Self {
            Self(e.write_u64(n), PhantomData)
        }
        /// Return the value as a native endian value.
        pub fn get(self, e: E) -> u64 {
            e.read_u64(self.0)
        }
        /// Set the value given a native endian value.
        pub fn set(&mut self, e: E, n: u64) {
            self.0 = e.write_u64(n);
        }
    }

    /// An `i16` value with an externally specified endianness of type `E`.
    #[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[repr(transparent)]
    pub struct I16<E: Endian>(i16, PhantomData<E>);

    impl<E: Endian> I16<E> {
        /// Construct a new value given bytes that already have the required endianness.
        pub const fn from_bytes(n: [u8; 2]) -> Self {
            Self(i16::from_ne_bytes(n), PhantomData)
        }

        /// Construct a new value given a native endian value.
        pub fn new(e: E, n: i16) -> Self {
            Self(e.write_i16(n), PhantomData)
        }
        /// Return the value as a native endian value.
        pub fn get(self, e: E) -> i16 {
            e.read_i16(self.0)
        }
        /// Set the value given a native endian value.
        pub fn set(&mut self, e: E, n: i16) {
            self.0 = e.write_i16(n);
        }
    }

    /// An `i32` value with an externally specified endianness of type `E`.
    #[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[repr(transparent)]
    pub struct I32<E: Endian>(i32, PhantomData<E>);

    impl<E: Endian> I32<E> {
        /// Construct a new value given bytes that already have the required endianness.
        pub const fn from_bytes(n: [u8; 4]) -> Self {
            Self(i32::from_ne_bytes(n), PhantomData)
        }

        /// Construct a new value given a native endian value.
        pub fn new(e: E, n: i32) -> Self {
            Self(e.write_i32(n), PhantomData)
        }
        /// Return the value as a native endian value.
        pub fn get(self, e: E) -> i32 {
            e.read_i32(self.0)
        }
        /// Set the value given a native endian value.
        pub fn set(&mut self, e: E, n: i32) {
            self.0 = e.write_i32(n);
        }
    }

    /// An `i64` value with an externally specified endianness of type `E`.
    #[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[repr(transparent)]
    pub struct I64<E: Endian>(i64, PhantomData<E>);

    impl<E: Endian> I64<E> {
        /// Construct a new value given bytes that already have the required endianness.
        pub const fn from_bytes(n: [u8; 8]) -> Self {
            Self(i64::from_ne_bytes(n), PhantomData)
        }

        /// Construct a new value given a native endian value.
        pub fn new(e: E, n: i64) -> Self {
            Self(e.write_i64(n), PhantomData)
        }
        /// Return the value as a native endian value.
        pub fn get(self, e: E) -> i64 {
            e.read_i64(self.0)
        }
        /// Set the value given a native endian value.
        pub fn set(&mut self, e: E, n: i64) {
            self.0 = e.write_i64(n);
        }
    }

    impl<E: Endian> fmt::Debug for U16<E> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "U16({:x})", self.0)
        }
    }

    impl<E: Endian> fmt::Debug for U32<E> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "U32({:x})", self.0)
        }
    }

    impl<E: Endian> fmt::Debug for U64<E> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "U64({:x})", self.0)
        }
    }

    impl<E: Endian> fmt::Debug for I16<E> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "I16({:x})", self.0)
        }
    }

    impl<E: Endian> fmt::Debug for I32<E> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "I32({:x})", self.0)
        }
    }

    impl<E: Endian> fmt::Debug for I64<E> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "I64({:x})", self.0)
        }
    }

    unsafe_impl_endian_pod!(U16, U32, U64, I16, I32, I64);
}

#[cfg(not(feature = "unaligned"))]
pub use aligned::*;

/// A `u16` value with an externally specified endianness of type `E`.
#[cfg(feature = "unaligned")]
pub type U16<E> = U16Bytes<E>;

/// A `u32` value with an externally specified endianness of type `E`.
#[cfg(feature = "unaligned")]
pub type U32<E> = U32Bytes<E>;

/// A `u64` value with an externally specified endianness of type `E`.
#[cfg(feature = "unaligned")]
pub type U64<E> = U64Bytes<E>;

/// An `i16` value with an externally specified endianness of type `E`.
#[cfg(feature = "unaligned")]
pub type I16<E> = I16Bytes<E>;

/// An `i32` value with an externally specified endianness of type `E`.
#[cfg(feature = "unaligned")]
pub type I32<E> = I32Bytes<E>;

/// An `i64` value with an externally specified endianness of type `E`.
#[cfg(feature = "unaligned")]
pub type I64<E> = I64Bytes<E>;

/// An unaligned `u16` value with an externally specified endianness of type `E`.
#[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct U16Bytes<E: Endian>([u8; 2], PhantomData<E>);

impl<E: Endian> U16Bytes<E> {
    /// Construct a new value given bytes that already have the required endianness.
    pub const fn from_bytes(n: [u8; 2]) -> Self {
        Self(n, PhantomData)
    }

    /// Construct a new value given a native endian value.
    pub fn new(e: E, n: u16) -> Self {
        Self(e.write_u16_bytes(n), PhantomData)
    }

    /// Return the value as a native endian value.
    pub fn get(self, e: E) -> u16 {
        e.read_u16_bytes(self.0)
    }

    /// Set the value given a native endian value.
    pub fn set(&mut self, e: E, n: u16) {
        self.0 = e.write_u16_bytes(n);
    }
}

/// An unaligned `u32` value with an externally specified endianness of type `E`.
#[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct U32Bytes<E: Endian>([u8; 4], PhantomData<E>);

impl<E: Endian> U32Bytes<E> {
    /// Construct a new value given bytes that already have the required endianness.
    pub const fn from_bytes(n: [u8; 4]) -> Self {
        Self(n, PhantomData)
    }

    /// Construct a new value given a native endian value.
    pub fn new(e: E, n: u32) -> Self {
        Self(e.write_u32_bytes(n), PhantomData)
    }

    /// Return the value as a native endian value.
    pub fn get(self, e: E) -> u32 {
        e.read_u32_bytes(self.0)
    }

    /// Set the value given a native endian value.
    pub fn set(&mut self, e: E, n: u32) {
        self.0 = e.write_u32_bytes(n);
    }
}

/// An unaligned `u64` value with an externally specified endianness of type `E`.
#[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct U64Bytes<E: Endian>([u8; 8], PhantomData<E>);

impl<E: Endian> U64Bytes<E> {
    /// Construct a new value given bytes that already have the required endianness.
    pub const fn from_bytes(n: [u8; 8]) -> Self {
        Self(n, PhantomData)
    }

    /// Construct a new value given a native endian value.
    pub fn new(e: E, n: u64) -> Self {
        Self(e.write_u64_bytes(n), PhantomData)
    }

    /// Return the value as a native endian value.
    pub fn get(self, e: E) -> u64 {
        e.read_u64_bytes(self.0)
    }

    /// Set the value given a native endian value.
    pub fn set(&mut self, e: E, n: u64) {
        self.0 = e.write_u64_bytes(n);
    }
}

/// An unaligned `i16` value with an externally specified endianness of type `E`.
#[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct I16Bytes<E: Endian>([u8; 2], PhantomData<E>);

impl<E: Endian> I16Bytes<E> {
    /// Construct a new value given bytes that already have the required endianness.
    pub const fn from_bytes(n: [u8; 2]) -> Self {
        Self(n, PhantomData)
    }

    /// Construct a new value given a native endian value.
    pub fn new(e: E, n: i16) -> Self {
        Self(e.write_i16_bytes(n), PhantomData)
    }

    /// Return the value as a native endian value.
    pub fn get(self, e: E) -> i16 {
        e.read_i16_bytes(self.0)
    }

    /// Set the value given a native endian value.
    pub fn set(&mut self, e: E, n: i16) {
        self.0 = e.write_i16_bytes(n);
    }
}

/// An unaligned `i32` value with an externally specified endianness of type `E`.
#[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct I32Bytes<E: Endian>([u8; 4], PhantomData<E>);

impl<E: Endian> I32Bytes<E> {
    /// Construct a new value given bytes that already have the required endianness.
    pub const fn from_bytes(n: [u8; 4]) -> Self {
        Self(n, PhantomData)
    }

    /// Construct a new value given a native endian value.
    pub fn new(e: E, n: i32) -> Self {
        Self(e.write_i32_bytes(n), PhantomData)
    }

    /// Return the value as a native endian value.
    pub fn get(self, e: E) -> i32 {
        e.read_i32_bytes(self.0)
    }

    /// Set the value given a native endian value.
    pub fn set(&mut self, e: E, n: i32) {
        self.0 = e.write_i32_bytes(n);
    }
}

/// An unaligned `i64` value with an externally specified endianness of type `E`.
#[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct I64Bytes<E: Endian>([u8; 8], PhantomData<E>);

impl<E: Endian> I64Bytes<E> {
    /// Construct a new value given bytes that already have the required endianness.
    pub const fn from_bytes(n: [u8; 8]) -> Self {
        Self(n, PhantomData)
    }

    /// Construct a new value given a native endian value.
    pub fn new(e: E, n: i64) -> Self {
        Self(e.write_i64_bytes(n), PhantomData)
    }

    /// Return the value as a native endian value.
    pub fn get(self, e: E) -> i64 {
        e.read_i64_bytes(self.0)
    }

    /// Set the value given a native endian value.
    pub fn set(&mut self, e: E, n: i64) {
        self.0 = e.write_i64_bytes(n);
    }
}

impl<E: Endian> fmt::Debug for U16Bytes<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "U16({:x}, {:x})", self.0[0], self.0[1],)
    }
}

impl<E: Endian> fmt::Debug for U32Bytes<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "U32({:x}, {:x}, {:x}, {:x})",
            self.0[0], self.0[1], self.0[2], self.0[3],
        )
    }
}

impl<E: Endian> fmt::Debug for U64Bytes<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "U64({:x}, {:x}, {:x}, {:x}, {:x}, {:x}, {:x}, {:x})",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], self.0[7],
        )
    }
}

impl<E: Endian> fmt::Debug for I16Bytes<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "I16({:x}, {:x})", self.0[0], self.0[1],)
    }
}

impl<E: Endian> fmt::Debug for I32Bytes<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "I32({:x}, {:x}, {:x}, {:x})",
            self.0[0], self.0[1], self.0[2], self.0[3],
        )
    }
}

impl<E: Endian> fmt::Debug for I64Bytes<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "I64({:x}, {:x}, {:x}, {:x}, {:x}, {:x}, {:x}, {:x})",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], self.0[7],
        )
    }
}

unsafe_impl_endian_pod!(U16Bytes, U32Bytes, U64Bytes, I16Bytes, I32Bytes, I64Bytes);
