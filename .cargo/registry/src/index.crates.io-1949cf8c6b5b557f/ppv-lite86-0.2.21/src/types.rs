#![allow(non_camel_case_types)]
use core::ops::{Add, AddAssign, BitAnd, BitOr, BitXor, BitXorAssign, Not};

pub trait AndNot {
    type Output;
    fn andnot(self, rhs: Self) -> Self::Output;
}
pub trait BSwap {
    fn bswap(self) -> Self;
}
/// Ops that depend on word size
pub trait ArithOps: Add<Output = Self> + AddAssign + Sized + Copy + Clone + BSwap {}
/// Ops that are independent of word size and endian
pub trait BitOps0:
    BitAnd<Output = Self>
    + BitOr<Output = Self>
    + BitXor<Output = Self>
    + BitXorAssign
    + Not<Output = Self>
    + AndNot<Output = Self>
    + Sized
    + Copy
    + Clone
{
}

pub trait BitOps32: BitOps0 + RotateEachWord32 {}
pub trait BitOps64: BitOps32 + RotateEachWord64 {}
pub trait BitOps128: BitOps64 + RotateEachWord128 {}

pub trait RotateEachWord32 {
    fn rotate_each_word_right7(self) -> Self;
    fn rotate_each_word_right8(self) -> Self;
    fn rotate_each_word_right11(self) -> Self;
    fn rotate_each_word_right12(self) -> Self;
    fn rotate_each_word_right16(self) -> Self;
    fn rotate_each_word_right20(self) -> Self;
    fn rotate_each_word_right24(self) -> Self;
    fn rotate_each_word_right25(self) -> Self;
}

pub trait RotateEachWord64 {
    fn rotate_each_word_right32(self) -> Self;
}

pub trait RotateEachWord128 {}

// Vector type naming scheme:
// uN[xP]xL
// Unsigned; N-bit words * P bits per lane * L lanes
//
// A lane is always 128-bits, chosen because common SIMD architectures treat 128-bit units of
// wide vectors specially (supporting e.g. intra-lane shuffles), and tend to have limited and
// slow inter-lane operations.

use crate::arch::{vec128_storage, vec256_storage, vec512_storage};

#[allow(clippy::missing_safety_doc)]
pub trait UnsafeFrom<T> {
    unsafe fn unsafe_from(t: T) -> Self;
}

/// A vector composed of two elements, which may be words or themselves vectors.
pub trait Vec2<W> {
    fn extract(self, i: u32) -> W;
    fn insert(self, w: W, i: u32) -> Self;
}

/// A vector composed of four elements, which may be words or themselves vectors.
pub trait Vec4<W> {
    fn extract(self, i: u32) -> W;
    fn insert(self, w: W, i: u32) -> Self;
}
/// Vec4 functions which may not be implemented yet for all Vec4 types.
/// NOTE: functions in this trait may be moved to Vec4 in any patch release. To avoid breakage,
/// import Vec4Ext only together with Vec4, and don't qualify its methods.
pub trait Vec4Ext<W> {
    fn transpose4(a: Self, b: Self, c: Self, d: Self) -> (Self, Self, Self, Self)
    where
        Self: Sized;
}
pub trait Vector<T> {
    fn to_scalars(self) -> T;
}

// TODO: multiples of 4 should inherit this
/// A vector composed of four words; depending on their size, operations may cross lanes.
pub trait Words4 {
    fn shuffle1230(self) -> Self;
    fn shuffle2301(self) -> Self;
    fn shuffle3012(self) -> Self;
}

/// A vector composed one or more lanes each composed of four words.
pub trait LaneWords4 {
    fn shuffle_lane_words1230(self) -> Self;
    fn shuffle_lane_words2301(self) -> Self;
    fn shuffle_lane_words3012(self) -> Self;
}

// TODO: make this a part of BitOps
/// Exchange neigboring ranges of bits of the specified size
pub trait Swap64 {
    fn swap1(self) -> Self;
    fn swap2(self) -> Self;
    fn swap4(self) -> Self;
    fn swap8(self) -> Self;
    fn swap16(self) -> Self;
    fn swap32(self) -> Self;
    fn swap64(self) -> Self;
}

pub trait u32x4<M: Machine>:
    BitOps32
    + Store<vec128_storage>
    + ArithOps
    + Vec4<u32>
    + Words4
    + LaneWords4
    + StoreBytes
    + MultiLane<[u32; 4]>
    + Into<vec128_storage>
{
}
pub trait u64x2<M: Machine>:
    BitOps64 + Store<vec128_storage> + ArithOps + Vec2<u64> + MultiLane<[u64; 2]> + Into<vec128_storage>
{
}
pub trait u128x1<M: Machine>:
    BitOps128 + Store<vec128_storage> + Swap64 + MultiLane<[u128; 1]> + Into<vec128_storage>
{
}

pub trait u32x4x2<M: Machine>:
    BitOps32
    + Store<vec256_storage>
    + Vec2<M::u32x4>
    + MultiLane<[M::u32x4; 2]>
    + ArithOps
    + Into<vec256_storage>
    + StoreBytes
{
}
pub trait u64x2x2<M: Machine>:
    BitOps64
    + Store<vec256_storage>
    + Vec2<M::u64x2>
    + MultiLane<[M::u64x2; 2]>
    + ArithOps
    + StoreBytes
    + Into<vec256_storage>
{
}
pub trait u64x4<M: Machine>:
    BitOps64
    + Store<vec256_storage>
    + Vec4<u64>
    + MultiLane<[u64; 4]>
    + ArithOps
    + Words4
    + StoreBytes
    + Into<vec256_storage>
{
}
pub trait u128x2<M: Machine>:
    BitOps128
    + Store<vec256_storage>
    + Vec2<M::u128x1>
    + MultiLane<[M::u128x1; 2]>
    + Swap64
    + Into<vec256_storage>
{
}

pub trait u32x4x4<M: Machine>:
    BitOps32
    + Store<vec512_storage>
    + Vec4<M::u32x4>
    + Vec4Ext<M::u32x4>
    + Vector<[u32; 16]>
    + MultiLane<[M::u32x4; 4]>
    + ArithOps
    + LaneWords4
    + Into<vec512_storage>
    + StoreBytes
{
}
pub trait u64x2x4<M: Machine>:
    BitOps64
    + Store<vec512_storage>
    + Vec4<M::u64x2>
    + MultiLane<[M::u64x2; 4]>
    + ArithOps
    + Into<vec512_storage>
{
}
// TODO: Words4
pub trait u128x4<M: Machine>:
    BitOps128
    + Store<vec512_storage>
    + Vec4<M::u128x1>
    + MultiLane<[M::u128x1; 4]>
    + Swap64
    + Into<vec512_storage>
{
}

/// A vector composed of multiple 128-bit lanes.
pub trait MultiLane<Lanes> {
    /// Split a multi-lane vector into single-lane vectors.
    fn to_lanes(self) -> Lanes;
    /// Build a multi-lane vector from individual lanes.
    fn from_lanes(lanes: Lanes) -> Self;
}

/// Combine single vectors into a multi-lane vector.
pub trait VZip<V> {
    fn vzip(self) -> V;
}

impl<V, T> VZip<V> for T
where
    V: MultiLane<T>,
{
    #[inline(always)]
    fn vzip(self) -> V {
        V::from_lanes(self)
    }
}

pub trait Machine: Sized + Copy {
    type u32x4: u32x4<Self>;
    type u64x2: u64x2<Self>;
    type u128x1: u128x1<Self>;

    type u32x4x2: u32x4x2<Self>;
    type u64x2x2: u64x2x2<Self>;
    type u64x4: u64x4<Self>;
    type u128x2: u128x2<Self>;

    type u32x4x4: u32x4x4<Self>;
    type u64x2x4: u64x2x4<Self>;
    type u128x4: u128x4<Self>;

    #[inline(always)]
    fn unpack<S, V: Store<S>>(self, s: S) -> V {
        unsafe { V::unpack(s) }
    }

    #[inline(always)]
    fn vec<V, A>(self, a: A) -> V
    where
        V: MultiLane<A>,
    {
        V::from_lanes(a)
    }

    #[inline(always)]
    fn read_le<V>(self, input: &[u8]) -> V
    where
        V: StoreBytes,
    {
        unsafe { V::unsafe_read_le(input) }
    }

    #[inline(always)]
    fn read_be<V>(self, input: &[u8]) -> V
    where
        V: StoreBytes,
    {
        unsafe { V::unsafe_read_be(input) }
    }

    /// # Safety
    /// Caller must ensure the type of Self is appropriate for the hardware of the execution
    /// environment.
    unsafe fn instance() -> Self;
}

pub trait Store<S> {
    /// # Safety
    /// Caller must ensure the type of Self is appropriate for the hardware of the execution
    /// environment.
    unsafe fn unpack(p: S) -> Self;
}

pub trait StoreBytes {
    /// # Safety
    /// Caller must ensure the type of Self is appropriate for the hardware of the execution
    /// environment.
    unsafe fn unsafe_read_le(input: &[u8]) -> Self;
    /// # Safety
    /// Caller must ensure the type of Self is appropriate for the hardware of the execution
    /// environment.
    unsafe fn unsafe_read_be(input: &[u8]) -> Self;
    fn write_le(self, out: &mut [u8]);
    fn write_be(self, out: &mut [u8]);
}
