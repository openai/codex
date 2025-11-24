#![allow(non_camel_case_types)]

use crate::soft::{x2, x4};
use crate::types::*;
use core::ops::*;
use zerocopy::{FromBytes, IntoBytes};

zerocopy::cryptocorrosion_derive_traits! {
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub union vec128_storage {
        d: [u32; 4],
        q: [u64; 2],
    }
}

impl From<[u32; 4]> for vec128_storage {
    #[inline(always)]
    fn from(d: [u32; 4]) -> Self {
        Self { d }
    }
}
impl From<vec128_storage> for [u32; 4] {
    #[inline(always)]
    fn from(d: vec128_storage) -> Self {
        unsafe { d.d }
    }
}
impl From<[u64; 2]> for vec128_storage {
    #[inline(always)]
    fn from(q: [u64; 2]) -> Self {
        Self { q }
    }
}
impl From<vec128_storage> for [u64; 2] {
    #[inline(always)]
    fn from(q: vec128_storage) -> Self {
        unsafe { q.q }
    }
}
impl Default for vec128_storage {
    #[inline(always)]
    fn default() -> Self {
        Self { q: [0, 0] }
    }
}
impl Eq for vec128_storage {}
impl PartialEq<vec128_storage> for vec128_storage {
    #[inline(always)]
    fn eq(&self, rhs: &Self) -> bool {
        unsafe { self.q == rhs.q }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct vec256_storage {
    v128: [vec128_storage; 2],
}
impl vec256_storage {
    #[inline(always)]
    pub fn new128(v128: [vec128_storage; 2]) -> Self {
        Self { v128 }
    }
    #[inline(always)]
    pub fn split128(self) -> [vec128_storage; 2] {
        self.v128
    }
}
impl From<vec256_storage> for [u64; 4] {
    #[inline(always)]
    fn from(q: vec256_storage) -> Self {
        let [a, b]: [u64; 2] = q.v128[0].into();
        let [c, d]: [u64; 2] = q.v128[1].into();
        [a, b, c, d]
    }
}
impl From<[u64; 4]> for vec256_storage {
    #[inline(always)]
    fn from([a, b, c, d]: [u64; 4]) -> Self {
        Self {
            v128: [[a, b].into(), [c, d].into()],
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct vec512_storage {
    v128: [vec128_storage; 4],
}
impl vec512_storage {
    #[inline(always)]
    pub fn new128(v128: [vec128_storage; 4]) -> Self {
        Self { v128 }
    }
    #[inline(always)]
    pub fn split128(self) -> [vec128_storage; 4] {
        self.v128
    }
}

#[inline(always)]
fn dmap<T, F>(t: T, f: F) -> T
where
    T: Store<vec128_storage> + Into<vec128_storage>,
    F: Fn(u32) -> u32,
{
    let t: vec128_storage = t.into();
    let d = unsafe { t.d };
    let d = vec128_storage {
        d: [f(d[0]), f(d[1]), f(d[2]), f(d[3])],
    };
    unsafe { T::unpack(d) }
}

fn dmap2<T, F>(a: T, b: T, f: F) -> T
where
    T: Store<vec128_storage> + Into<vec128_storage>,
    F: Fn(u32, u32) -> u32,
{
    let a: vec128_storage = a.into();
    let b: vec128_storage = b.into();
    let ao = unsafe { a.d };
    let bo = unsafe { b.d };
    let d = vec128_storage {
        d: [
            f(ao[0], bo[0]),
            f(ao[1], bo[1]),
            f(ao[2], bo[2]),
            f(ao[3], bo[3]),
        ],
    };
    unsafe { T::unpack(d) }
}

#[inline(always)]
fn qmap<T, F>(t: T, f: F) -> T
where
    T: Store<vec128_storage> + Into<vec128_storage>,
    F: Fn(u64) -> u64,
{
    let t: vec128_storage = t.into();
    let q = unsafe { t.q };
    let q = vec128_storage {
        q: [f(q[0]), f(q[1])],
    };
    unsafe { T::unpack(q) }
}

#[inline(always)]
fn qmap2<T, F>(a: T, b: T, f: F) -> T
where
    T: Store<vec128_storage> + Into<vec128_storage>,
    F: Fn(u64, u64) -> u64,
{
    let a: vec128_storage = a.into();
    let b: vec128_storage = b.into();
    let ao = unsafe { a.q };
    let bo = unsafe { b.q };
    let q = vec128_storage {
        q: [f(ao[0], bo[0]), f(ao[1], bo[1])],
    };
    unsafe { T::unpack(q) }
}

#[inline(always)]
fn o_of_q(q: [u64; 2]) -> u128 {
    u128::from(q[0]) | (u128::from(q[1]) << 64)
}

#[inline(always)]
fn q_of_o(o: u128) -> [u64; 2] {
    [o as u64, (o >> 64) as u64]
}

#[inline(always)]
fn omap<T, F>(a: T, f: F) -> T
where
    T: Store<vec128_storage> + Into<vec128_storage>,
    F: Fn(u128) -> u128,
{
    let a: vec128_storage = a.into();
    let ao = o_of_q(unsafe { a.q });
    let o = vec128_storage { q: q_of_o(f(ao)) };
    unsafe { T::unpack(o) }
}

#[inline(always)]
fn omap2<T, F>(a: T, b: T, f: F) -> T
where
    T: Store<vec128_storage> + Into<vec128_storage>,
    F: Fn(u128, u128) -> u128,
{
    let a: vec128_storage = a.into();
    let b: vec128_storage = b.into();
    let ao = o_of_q(unsafe { a.q });
    let bo = o_of_q(unsafe { b.q });
    let o = vec128_storage {
        q: q_of_o(f(ao, bo)),
    };
    unsafe { T::unpack(o) }
}

impl RotateEachWord128 for u128x1_generic {}
impl BitOps128 for u128x1_generic {}
impl BitOps64 for u128x1_generic {}
impl BitOps64 for u64x2_generic {}
impl BitOps32 for u128x1_generic {}
impl BitOps32 for u64x2_generic {}
impl BitOps32 for u32x4_generic {}
impl BitOps0 for u128x1_generic {}
impl BitOps0 for u64x2_generic {}
impl BitOps0 for u32x4_generic {}

macro_rules! impl_bitops {
    ($vec:ident) => {
        impl Not for $vec {
            type Output = Self;
            #[inline(always)]
            fn not(self) -> Self::Output {
                omap(self, |x| !x)
            }
        }
        impl BitAnd for $vec {
            type Output = Self;
            #[inline(always)]
            fn bitand(self, rhs: Self) -> Self::Output {
                omap2(self, rhs, |x, y| x & y)
            }
        }
        impl BitOr for $vec {
            type Output = Self;
            #[inline(always)]
            fn bitor(self, rhs: Self) -> Self::Output {
                omap2(self, rhs, |x, y| x | y)
            }
        }
        impl BitXor for $vec {
            type Output = Self;
            #[inline(always)]
            fn bitxor(self, rhs: Self) -> Self::Output {
                omap2(self, rhs, |x, y| x ^ y)
            }
        }
        impl AndNot for $vec {
            type Output = Self;
            #[inline(always)]
            fn andnot(self, rhs: Self) -> Self::Output {
                omap2(self, rhs, |x, y| !x & y)
            }
        }
        impl BitAndAssign for $vec {
            #[inline(always)]
            fn bitand_assign(&mut self, rhs: Self) {
                *self = *self & rhs
            }
        }
        impl BitOrAssign for $vec {
            #[inline(always)]
            fn bitor_assign(&mut self, rhs: Self) {
                *self = *self | rhs
            }
        }
        impl BitXorAssign for $vec {
            #[inline(always)]
            fn bitxor_assign(&mut self, rhs: Self) {
                *self = *self ^ rhs
            }
        }

        impl Swap64 for $vec {
            #[inline(always)]
            fn swap1(self) -> Self {
                qmap(self, |x| {
                    ((x & 0x5555555555555555) << 1) | ((x & 0xaaaaaaaaaaaaaaaa) >> 1)
                })
            }
            #[inline(always)]
            fn swap2(self) -> Self {
                qmap(self, |x| {
                    ((x & 0x3333333333333333) << 2) | ((x & 0xcccccccccccccccc) >> 2)
                })
            }
            #[inline(always)]
            fn swap4(self) -> Self {
                qmap(self, |x| {
                    ((x & 0x0f0f0f0f0f0f0f0f) << 4) | ((x & 0xf0f0f0f0f0f0f0f0) >> 4)
                })
            }
            #[inline(always)]
            fn swap8(self) -> Self {
                qmap(self, |x| {
                    ((x & 0x00ff00ff00ff00ff) << 8) | ((x & 0xff00ff00ff00ff00) >> 8)
                })
            }
            #[inline(always)]
            fn swap16(self) -> Self {
                dmap(self, |x| x.rotate_left(16))
            }
            #[inline(always)]
            fn swap32(self) -> Self {
                qmap(self, |x| x.rotate_left(32))
            }
            #[inline(always)]
            fn swap64(self) -> Self {
                omap(self, |x| (x << 64) | (x >> 64))
            }
        }
    };
}
impl_bitops!(u32x4_generic);
impl_bitops!(u64x2_generic);
impl_bitops!(u128x1_generic);

impl RotateEachWord32 for u32x4_generic {
    #[inline(always)]
    fn rotate_each_word_right7(self) -> Self {
        dmap(self, |x| x.rotate_right(7))
    }
    #[inline(always)]
    fn rotate_each_word_right8(self) -> Self {
        dmap(self, |x| x.rotate_right(8))
    }
    #[inline(always)]
    fn rotate_each_word_right11(self) -> Self {
        dmap(self, |x| x.rotate_right(11))
    }
    #[inline(always)]
    fn rotate_each_word_right12(self) -> Self {
        dmap(self, |x| x.rotate_right(12))
    }
    #[inline(always)]
    fn rotate_each_word_right16(self) -> Self {
        dmap(self, |x| x.rotate_right(16))
    }
    #[inline(always)]
    fn rotate_each_word_right20(self) -> Self {
        dmap(self, |x| x.rotate_right(20))
    }
    #[inline(always)]
    fn rotate_each_word_right24(self) -> Self {
        dmap(self, |x| x.rotate_right(24))
    }
    #[inline(always)]
    fn rotate_each_word_right25(self) -> Self {
        dmap(self, |x| x.rotate_right(25))
    }
}

impl RotateEachWord32 for u64x2_generic {
    #[inline(always)]
    fn rotate_each_word_right7(self) -> Self {
        qmap(self, |x| x.rotate_right(7))
    }
    #[inline(always)]
    fn rotate_each_word_right8(self) -> Self {
        qmap(self, |x| x.rotate_right(8))
    }
    #[inline(always)]
    fn rotate_each_word_right11(self) -> Self {
        qmap(self, |x| x.rotate_right(11))
    }
    #[inline(always)]
    fn rotate_each_word_right12(self) -> Self {
        qmap(self, |x| x.rotate_right(12))
    }
    #[inline(always)]
    fn rotate_each_word_right16(self) -> Self {
        qmap(self, |x| x.rotate_right(16))
    }
    #[inline(always)]
    fn rotate_each_word_right20(self) -> Self {
        qmap(self, |x| x.rotate_right(20))
    }
    #[inline(always)]
    fn rotate_each_word_right24(self) -> Self {
        qmap(self, |x| x.rotate_right(24))
    }
    #[inline(always)]
    fn rotate_each_word_right25(self) -> Self {
        qmap(self, |x| x.rotate_right(25))
    }
}
impl RotateEachWord64 for u64x2_generic {
    #[inline(always)]
    fn rotate_each_word_right32(self) -> Self {
        qmap(self, |x| x.rotate_right(32))
    }
}

// workaround for koute/cargo-web#52 (u128::rotate_* broken with cargo web)
#[inline(always)]
fn rotate_u128_right(x: u128, i: u32) -> u128 {
    (x >> i) | (x << (128 - i))
}
#[test]
fn test_rotate_u128() {
    const X: u128 = 0x0001_0203_0405_0607_0809_0a0b_0c0d_0e0f;
    assert_eq!(rotate_u128_right(X, 17), X.rotate_right(17));
}

impl RotateEachWord32 for u128x1_generic {
    #[inline(always)]
    fn rotate_each_word_right7(self) -> Self {
        Self([rotate_u128_right(self.0[0], 7)])
    }
    #[inline(always)]
    fn rotate_each_word_right8(self) -> Self {
        Self([rotate_u128_right(self.0[0], 8)])
    }
    #[inline(always)]
    fn rotate_each_word_right11(self) -> Self {
        Self([rotate_u128_right(self.0[0], 11)])
    }
    #[inline(always)]
    fn rotate_each_word_right12(self) -> Self {
        Self([rotate_u128_right(self.0[0], 12)])
    }
    #[inline(always)]
    fn rotate_each_word_right16(self) -> Self {
        Self([rotate_u128_right(self.0[0], 16)])
    }
    #[inline(always)]
    fn rotate_each_word_right20(self) -> Self {
        Self([rotate_u128_right(self.0[0], 20)])
    }
    #[inline(always)]
    fn rotate_each_word_right24(self) -> Self {
        Self([rotate_u128_right(self.0[0], 24)])
    }
    #[inline(always)]
    fn rotate_each_word_right25(self) -> Self {
        Self([rotate_u128_right(self.0[0], 25)])
    }
}
impl RotateEachWord64 for u128x1_generic {
    #[inline(always)]
    fn rotate_each_word_right32(self) -> Self {
        Self([rotate_u128_right(self.0[0], 32)])
    }
}

#[derive(Copy, Clone)]
pub struct GenericMachine;
impl Machine for GenericMachine {
    type u32x4 = u32x4_generic;
    type u64x2 = u64x2_generic;
    type u128x1 = u128x1_generic;
    type u32x4x2 = u32x4x2_generic;
    type u64x2x2 = u64x2x2_generic;
    type u64x4 = u64x4_generic;
    type u128x2 = u128x2_generic;
    type u32x4x4 = u32x4x4_generic;
    type u64x2x4 = u64x2x4_generic;
    type u128x4 = u128x4_generic;
    #[inline(always)]
    unsafe fn instance() -> Self {
        Self
    }
}

zerocopy::cryptocorrosion_derive_traits! {
    #[repr(transparent)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    pub struct u32x4_generic([u32; 4]);
}

zerocopy::cryptocorrosion_derive_traits! {
    #[repr(transparent)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    pub struct u64x2_generic([u64; 2]);
}

zerocopy::cryptocorrosion_derive_traits! {
    #[repr(transparent)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    pub struct u128x1_generic([u128; 1]);
}

impl From<u32x4_generic> for vec128_storage {
    #[inline(always)]
    fn from(d: u32x4_generic) -> Self {
        Self { d: d.0 }
    }
}
impl From<u64x2_generic> for vec128_storage {
    #[inline(always)]
    fn from(q: u64x2_generic) -> Self {
        Self { q: q.0 }
    }
}
impl From<u128x1_generic> for vec128_storage {
    #[inline(always)]
    fn from(o: u128x1_generic) -> Self {
        Self { q: q_of_o(o.0[0]) }
    }
}

impl Store<vec128_storage> for u32x4_generic {
    #[inline(always)]
    unsafe fn unpack(s: vec128_storage) -> Self {
        Self(s.d)
    }
}
impl Store<vec128_storage> for u64x2_generic {
    #[inline(always)]
    unsafe fn unpack(s: vec128_storage) -> Self {
        Self(s.q)
    }
}
impl Store<vec128_storage> for u128x1_generic {
    #[inline(always)]
    unsafe fn unpack(s: vec128_storage) -> Self {
        Self([o_of_q(s.q); 1])
    }
}

impl ArithOps for u32x4_generic {}
impl ArithOps for u64x2_generic {}
impl ArithOps for u128x1_generic {}

impl Add for u32x4_generic {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        dmap2(self, rhs, |x, y| x.wrapping_add(y))
    }
}
impl Add for u64x2_generic {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        qmap2(self, rhs, |x, y| x.wrapping_add(y))
    }
}
impl Add for u128x1_generic {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        omap2(self, rhs, |x, y| x.wrapping_add(y))
    }
}
impl AddAssign for u32x4_generic {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs
    }
}
impl AddAssign for u64x2_generic {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs
    }
}
impl AddAssign for u128x1_generic {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs
    }
}
impl BSwap for u32x4_generic {
    #[inline(always)]
    fn bswap(self) -> Self {
        dmap(self, |x| x.swap_bytes())
    }
}
impl BSwap for u64x2_generic {
    #[inline(always)]
    fn bswap(self) -> Self {
        qmap(self, |x| x.swap_bytes())
    }
}
impl BSwap for u128x1_generic {
    #[inline(always)]
    fn bswap(self) -> Self {
        omap(self, |x| x.swap_bytes())
    }
}
impl StoreBytes for u32x4_generic {
    #[inline(always)]
    unsafe fn unsafe_read_le(input: &[u8]) -> Self {
        let x = u32x4_generic::read_from_bytes(input).unwrap();
        dmap(x, |x| x.to_le())
    }
    #[inline(always)]
    unsafe fn unsafe_read_be(input: &[u8]) -> Self {
        let x = u32x4_generic::read_from_bytes(input).unwrap();
        dmap(x, |x| x.to_be())
    }
    #[inline(always)]
    fn write_le(self, out: &mut [u8]) {
        let x = dmap(self, |x| x.to_le());
        x.write_to(out).unwrap();
    }
    #[inline(always)]
    fn write_be(self, out: &mut [u8]) {
        let x = dmap(self, |x| x.to_be());
        x.write_to(out).unwrap();
    }
}
impl StoreBytes for u64x2_generic {
    #[inline(always)]
    unsafe fn unsafe_read_le(input: &[u8]) -> Self {
        let x = u64x2_generic::read_from_bytes(input).unwrap();
        qmap(x, |x| x.to_le())
    }
    #[inline(always)]
    unsafe fn unsafe_read_be(input: &[u8]) -> Self {
        let x = u64x2_generic::read_from_bytes(input).unwrap();
        qmap(x, |x| x.to_be())
    }
    #[inline(always)]
    fn write_le(self, out: &mut [u8]) {
        let x = qmap(self, |x| x.to_le());
        x.write_to(out).unwrap();
    }
    #[inline(always)]
    fn write_be(self, out: &mut [u8]) {
        let x = qmap(self, |x| x.to_be());
        x.write_to(out).unwrap();
    }
}

#[derive(Copy, Clone)]
pub struct G0;
#[derive(Copy, Clone)]
pub struct G1;
pub type u32x4x2_generic = x2<u32x4_generic, G0>;
pub type u64x2x2_generic = x2<u64x2_generic, G0>;
pub type u64x4_generic = x2<u64x2_generic, G1>;
pub type u128x2_generic = x2<u128x1_generic, G0>;
pub type u32x4x4_generic = x4<u32x4_generic>;
pub type u64x2x4_generic = x4<u64x2_generic>;
pub type u128x4_generic = x4<u128x1_generic>;

impl Vector<[u32; 16]> for u32x4x4_generic {
    fn to_scalars(self) -> [u32; 16] {
        let [a, b, c, d] = self.0;
        let a = a.0;
        let b = b.0;
        let c = c.0;
        let d = d.0;
        [
            a[0], a[1], a[2], a[3], //
            b[0], b[1], b[2], b[3], //
            c[0], c[1], c[2], c[3], //
            d[0], d[1], d[2], d[3], //
        ]
    }
}

impl MultiLane<[u32; 4]> for u32x4_generic {
    #[inline(always)]
    fn to_lanes(self) -> [u32; 4] {
        self.0
    }
    #[inline(always)]
    fn from_lanes(xs: [u32; 4]) -> Self {
        Self(xs)
    }
}
impl MultiLane<[u64; 2]> for u64x2_generic {
    #[inline(always)]
    fn to_lanes(self) -> [u64; 2] {
        self.0
    }
    #[inline(always)]
    fn from_lanes(xs: [u64; 2]) -> Self {
        Self(xs)
    }
}
impl MultiLane<[u64; 4]> for u64x4_generic {
    #[inline(always)]
    fn to_lanes(self) -> [u64; 4] {
        let (a, b) = (self.0[0].to_lanes(), self.0[1].to_lanes());
        [a[0], a[1], b[0], b[1]]
    }
    #[inline(always)]
    fn from_lanes(xs: [u64; 4]) -> Self {
        let (a, b) = (
            u64x2_generic::from_lanes([xs[0], xs[1]]),
            u64x2_generic::from_lanes([xs[2], xs[3]]),
        );
        x2::new([a, b])
    }
}
impl MultiLane<[u128; 1]> for u128x1_generic {
    #[inline(always)]
    fn to_lanes(self) -> [u128; 1] {
        self.0
    }
    #[inline(always)]
    fn from_lanes(xs: [u128; 1]) -> Self {
        Self(xs)
    }
}
impl Vec4<u32> for u32x4_generic {
    #[inline(always)]
    fn extract(self, i: u32) -> u32 {
        self.0[i as usize]
    }
    #[inline(always)]
    fn insert(mut self, v: u32, i: u32) -> Self {
        self.0[i as usize] = v;
        self
    }
}
impl Vec4<u64> for u64x4_generic {
    #[inline(always)]
    fn extract(self, i: u32) -> u64 {
        let d: [u64; 4] = self.to_lanes();
        d[i as usize]
    }
    #[inline(always)]
    fn insert(self, v: u64, i: u32) -> Self {
        self.0[(i / 2) as usize].insert(v, i % 2);
        self
    }
}
impl Vec2<u64> for u64x2_generic {
    #[inline(always)]
    fn extract(self, i: u32) -> u64 {
        self.0[i as usize]
    }
    #[inline(always)]
    fn insert(mut self, v: u64, i: u32) -> Self {
        self.0[i as usize] = v;
        self
    }
}

impl Words4 for u32x4_generic {
    #[inline(always)]
    fn shuffle2301(self) -> Self {
        self.swap64()
    }
    #[inline(always)]
    fn shuffle1230(self) -> Self {
        let x = self.0;
        Self([x[3], x[0], x[1], x[2]])
    }
    #[inline(always)]
    fn shuffle3012(self) -> Self {
        let x = self.0;
        Self([x[1], x[2], x[3], x[0]])
    }
}
impl LaneWords4 for u32x4_generic {
    #[inline(always)]
    fn shuffle_lane_words2301(self) -> Self {
        self.shuffle2301()
    }
    #[inline(always)]
    fn shuffle_lane_words1230(self) -> Self {
        self.shuffle1230()
    }
    #[inline(always)]
    fn shuffle_lane_words3012(self) -> Self {
        self.shuffle3012()
    }
}

impl Words4 for u64x4_generic {
    #[inline(always)]
    fn shuffle2301(self) -> Self {
        x2::new([self.0[1], self.0[0]])
    }
    #[inline(always)]
    fn shuffle1230(self) -> Self {
        unimplemented!()
    }
    #[inline(always)]
    fn shuffle3012(self) -> Self {
        unimplemented!()
    }
}

impl u32x4<GenericMachine> for u32x4_generic {}
impl u64x2<GenericMachine> for u64x2_generic {}
impl u128x1<GenericMachine> for u128x1_generic {}
impl u32x4x2<GenericMachine> for u32x4x2_generic {}
impl u64x2x2<GenericMachine> for u64x2x2_generic {}
impl u64x4<GenericMachine> for u64x4_generic {}
impl u128x2<GenericMachine> for u128x2_generic {}
impl u32x4x4<GenericMachine> for u32x4x4_generic {}
impl u64x2x4<GenericMachine> for u64x2x4_generic {}
impl u128x4<GenericMachine> for u128x4_generic {}

#[macro_export]
macro_rules! dispatch {
    ($mach:ident, $MTy:ident, { $([$pub:tt$(($krate:tt))*])* fn $name:ident($($arg:ident: $argty:ty),* $(,)*) -> $ret:ty $body:block }) => {
        #[inline(always)]
        $($pub$(($krate))*)* fn $name($($arg: $argty),*) -> $ret {
            let $mach = unsafe { $crate::generic::GenericMachine::instance() };
            #[inline(always)]
            fn fn_impl<$MTy: $crate::Machine>($mach: $MTy, $($arg: $argty),*) -> $ret $body
            fn_impl($mach, $($arg),*)
        }
    };
    ($mach:ident, $MTy:ident, { $([$pub:tt $(($krate:tt))*])* fn $name:ident($($arg:ident: $argty:ty),* $(,)*) $body:block }) => {
        dispatch!($mach, $MTy, {
            $([$pub $(($krate))*])* fn $name($($arg: $argty),*) -> () $body
        });
    }
}
#[macro_export]
macro_rules! dispatch_light128 {
    ($mach:ident, $MTy:ident, { $([$pub:tt$(($krate:tt))*])* fn $name:ident($($arg:ident: $argty:ty),* $(,)*) -> $ret:ty $body:block }) => {
        #[inline(always)]
        $($pub$(($krate))*)* fn $name($($arg: $argty),*) -> $ret {
            let $mach = unsafe { $crate::generic::GenericMachine::instance() };
            #[inline(always)]
            fn fn_impl<$MTy: $crate::Machine>($mach: $MTy, $($arg: $argty),*) -> $ret $body
            fn_impl($mach, $($arg),*)
        }
    };
    ($mach:ident, $MTy:ident, { $([$pub:tt $(($krate:tt))*])* fn $name:ident($($arg:ident: $argty:ty),* $(,)*) $body:block }) => {
        dispatch!($mach, $MTy, {
            $([$pub $(($krate))*])* fn $name($($arg: $argty),*) -> () $body
        });
    }
}
#[macro_export]
macro_rules! dispatch_light256 {
    ($mach:ident, $MTy:ident, { $([$pub:tt$(($krate:tt))*])* fn $name:ident($($arg:ident: $argty:ty),* $(,)*) -> $ret:ty $body:block }) => {
        #[inline(always)]
        $($pub$(($krate))*)* fn $name($($arg: $argty),*) -> $ret {
            let $mach = unsafe { $crate::generic::GenericMachine::instance() };
            #[inline(always)]
            fn fn_impl<$MTy: $crate::Machine>($mach: $MTy, $($arg: $argty),*) -> $ret $body
            fn_impl($mach, $($arg),*)
        }
    };
    ($mach:ident, $MTy:ident, { $([$pub:tt $(($krate:tt))*])* fn $name:ident($($arg:ident: $argty:ty),* $(,)*) $body:block }) => {
        dispatch!($mach, $MTy, {
            $([$pub $(($krate))*])* fn $name($($arg: $argty),*) -> () $body
        });
    }
}
#[macro_export]
macro_rules! dispatch_light512 {
    ($mach:ident, $MTy:ident, { $([$pub:tt$(($krate:tt))*])* fn $name:ident($($arg:ident: $argty:ty),* $(,)*) -> $ret:ty $body:block }) => {
        #[inline(always)]
        $($pub$(($krate))*)* fn $name($($arg: $argty),*) -> $ret {
            let $mach = unsafe { $crate::generic::GenericMachine::instance() };
            #[inline(always)]
            fn fn_impl<$MTy: $crate::Machine>($mach: $MTy, $($arg: $argty),*) -> $ret $body
            fn_impl($mach, $($arg),*)
        }
    };
    ($mach:ident, $MTy:ident, { $([$pub:tt $(($krate:tt))*])* fn $name:ident($($arg:ident: $argty:ty),* $(,)*) $body:block }) => {
        dispatch!($mach, $MTy, {
            $([$pub $(($krate))*])* fn $name($($arg: $argty),*) -> () $body
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_bswap32() {
        let xs = [0x0f0e_0d0c, 0x0b0a_0908, 0x0706_0504, 0x0302_0100];
        let ys = [0x0c0d_0e0f, 0x0809_0a0b, 0x0405_0607, 0x0001_0203];

        let m = unsafe { GenericMachine::instance() };

        let x: <GenericMachine as Machine>::u32x4 = m.vec(xs);
        let x = x.bswap();

        let y = m.vec(ys);
        assert_eq!(x, y);
    }
}
