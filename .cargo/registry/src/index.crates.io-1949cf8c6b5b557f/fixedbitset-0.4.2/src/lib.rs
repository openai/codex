//! `FixedBitSet` is a simple fixed size set of bits.
//!
//!
//! ### Crate features
//!
//! - `std` (default feature)  
//!   Disabling this feature disables using std and instead uses crate alloc.
//!   Requires Rust 1.36 to disable.
//!
//! ### Rust Version
//!
//! This version of fixedbitset requires Rust 1.39 or later.
//!
#![doc(html_root_url = "https://docs.rs/fixedbitset/0.4.2/")]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{vec, vec::Vec};

#[cfg(not(feature = "std"))]
use core as std;

mod range;

#[cfg(feature = "serde")]
extern crate serde;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use std::fmt::Write;
use std::fmt::{Binary, Display, Error, Formatter};

pub use range::IndexRange;
use std::cmp::{Ord, Ordering};
use std::iter::{Chain, FromIterator};
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Index};

const BITS: usize = 32;
type Block = u32;

#[inline]
fn div_rem(x: usize, d: usize) -> (usize, usize) {
    (x / d, x % d)
}

/// `FixedBitSet` is a simple fixed size set of bits that each can
/// be enabled (1 / **true**) or disabled (0 / **false**).
///
/// The bit set has a fixed capacity in terms of enabling bits (and the
/// capacity can grow using the `grow` method).
///
/// Derived traits depend on both the zeros and ones, so [0,1] is not equal to
/// [0,1,0].
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FixedBitSet {
    data: Vec<Block>,
    /// length in bits
    length: usize,
}

impl FixedBitSet {
    /// Create a new empty **FixedBitSet**.
    pub const fn new() -> Self {
        FixedBitSet {
            data: Vec::new(),
            length: 0,
        }
    }

    /// Create a new **FixedBitSet** with a specific number of bits,
    /// all initially clear.
    pub fn with_capacity(bits: usize) -> Self {
        let (mut blocks, rem) = div_rem(bits, BITS);
        blocks += (rem > 0) as usize;
        FixedBitSet {
            data: vec![0; blocks],
            length: bits,
        }
    }

    /// Create a new **FixedBitSet** with a specific number of bits,
    /// initialized from provided blocks.
    ///
    /// If the blocks are not the exact size needed for the capacity
    /// they will be padded with zeros (if shorter) or truncated to
    /// the capacity (if longer).
    ///
    /// For example:
    /// ```
    /// let data = vec![4];
    /// let bs = fixedbitset::FixedBitSet::with_capacity_and_blocks(4, data);
    /// assert_eq!(format!("{:b}", bs), "0010");
    /// ```
    pub fn with_capacity_and_blocks<I: IntoIterator<Item = Block>>(bits: usize, blocks: I) -> Self {
        let (mut n_blocks, rem) = div_rem(bits, BITS);
        n_blocks += (rem > 0) as usize;
        let mut data: Vec<Block> = blocks.into_iter().collect();
        // Pad data with zeros if smaller or truncate if larger
        if data.len() != n_blocks {
            data.resize(n_blocks, 0);
        }
        // Disable bits in blocks beyond capacity
        let end = data.len() * 32;
        for (block, mask) in Masks::new(bits..end, end) {
            unsafe {
                *data.get_unchecked_mut(block) &= !mask;
            }
        }
        FixedBitSet { data, length: bits }
    }

    /// Grow capacity to **bits**, all new bits initialized to zero
    pub fn grow(&mut self, bits: usize) {
        let (mut blocks, rem) = div_rem(bits, BITS);
        blocks += (rem > 0) as usize;
        if bits > self.length {
            self.length = bits;
            self.data.resize(blocks, 0);
        }
    }

    /// The length of the [`FixedBitSet`] in bits.
    ///
    /// Note: `len` includes both set and unset bits.
    /// ```
    /// # use fixedbitset::FixedBitSet;
    /// let bitset = FixedBitSet::with_capacity(10);
    /// // there are 0 set bits, but 10 unset bits
    /// assert_eq!(bitset.len(), 10);
    /// ```
    /// `len` does not return the count of set bits. For that, use
    /// [`bitset.count_ones(..)`](FixedBitSet::count_ones) instead.
    #[inline]
    pub fn len(&self) -> usize {
        self.length
    }

    /// `true` if the [`FixedBitSet`] is empty.
    ///
    /// Note that an "empty" `FixedBitSet` is a `FixedBitSet` with
    /// no bits (meaning: it's length is zero). If you want to check
    /// if all bits are unset, use [`FixedBitSet::is_clear`].
    ///
    /// ```
    /// # use fixedbitset::FixedBitSet;
    /// let bitset = FixedBitSet::with_capacity(10);
    /// assert!(!bitset.is_empty());
    ///
    /// let bitset = FixedBitSet::with_capacity(0);
    /// assert!(bitset.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// `true` if all bits in the [`FixedBitSet`] are unset.
    ///
    /// As opposed to [`FixedBitSet::is_empty`], which is `true` only for
    /// sets without any bits, set or unset.
    ///
    /// ```
    /// # use fixedbitset::FixedBitSet;
    /// let mut bitset = FixedBitSet::with_capacity(10);
    /// assert!(bitset.is_clear());
    ///
    /// bitset.insert(2);
    /// assert!(!bitset.is_clear());
    /// ```
    ///
    /// This is equivalent to [`bitset.count_ones(..) == 0`](FixedBitSet::count_ones).
    #[inline]
    pub fn is_clear(&self) -> bool {
        self.data.iter().all(|block| *block == 0)
    }

    /// Return **true** if the bit is enabled in the **FixedBitSet**,
    /// **false** otherwise.
    ///
    /// Note: bits outside the capacity are always disabled.
    ///
    /// Note: Also available with index syntax: `bitset[bit]`.
    #[inline]
    pub fn contains(&self, bit: usize) -> bool {
        let (block, i) = div_rem(bit, BITS);
        match self.data.get(block) {
            None => false,
            Some(b) => (b & (1 << i)) != 0,
        }
    }

    /// Clear all bits.
    #[inline]
    pub fn clear(&mut self) {
        for elt in &mut self.data[..] {
            *elt = 0
        }
    }

    /// Enable `bit`.
    ///
    /// **Panics** if **bit** is out of bounds.
    #[inline]
    pub fn insert(&mut self, bit: usize) {
        assert!(
            bit < self.length,
            "insert at index {} exceeds fixbitset size {}",
            bit,
            self.length
        );
        let (block, i) = div_rem(bit, BITS);
        unsafe {
            *self.data.get_unchecked_mut(block) |= 1 << i;
        }
    }

    /// Enable `bit`, and return its previous value.
    ///
    /// **Panics** if **bit** is out of bounds.
    #[inline]
    pub fn put(&mut self, bit: usize) -> bool {
        assert!(
            bit < self.length,
            "put at index {} exceeds fixbitset size {}",
            bit,
            self.length
        );
        let (block, i) = div_rem(bit, BITS);
        unsafe {
            let word = self.data.get_unchecked_mut(block);
            let prev = *word & (1 << i) != 0;
            *word |= 1 << i;
            prev
        }
    }
    /// Toggle `bit` (inverting its state).
    ///
    /// ***Panics*** if **bit** is out of bounds
    #[inline]
    pub fn toggle(&mut self, bit: usize) {
        assert!(
            bit < self.length,
            "toggle at index {} exceeds fixbitset size {}",
            bit,
            self.length
        );
        let (block, i) = div_rem(bit, BITS);
        unsafe {
            *self.data.get_unchecked_mut(block) ^= 1 << i;
        }
    }
    /// **Panics** if **bit** is out of bounds.
    #[inline]
    pub fn set(&mut self, bit: usize, enabled: bool) {
        assert!(
            bit < self.length,
            "set at index {} exceeds fixbitset size {}",
            bit,
            self.length
        );
        let (block, i) = div_rem(bit, BITS);
        unsafe {
            let elt = self.data.get_unchecked_mut(block);
            if enabled {
                *elt |= 1 << i;
            } else {
                *elt &= !(1 << i);
            }
        }
    }

    /// Copies boolean value from specified bit to the specified bit.
    ///
    /// **Panics** if **to** is out of bounds.
    #[inline]
    pub fn copy_bit(&mut self, from: usize, to: usize) {
        assert!(
            to < self.length,
            "copy at index {} exceeds fixbitset size {}",
            to,
            self.length
        );
        let (to_block, t) = div_rem(to, BITS);
        let enabled = self.contains(from);
        unsafe {
            let to_elt = self.data.get_unchecked_mut(to_block);
            if enabled {
                *to_elt |= 1 << t;
            } else {
                *to_elt &= !(1 << t);
            }
        }
    }

    /// Count the number of set bits in the given bit range.
    ///
    /// Use `..` to count the whole content of the bitset.
    ///
    /// **Panics** if the range extends past the end of the bitset.
    #[inline]
    pub fn count_ones<T: IndexRange>(&self, range: T) -> usize {
        Masks::new(range, self.length)
            .map(|(block, mask)| unsafe {
                let value = *self.data.get_unchecked(block);
                (value & mask).count_ones() as usize
            })
            .sum()
    }

    /// Sets every bit in the given range to the given state (`enabled`)
    ///
    /// Use `..` to set the whole bitset.
    ///
    /// **Panics** if the range extends past the end of the bitset.
    #[inline]
    pub fn set_range<T: IndexRange>(&mut self, range: T, enabled: bool) {
        for (block, mask) in Masks::new(range, self.length) {
            unsafe {
                if enabled {
                    *self.data.get_unchecked_mut(block) |= mask;
                } else {
                    *self.data.get_unchecked_mut(block) &= !mask;
                }
            }
        }
    }

    /// Enables every bit in the given range.
    ///
    /// Use `..` to make the whole bitset ones.
    ///
    /// **Panics** if the range extends past the end of the bitset.
    #[inline]
    pub fn insert_range<T: IndexRange>(&mut self, range: T) {
        self.set_range(range, true);
    }

    /// Toggles (inverts) every bit in the given range.
    ///
    /// Use `..` to toggle the whole bitset.
    ///
    /// **Panics** if the range extends past the end of the bitset.
    #[inline]
    pub fn toggle_range<T: IndexRange>(&mut self, range: T) {
        for (block, mask) in Masks::new(range, self.length) {
            unsafe {
                *self.data.get_unchecked_mut(block) ^= mask;
            }
        }
    }

    /// View the bitset as a slice of `u32` blocks
    #[inline]
    pub fn as_slice(&self) -> &[u32] {
        &self.data
    }

    /// View the bitset as a mutable slice of `u32` blocks. Writing past the bitlength in the last
    /// will cause `contains` to return potentially incorrect results for bits past the bitlength.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u32] {
        &mut self.data
    }

    /// Iterates over all enabled bits.
    ///
    /// Iterator element is the index of the `1` bit, type `usize`.
    #[inline]
    pub fn ones(&self) -> Ones {
        match self.as_slice().split_first() {
            Some((&block, rem)) => Ones {
                bitset: block,
                block_idx: 0,
                remaining_blocks: rem,
            },
            None => Ones {
                bitset: 0,
                block_idx: 0,
                remaining_blocks: &[],
            },
        }
    }

    /// Returns a lazy iterator over the intersection of two `FixedBitSet`s
    pub fn intersection<'a>(&'a self, other: &'a FixedBitSet) -> Intersection<'a> {
        Intersection {
            iter: self.ones(),
            other,
        }
    }

    /// Returns a lazy iterator over the union of two `FixedBitSet`s.
    pub fn union<'a>(&'a self, other: &'a FixedBitSet) -> Union<'a> {
        Union {
            iter: self.ones().chain(other.difference(self)),
        }
    }

    /// Returns a lazy iterator over the difference of two `FixedBitSet`s. The difference of `a`
    /// and `b` is the elements of `a` which are not in `b`.
    pub fn difference<'a>(&'a self, other: &'a FixedBitSet) -> Difference<'a> {
        Difference {
            iter: self.ones(),
            other,
        }
    }

    /// Returns a lazy iterator over the symmetric difference of two `FixedBitSet`s.
    /// The symmetric difference of `a` and `b` is the elements of one, but not both, sets.
    pub fn symmetric_difference<'a>(&'a self, other: &'a FixedBitSet) -> SymmetricDifference<'a> {
        SymmetricDifference {
            iter: self.difference(other).chain(other.difference(self)),
        }
    }

    /// In-place union of two `FixedBitSet`s.
    ///
    /// On calling this method, `self`'s capacity may be increased to match `other`'s.
    pub fn union_with(&mut self, other: &FixedBitSet) {
        if other.len() >= self.len() {
            self.grow(other.len());
        }
        for (x, y) in self.data.iter_mut().zip(other.data.iter()) {
            *x |= *y;
        }
    }

    /// In-place intersection of two `FixedBitSet`s.
    ///
    /// On calling this method, `self`'s capacity will remain the same as before.
    pub fn intersect_with(&mut self, other: &FixedBitSet) {
        for (x, y) in self.data.iter_mut().zip(other.data.iter()) {
            *x &= *y;
        }
        let mn = std::cmp::min(self.data.len(), other.data.len());
        for wd in &mut self.data[mn..] {
            *wd = 0;
        }
    }

    /// In-place difference of two `FixedBitSet`s.
    ///
    /// On calling this method, `self`'s capacity will remain the same as before.
    pub fn difference_with(&mut self, other: &FixedBitSet) {
        for (x, y) in self.data.iter_mut().zip(other.data.iter()) {
            *x &= !*y;
        }

        // There's no need to grow self or do any other adjustments.
        //
        // * If self is longer than other, the bits at the end of self won't be affected since other
        //   has them implicitly set to 0.
        // * If other is longer than self, the bits at the end of other are irrelevant since self
        //   has them set to 0 anyway.
    }

    /// In-place symmetric difference of two `FixedBitSet`s.
    ///
    /// On calling this method, `self`'s capacity may be increased to match `other`'s.
    pub fn symmetric_difference_with(&mut self, other: &FixedBitSet) {
        if other.len() >= self.len() {
            self.grow(other.len());
        }
        for (x, y) in self.data.iter_mut().zip(other.data.iter()) {
            *x ^= *y;
        }
    }

    /// Returns `true` if `self` has no elements in common with `other`. This
    /// is equivalent to checking for an empty intersection.
    pub fn is_disjoint(&self, other: &FixedBitSet) -> bool {
        self.data
            .iter()
            .zip(other.data.iter())
            .all(|(x, y)| x & y == 0)
    }

    /// Returns `true` if the set is a subset of another, i.e. `other` contains
    /// at least all the values in `self`.
    pub fn is_subset(&self, other: &FixedBitSet) -> bool {
        self.data
            .iter()
            .zip(other.data.iter())
            .all(|(x, y)| x & !y == 0)
            && self.data.iter().skip(other.data.len()).all(|x| *x == 0)
    }

    /// Returns `true` if the set is a superset of another, i.e. `self` contains
    /// at least all the values in `other`.
    pub fn is_superset(&self, other: &FixedBitSet) -> bool {
        other.is_subset(self)
    }
}

impl Binary for FixedBitSet {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        if f.alternate() {
            f.write_str("0b")?;
        }

        for i in 0..self.length {
            if self[i] {
                f.write_char('1')?;
            } else {
                f.write_char('0')?;
            }
        }

        Ok(())
    }
}

impl Display for FixedBitSet {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        Binary::fmt(&self, f)
    }
}

/// An iterator producing elements in the difference of two sets.
///
/// This struct is created by the [`FixedBitSet::difference`] method.
pub struct Difference<'a> {
    iter: Ones<'a>,
    other: &'a FixedBitSet,
}

impl<'a> Iterator for Difference<'a> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(nxt) = self.iter.next() {
            if !self.other.contains(nxt) {
                return Some(nxt);
            }
        }
        None
    }
}

/// An iterator producing elements in the symmetric difference of two sets.
///
/// This struct is created by the [`FixedBitSet::symmetric_difference`] method.
pub struct SymmetricDifference<'a> {
    iter: Chain<Difference<'a>, Difference<'a>>,
}

impl<'a> Iterator for SymmetricDifference<'a> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// An iterator producing elements in the intersection of two sets.
///
/// This struct is created by the [`FixedBitSet::intersection`] method.
pub struct Intersection<'a> {
    iter: Ones<'a>,
    other: &'a FixedBitSet,
}

impl<'a> Iterator for Intersection<'a> {
    type Item = usize; // the bit position of the '1'

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(nxt) = self.iter.next() {
            if self.other.contains(nxt) {
                return Some(nxt);
            }
        }
        None
    }
}

/// An iterator producing elements in the union of two sets.
///
/// This struct is created by the [`FixedBitSet::union`] method.
pub struct Union<'a> {
    iter: Chain<Ones<'a>, Difference<'a>>,
}

impl<'a> Iterator for Union<'a> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

struct Masks {
    first_block: usize,
    first_mask: Block,
    last_block: usize,
    last_mask: Block,
}

impl Masks {
    #[inline]
    fn new<T: IndexRange>(range: T, length: usize) -> Masks {
        let start = range.start().unwrap_or(0);
        let end = range.end().unwrap_or(length);
        assert!(
            start <= end && end <= length,
            "invalid range {}..{} for a fixedbitset of size {}",
            start,
            end,
            length
        );

        let (first_block, first_rem) = div_rem(start, BITS);
        let (last_block, last_rem) = div_rem(end, BITS);

        Masks {
            first_block: first_block as usize,
            first_mask: Block::max_value() << first_rem,
            last_block: last_block as usize,
            last_mask: (Block::max_value() >> 1) >> (BITS - last_rem - 1),
            // this is equivalent to `MAX >> (BITS - x)` with correct semantics when x == 0.
        }
    }
}

impl Iterator for Masks {
    type Item = (usize, Block);
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.first_block.cmp(&self.last_block) {
            Ordering::Less => {
                let res = (self.first_block, self.first_mask);
                self.first_block += 1;
                self.first_mask = !0;
                Some(res)
            }
            Ordering::Equal => {
                let mask = self.first_mask & self.last_mask;
                let res = if mask == 0 {
                    None
                } else {
                    Some((self.first_block, mask))
                };
                self.first_block += 1;
                res
            }
            Ordering::Greater => None,
        }
    }
}

/// An  iterator producing the indices of the set bit in a set.
///
/// This struct is created by the [`FixedBitSet::ones`] method.
pub struct Ones<'a> {
    bitset: Block,
    block_idx: usize,
    remaining_blocks: &'a [Block],
}

impl<'a> Iterator for Ones<'a> {
    type Item = usize; // the bit position of the '1'

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while self.bitset == 0 {
            if self.remaining_blocks.is_empty() {
                return None;
            }
            self.bitset = self.remaining_blocks[0];
            self.remaining_blocks = &self.remaining_blocks[1..];
            self.block_idx += 1;
        }
        let t = self.bitset & (0 as Block).wrapping_sub(self.bitset);
        let r = self.bitset.trailing_zeros() as usize;
        self.bitset ^= t;
        Some(self.block_idx * BITS + r)
    }
}

impl Clone for FixedBitSet {
    #[inline]
    fn clone(&self) -> Self {
        FixedBitSet {
            data: self.data.clone(),
            length: self.length,
        }
    }
}

/// Return **true** if the bit is enabled in the bitset,
/// or **false** otherwise.
///
/// Note: bits outside the capacity are always disabled, and thus
/// indexing a FixedBitSet will not panic.
impl Index<usize> for FixedBitSet {
    type Output = bool;

    #[inline]
    fn index(&self, bit: usize) -> &bool {
        if self.contains(bit) {
            &true
        } else {
            &false
        }
    }
}

/// Sets the bit at index **i** to **true** for each item **i** in the input **src**.
impl Extend<usize> for FixedBitSet {
    fn extend<I: IntoIterator<Item = usize>>(&mut self, src: I) {
        let iter = src.into_iter();
        for i in iter {
            if i >= self.len() {
                self.grow(i + 1);
            }
            self.put(i);
        }
    }
}

/// Return a FixedBitSet containing bits set to **true** for every bit index in
/// the iterator, other bits are set to **false**.
impl FromIterator<usize> for FixedBitSet {
    fn from_iter<I: IntoIterator<Item = usize>>(src: I) -> Self {
        let mut fbs = FixedBitSet::with_capacity(0);
        fbs.extend(src);
        fbs
    }
}

impl<'a> BitAnd for &'a FixedBitSet {
    type Output = FixedBitSet;
    fn bitand(self, other: &FixedBitSet) -> FixedBitSet {
        let (short, long) = {
            if self.len() <= other.len() {
                (&self.data, &other.data)
            } else {
                (&other.data, &self.data)
            }
        };
        let mut data = short.clone();
        for (data, block) in data.iter_mut().zip(long.iter()) {
            *data &= *block;
        }
        let len = std::cmp::min(self.len(), other.len());
        FixedBitSet { data, length: len }
    }
}

impl<'a> BitAndAssign for FixedBitSet {
    fn bitand_assign(&mut self, other: Self) {
        self.intersect_with(&other);
    }
}

impl<'a> BitAndAssign<&Self> for FixedBitSet {
    fn bitand_assign(&mut self, other: &Self) {
        self.intersect_with(other);
    }
}

impl<'a> BitOr for &'a FixedBitSet {
    type Output = FixedBitSet;
    fn bitor(self, other: &FixedBitSet) -> FixedBitSet {
        let (short, long) = {
            if self.len() <= other.len() {
                (&self.data, &other.data)
            } else {
                (&other.data, &self.data)
            }
        };
        let mut data = long.clone();
        for (data, block) in data.iter_mut().zip(short.iter()) {
            *data |= *block;
        }
        let len = std::cmp::max(self.len(), other.len());
        FixedBitSet { data, length: len }
    }
}

impl<'a> BitOrAssign for FixedBitSet {
    fn bitor_assign(&mut self, other: Self) {
        self.union_with(&other);
    }
}

impl<'a> BitOrAssign<&Self> for FixedBitSet {
    fn bitor_assign(&mut self, other: &Self) {
        self.union_with(other);
    }
}

impl<'a> BitXor for &'a FixedBitSet {
    type Output = FixedBitSet;
    fn bitxor(self, other: &FixedBitSet) -> FixedBitSet {
        let (short, long) = {
            if self.len() <= other.len() {
                (&self.data, &other.data)
            } else {
                (&other.data, &self.data)
            }
        };
        let mut data = long.clone();
        for (data, block) in data.iter_mut().zip(short.iter()) {
            *data ^= *block;
        }
        let len = std::cmp::max(self.len(), other.len());
        FixedBitSet { data, length: len }
    }
}

impl<'a> BitXorAssign for FixedBitSet {
    fn bitxor_assign(&mut self, other: Self) {
        self.symmetric_difference_with(&other);
    }
}

impl<'a> BitXorAssign<&Self> for FixedBitSet {
    fn bitxor_assign(&mut self, other: &Self) {
        self.symmetric_difference_with(other);
    }
}

#[test]
fn it_works() {
    const N: usize = 50;
    let mut fb = FixedBitSet::with_capacity(N);

    for i in 0..(N + 10) {
        assert_eq!(fb.contains(i), false);
    }

    fb.insert(10);
    fb.set(11, false);
    fb.set(12, false);
    fb.set(12, true);
    fb.set(N - 1, true);

    assert!(fb.contains(10));
    assert!(!fb.contains(11));
    assert!(fb.contains(12));
    assert!(fb.contains(N - 1));
    for i in 0..N {
        let contain = i == 10 || i == 12 || i == N - 1;
        assert_eq!(contain, fb[i]);
    }

    fb.clear();
}

#[test]
fn with_blocks() {
    let fb = FixedBitSet::with_capacity_and_blocks(50, vec![8u32, 0u32]);
    assert!(fb.contains(3));

    let ones: Vec<_> = fb.ones().collect();
    assert_eq!(ones.len(), 1);
}

#[test]
fn with_blocks_too_small() {
    let mut fb = FixedBitSet::with_capacity_and_blocks(500, vec![8u32, 0u32]);
    fb.insert(400);
    assert!(fb.contains(400));
}

#[test]
fn with_blocks_too_big() {
    let fb = FixedBitSet::with_capacity_and_blocks(1, vec![8u32]);

    // since capacity is 1, 3 shouldn't be set here
    assert!(!fb.contains(3));
}

#[test]
fn with_blocks_too_big_range_check() {
    let fb = FixedBitSet::with_capacity_and_blocks(1, vec![0xff]);

    // since capacity is 1, only 0 should be set
    assert!(fb.contains(0));
    for i in 1..0xff {
        assert!(!fb.contains(i));
    }
}

#[test]
fn grow() {
    let mut fb = FixedBitSet::with_capacity(48);
    for i in 0..fb.len() {
        fb.set(i, true);
    }

    let old_len = fb.len();
    fb.grow(72);
    for j in 0..fb.len() {
        assert_eq!(fb.contains(j), j < old_len);
    }
    fb.set(64, true);
    assert!(fb.contains(64));
}

#[test]
fn test_toggle() {
    let mut fb = FixedBitSet::with_capacity(16);
    fb.toggle(1);
    fb.put(2);
    fb.toggle(2);
    fb.put(3);
    assert!(fb.contains(1));
    assert!(!fb.contains(2));
    assert!(fb.contains(3));
}

#[test]
fn copy_bit() {
    let mut fb = FixedBitSet::with_capacity(48);
    for i in 0..fb.len() {
        fb.set(i, true);
    }
    fb.set(42, false);
    fb.copy_bit(42, 2);
    assert!(!fb.contains(42));
    assert!(!fb.contains(2));
    assert!(fb.contains(1));
    fb.copy_bit(1, 42);
    assert!(fb.contains(42));
    fb.copy_bit(1024, 42);
    assert!(!fb[42]);
}

#[test]
fn count_ones() {
    let mut fb = FixedBitSet::with_capacity(100);
    fb.set(11, true);
    fb.set(12, true);
    fb.set(7, true);
    fb.set(35, true);
    fb.set(40, true);
    fb.set(77, true);
    fb.set(95, true);
    fb.set(50, true);
    fb.set(99, true);
    assert_eq!(fb.count_ones(..7), 0);
    assert_eq!(fb.count_ones(..8), 1);
    assert_eq!(fb.count_ones(..11), 1);
    assert_eq!(fb.count_ones(..12), 2);
    assert_eq!(fb.count_ones(..13), 3);
    assert_eq!(fb.count_ones(..35), 3);
    assert_eq!(fb.count_ones(..36), 4);
    assert_eq!(fb.count_ones(..40), 4);
    assert_eq!(fb.count_ones(..41), 5);
    assert_eq!(fb.count_ones(50..), 4);
    assert_eq!(fb.count_ones(70..95), 1);
    assert_eq!(fb.count_ones(70..96), 2);
    assert_eq!(fb.count_ones(70..99), 2);
    assert_eq!(fb.count_ones(..), 9);
    assert_eq!(fb.count_ones(0..100), 9);
    assert_eq!(fb.count_ones(0..0), 0);
    assert_eq!(fb.count_ones(100..100), 0);
    assert_eq!(fb.count_ones(7..), 9);
    assert_eq!(fb.count_ones(8..), 8);
}

#[test]
fn ones() {
    let mut fb = FixedBitSet::with_capacity(100);
    fb.set(11, true);
    fb.set(12, true);
    fb.set(7, true);
    fb.set(35, true);
    fb.set(40, true);
    fb.set(77, true);
    fb.set(95, true);
    fb.set(50, true);
    fb.set(99, true);

    let ones: Vec<_> = fb.ones().collect();

    assert_eq!(vec![7, 11, 12, 35, 40, 50, 77, 95, 99], ones);
}

#[test]
fn iter_ones_range() {
    fn test_range(from: usize, to: usize, capa: usize) {
        assert!(to <= capa);
        let mut fb = FixedBitSet::with_capacity(capa);
        for i in from..to {
            fb.insert(i);
        }
        let ones: Vec<_> = fb.ones().collect();
        let expected: Vec<_> = (from..to).collect();
        assert_eq!(expected, ones);
    }

    for i in 0..100 {
        test_range(i, 100, 100);
        test_range(0, i, 100);
    }
}

#[should_panic]
#[test]
fn count_ones_oob() {
    let fb = FixedBitSet::with_capacity(100);
    fb.count_ones(90..101);
}

#[should_panic]
#[test]
fn count_ones_negative_range() {
    let fb = FixedBitSet::with_capacity(100);
    fb.count_ones(90..80);
}

#[test]
fn count_ones_panic() {
    for i in 1..128 {
        let fb = FixedBitSet::with_capacity(i);
        for j in 0..fb.len() + 1 {
            for k in j..fb.len() + 1 {
                assert_eq!(fb.count_ones(j..k), 0);
            }
        }
    }
}

#[test]
fn default() {
    let fb = FixedBitSet::default();
    assert_eq!(fb.len(), 0);
}

#[test]
fn insert_range() {
    let mut fb = FixedBitSet::with_capacity(97);
    fb.insert_range(..3);
    fb.insert_range(9..32);
    fb.insert_range(37..81);
    fb.insert_range(90..);
    for i in 0..97 {
        assert_eq!(
            fb.contains(i),
            i < 3 || 9 <= i && i < 32 || 37 <= i && i < 81 || 90 <= i
        );
    }
    assert!(!fb.contains(97));
    assert!(!fb.contains(127));
    assert!(!fb.contains(128));
}

#[test]
fn set_range() {
    let mut fb = FixedBitSet::with_capacity(48);
    fb.insert_range(..);

    fb.set_range(..32, false);
    fb.set_range(37.., false);
    fb.set_range(5..9, true);
    fb.set_range(40..40, true);

    for i in 0..48 {
        assert_eq!(fb.contains(i), 5 <= i && i < 9 || 32 <= i && i < 37);
    }
    assert!(!fb.contains(48));
    assert!(!fb.contains(64));
}

#[test]
fn toggle_range() {
    let mut fb = FixedBitSet::with_capacity(40);
    fb.insert_range(..10);
    fb.insert_range(34..38);

    fb.toggle_range(5..12);
    fb.toggle_range(30..);

    for i in 0..40 {
        assert_eq!(
            fb.contains(i),
            i < 5 || 10 <= i && i < 12 || 30 <= i && i < 34 || 38 <= i
        );
    }
    assert!(!fb.contains(40));
    assert!(!fb.contains(64));
}

#[test]
fn bitand_equal_lengths() {
    let len = 109;
    let a_end = 59;
    let b_start = 23;
    let mut a = FixedBitSet::with_capacity(len);
    let mut b = FixedBitSet::with_capacity(len);
    a.set_range(..a_end, true);
    b.set_range(b_start.., true);
    let ab = &a & &b;
    for i in 0..b_start {
        assert!(!ab.contains(i));
    }
    for i in b_start..a_end {
        assert!(ab.contains(i));
    }
    for i in a_end..len {
        assert!(!ab.contains(i));
    }
    assert_eq!(a.len(), ab.len());
}

#[test]
fn bitand_first_smaller() {
    let a_len = 113;
    let b_len = 137;
    let len = std::cmp::min(a_len, b_len);
    let a_end = 97;
    let b_start = 89;
    let mut a = FixedBitSet::with_capacity(a_len);
    let mut b = FixedBitSet::with_capacity(b_len);
    a.set_range(..a_end, true);
    b.set_range(b_start.., true);
    let ab = &a & &b;
    for i in 0..b_start {
        assert!(!ab.contains(i));
    }
    for i in b_start..a_end {
        assert!(ab.contains(i));
    }
    for i in a_end..len {
        assert!(!ab.contains(i));
    }
    assert_eq!(a.len(), ab.len());
}

#[test]
fn bitand_first_larger() {
    let a_len = 173;
    let b_len = 137;
    let len = std::cmp::min(a_len, b_len);
    let a_end = 107;
    let b_start = 43;
    let mut a = FixedBitSet::with_capacity(a_len);
    let mut b = FixedBitSet::with_capacity(b_len);
    a.set_range(..a_end, true);
    b.set_range(b_start.., true);
    let ab = &a & &b;
    for i in 0..b_start {
        assert!(!ab.contains(i));
    }
    for i in b_start..a_end {
        assert!(ab.contains(i));
    }
    for i in a_end..len {
        assert!(!ab.contains(i));
    }
    assert_eq!(b.len(), ab.len());
}

#[test]
fn intersection() {
    let len = 109;
    let a_end = 59;
    let b_start = 23;
    let mut a = FixedBitSet::with_capacity(len);
    let mut b = FixedBitSet::with_capacity(len);
    a.set_range(..a_end, true);
    b.set_range(b_start.., true);

    let mut ab = a.intersection(&b).collect::<FixedBitSet>();

    for i in 0..b_start {
        assert!(!ab.contains(i));
    }
    for i in b_start..a_end {
        assert!(ab.contains(i));
    }
    for i in a_end..len {
        assert!(!ab.contains(i));
    }

    a.intersect_with(&b);
    // intersection + collect produces the same results but with a shorter length.
    ab.grow(a.len());
    assert_eq!(
        ab, a,
        "intersection and intersect_with produce the same results"
    );
}

#[test]
fn union() {
    let a_len = 173;
    let b_len = 137;
    let a_start = 139;
    let b_end = 107;
    let mut a = FixedBitSet::with_capacity(a_len);
    let mut b = FixedBitSet::with_capacity(b_len);
    a.set_range(a_start.., true);
    b.set_range(..b_end, true);
    let ab = a.union(&b).collect::<FixedBitSet>();
    for i in a_start..a_len {
        assert!(ab.contains(i));
    }
    for i in 0..b_end {
        assert!(ab.contains(i));
    }
    for i in b_end..a_start {
        assert!(!ab.contains(i));
    }

    a.union_with(&b);
    assert_eq!(ab, a, "union and union_with produce the same results");
}

#[test]
fn difference() {
    let a_len = 83;
    let b_len = 151;
    let a_start = 0;
    let a_end = 79;
    let b_start = 53;
    let mut a = FixedBitSet::with_capacity(a_len);
    let mut b = FixedBitSet::with_capacity(b_len);
    a.set_range(a_start..a_end, true);
    b.set_range(b_start..b_len, true);
    let mut a_diff_b = a.difference(&b).collect::<FixedBitSet>();
    for i in a_start..b_start {
        assert!(a_diff_b.contains(i));
    }
    for i in b_start..b_len {
        assert!(!a_diff_b.contains(i));
    }

    a.difference_with(&b);
    // difference + collect produces the same results but with a shorter length.
    a_diff_b.grow(a.len());
    assert_eq!(
        a_diff_b, a,
        "difference and difference_with produce the same results"
    );
}

#[test]
fn symmetric_difference() {
    let a_len = 83;
    let b_len = 151;
    let a_start = 47;
    let a_end = 79;
    let b_start = 53;
    let mut a = FixedBitSet::with_capacity(a_len);
    let mut b = FixedBitSet::with_capacity(b_len);
    a.set_range(a_start..a_end, true);
    b.set_range(b_start..b_len, true);
    let a_sym_diff_b = a.symmetric_difference(&b).collect::<FixedBitSet>();
    for i in 0..a_start {
        assert!(!a_sym_diff_b.contains(i));
    }
    for i in a_start..b_start {
        assert!(a_sym_diff_b.contains(i));
    }
    for i in b_start..a_end {
        assert!(!a_sym_diff_b.contains(i));
    }
    for i in a_end..b_len {
        assert!(a_sym_diff_b.contains(i));
    }

    a.symmetric_difference_with(&b);
    assert_eq!(
        a_sym_diff_b, a,
        "symmetric_difference and _with produce the same results"
    );
}

#[test]
fn bitor_equal_lengths() {
    let len = 109;
    let a_start = 17;
    let a_end = 23;
    let b_start = 19;
    let b_end = 59;
    let mut a = FixedBitSet::with_capacity(len);
    let mut b = FixedBitSet::with_capacity(len);
    a.set_range(a_start..a_end, true);
    b.set_range(b_start..b_end, true);
    let ab = &a | &b;
    for i in 0..a_start {
        assert!(!ab.contains(i));
    }
    for i in a_start..b_end {
        assert!(ab.contains(i));
    }
    for i in b_end..len {
        assert!(!ab.contains(i));
    }
    assert_eq!(ab.len(), len);
}

#[test]
fn bitor_first_smaller() {
    let a_len = 113;
    let b_len = 137;
    let a_end = 89;
    let b_start = 97;
    let mut a = FixedBitSet::with_capacity(a_len);
    let mut b = FixedBitSet::with_capacity(b_len);
    a.set_range(..a_end, true);
    b.set_range(b_start.., true);
    let ab = &a | &b;
    for i in 0..a_end {
        assert!(ab.contains(i));
    }
    for i in a_end..b_start {
        assert!(!ab.contains(i));
    }
    for i in b_start..b_len {
        assert!(ab.contains(i));
    }
    assert_eq!(b_len, ab.len());
}

#[test]
fn bitor_first_larger() {
    let a_len = 173;
    let b_len = 137;
    let a_start = 139;
    let b_end = 107;
    let mut a = FixedBitSet::with_capacity(a_len);
    let mut b = FixedBitSet::with_capacity(b_len);
    a.set_range(a_start.., true);
    b.set_range(..b_end, true);
    let ab = &a | &b;
    for i in a_start..a_len {
        assert!(ab.contains(i));
    }
    for i in 0..b_end {
        assert!(ab.contains(i));
    }
    for i in b_end..a_start {
        assert!(!ab.contains(i));
    }
    assert_eq!(a_len, ab.len());
}

#[test]
fn bitxor_equal_lengths() {
    let len = 109;
    let a_end = 59;
    let b_start = 23;
    let mut a = FixedBitSet::with_capacity(len);
    let mut b = FixedBitSet::with_capacity(len);
    a.set_range(..a_end, true);
    b.set_range(b_start.., true);
    let ab = &a ^ &b;
    for i in 0..b_start {
        assert!(ab.contains(i));
    }
    for i in b_start..a_end {
        assert!(!ab.contains(i));
    }
    for i in a_end..len {
        assert!(ab.contains(i));
    }
    assert_eq!(a.len(), ab.len());
}

#[test]
fn bitxor_first_smaller() {
    let a_len = 113;
    let b_len = 137;
    let len = std::cmp::max(a_len, b_len);
    let a_end = 97;
    let b_start = 89;
    let mut a = FixedBitSet::with_capacity(a_len);
    let mut b = FixedBitSet::with_capacity(b_len);
    a.set_range(..a_end, true);
    b.set_range(b_start.., true);
    let ab = &a ^ &b;
    for i in 0..b_start {
        assert!(ab.contains(i));
    }
    for i in b_start..a_end {
        assert!(!ab.contains(i));
    }
    for i in a_end..len {
        assert!(ab.contains(i));
    }
    assert_eq!(b.len(), ab.len());
}

#[test]
fn bitxor_first_larger() {
    let a_len = 173;
    let b_len = 137;
    let len = std::cmp::max(a_len, b_len);
    let a_end = 107;
    let b_start = 43;
    let mut a = FixedBitSet::with_capacity(a_len);
    let mut b = FixedBitSet::with_capacity(b_len);
    a.set_range(..a_end, true);
    b.set_range(b_start.., true);
    let ab = &a ^ &b;
    for i in 0..b_start {
        assert!(ab.contains(i));
    }
    for i in b_start..a_end {
        assert!(!ab.contains(i));
    }
    for i in a_end..b_len {
        assert!(ab.contains(i));
    }
    for i in b_len..len {
        assert!(!ab.contains(i));
    }
    assert_eq!(a.len(), ab.len());
}

#[test]
fn bitand_assign_shorter() {
    let a_ones: Vec<usize> = vec![2, 3, 7, 19, 31, 32, 37, 41, 43, 47, 71, 73, 101];
    let b_ones: Vec<usize> = vec![2, 7, 8, 11, 23, 31, 32];
    let a_and_b: Vec<usize> = vec![2, 7, 31, 32];
    let mut a = a_ones.iter().cloned().collect::<FixedBitSet>();
    let b = b_ones.iter().cloned().collect::<FixedBitSet>();
    a &= b;
    let res = a.ones().collect::<Vec<usize>>();

    assert!(res == a_and_b);
}

#[test]
fn bitand_assign_longer() {
    let a_ones: Vec<usize> = vec![2, 7, 8, 11, 23, 31, 32];
    let b_ones: Vec<usize> = vec![2, 3, 7, 19, 31, 32, 37, 41, 43, 47, 71, 73, 101];
    let a_and_b: Vec<usize> = vec![2, 7, 31, 32];
    let mut a = a_ones.iter().cloned().collect::<FixedBitSet>();
    let b = b_ones.iter().cloned().collect::<FixedBitSet>();
    a &= b;
    let res = a.ones().collect::<Vec<usize>>();
    assert!(res == a_and_b);
}

#[test]
fn bitor_assign_shorter() {
    let a_ones: Vec<usize> = vec![2, 3, 7, 19, 31, 32, 37, 41, 43, 47, 71, 73, 101];
    let b_ones: Vec<usize> = vec![2, 7, 8, 11, 23, 31, 32];
    let a_or_b: Vec<usize> = vec![2, 3, 7, 8, 11, 19, 23, 31, 32, 37, 41, 43, 47, 71, 73, 101];
    let mut a = a_ones.iter().cloned().collect::<FixedBitSet>();
    let b = b_ones.iter().cloned().collect::<FixedBitSet>();
    a |= b;
    let res = a.ones().collect::<Vec<usize>>();
    assert!(res == a_or_b);
}

#[test]
fn bitor_assign_longer() {
    let a_ones: Vec<usize> = vec![2, 7, 8, 11, 23, 31, 32];
    let b_ones: Vec<usize> = vec![2, 3, 7, 19, 31, 32, 37, 41, 43, 47, 71, 73, 101];
    let a_or_b: Vec<usize> = vec![2, 3, 7, 8, 11, 19, 23, 31, 32, 37, 41, 43, 47, 71, 73, 101];
    let mut a = a_ones.iter().cloned().collect::<FixedBitSet>();
    let b = b_ones.iter().cloned().collect::<FixedBitSet>();
    a |= b;
    let res = a.ones().collect::<Vec<usize>>();
    assert!(res == a_or_b);
}

#[test]
fn bitxor_assign_shorter() {
    let a_ones: Vec<usize> = vec![2, 3, 7, 19, 31, 32, 37, 41, 43, 47, 71, 73, 101];
    let b_ones: Vec<usize> = vec![2, 7, 8, 11, 23, 31, 32];
    let a_xor_b: Vec<usize> = vec![3, 8, 11, 19, 23, 37, 41, 43, 47, 71, 73, 101];
    let mut a = a_ones.iter().cloned().collect::<FixedBitSet>();
    let b = b_ones.iter().cloned().collect::<FixedBitSet>();
    a ^= b;
    let res = a.ones().collect::<Vec<usize>>();
    assert!(res == a_xor_b);
}

#[test]
fn bitxor_assign_longer() {
    let a_ones: Vec<usize> = vec![2, 7, 8, 11, 23, 31, 32];
    let b_ones: Vec<usize> = vec![2, 3, 7, 19, 31, 32, 37, 41, 43, 47, 71, 73, 101];
    let a_xor_b: Vec<usize> = vec![3, 8, 11, 19, 23, 37, 41, 43, 47, 71, 73, 101];
    let mut a = a_ones.iter().cloned().collect::<FixedBitSet>();
    let b = b_ones.iter().cloned().collect::<FixedBitSet>();
    a ^= b;
    let res = a.ones().collect::<Vec<usize>>();
    assert!(res == a_xor_b);
}

#[test]
fn op_assign_ref() {
    let mut a = FixedBitSet::with_capacity(8);
    let b = FixedBitSet::with_capacity(8);

    //check that all assign type operators work on references
    a &= &b;
    a |= &b;
    a ^= &b;
}

#[test]
fn subset_superset_shorter() {
    let a_ones: Vec<usize> = vec![7, 31, 32, 63];
    let b_ones: Vec<usize> = vec![2, 7, 19, 31, 32, 37, 41, 43, 47, 63, 73, 101];
    let mut a = a_ones.iter().cloned().collect::<FixedBitSet>();
    let b = b_ones.iter().cloned().collect::<FixedBitSet>();
    assert!(a.is_subset(&b) && b.is_superset(&a));
    a.insert(14);
    assert!(!a.is_subset(&b) && !b.is_superset(&a));
}

#[test]
fn subset_superset_longer() {
    let a_len = 153;
    let b_len = 75;
    let a_ones: Vec<usize> = vec![7, 31, 32, 63];
    let b_ones: Vec<usize> = vec![2, 7, 19, 31, 32, 37, 41, 43, 47, 63, 73];
    let mut a = FixedBitSet::with_capacity(a_len);
    let mut b = FixedBitSet::with_capacity(b_len);
    a.extend(a_ones.iter().cloned());
    b.extend(b_ones.iter().cloned());
    assert!(a.is_subset(&b) && b.is_superset(&a));
    a.insert(100);
    assert!(!a.is_subset(&b) && !b.is_superset(&a));
}

#[test]
fn is_disjoint_first_shorter() {
    let a_len = 75;
    let b_len = 153;
    let a_ones: Vec<usize> = vec![2, 19, 32, 37, 41, 43, 47, 73];
    let b_ones: Vec<usize> = vec![7, 23, 31, 63, 124];
    let mut a = FixedBitSet::with_capacity(a_len);
    let mut b = FixedBitSet::with_capacity(b_len);
    a.extend(a_ones.iter().cloned());
    b.extend(b_ones.iter().cloned());
    assert!(a.is_disjoint(&b));
    a.insert(63);
    assert!(!a.is_disjoint(&b));
}

#[test]
fn is_disjoint_first_longer() {
    let a_ones: Vec<usize> = vec![2, 19, 32, 37, 41, 43, 47, 73, 101];
    let b_ones: Vec<usize> = vec![7, 23, 31, 63];
    let a = a_ones.iter().cloned().collect::<FixedBitSet>();
    let mut b = b_ones.iter().cloned().collect::<FixedBitSet>();
    assert!(a.is_disjoint(&b));
    b.insert(2);
    assert!(!a.is_disjoint(&b));
}

#[test]
fn extend_on_empty() {
    let items: Vec<usize> = vec![2, 3, 5, 7, 11, 13, 17, 19, 23, 27, 29, 31, 37, 167];
    let mut fbs = FixedBitSet::with_capacity(0);
    fbs.extend(items.iter().cloned());
    let ones = fbs.ones().collect::<Vec<usize>>();
    assert!(ones == items);
}

#[test]
fn extend() {
    let items: Vec<usize> = vec![2, 3, 5, 7, 11, 13, 17, 19, 23, 27, 29, 31, 37, 167];
    let mut fbs = FixedBitSet::with_capacity(168);
    let new: Vec<usize> = vec![7, 37, 67, 137];
    for i in &new {
        fbs.put(*i);
    }

    fbs.extend(items.iter().cloned());

    let ones = fbs.ones().collect::<Vec<usize>>();
    let expected = {
        let mut tmp = items.clone();
        tmp.extend(new);
        tmp.sort();
        tmp.dedup();
        tmp
    };

    assert!(ones == expected);
}

#[test]
fn from_iterator() {
    let items: Vec<usize> = vec![0, 2, 4, 6, 8];
    let fb = items.iter().cloned().collect::<FixedBitSet>();
    for i in items {
        assert!(fb.contains(i));
    }
    for i in vec![1, 3, 5, 7] {
        assert!(!fb.contains(i));
    }
    assert_eq!(fb.len(), 9);
}

#[test]
fn from_iterator_ones() {
    let len = 257;
    let mut fb = FixedBitSet::with_capacity(len);
    for i in (0..len).filter(|i| i % 7 == 0) {
        fb.put(i);
    }
    fb.put(len - 1);
    let dup = fb.ones().collect::<FixedBitSet>();

    assert_eq!(fb.len(), dup.len());
    assert_eq!(
        fb.ones().collect::<Vec<usize>>(),
        dup.ones().collect::<Vec<usize>>()
    );
}

#[cfg(feature = "std")]
#[test]
fn binary_trait() {
    let items: Vec<usize> = vec![1, 5, 7, 10, 14, 15];
    let fb = items.iter().cloned().collect::<FixedBitSet>();

    assert_eq!(format!("{:b}", fb), "0100010100100011");
    assert_eq!(format!("{:#b}", fb), "0b0100010100100011");
}

#[cfg(feature = "std")]
#[test]
fn display_trait() {
    let len = 8;
    let mut fb = FixedBitSet::with_capacity(len);

    fb.put(4);
    fb.put(2);

    assert_eq!(format!("{}", fb), "00101000");
    assert_eq!(format!("{:#}", fb), "0b00101000");
}

#[test]
#[cfg(feature = "serde")]
fn test_serialize() {
    let mut fb = FixedBitSet::with_capacity(10);
    fb.put(2);
    fb.put(3);
    fb.put(6);
    fb.put(8);
    let serialized = serde_json::to_string(&fb).unwrap();
    assert_eq!(r#"{"data":[332],"length":10}"#, serialized);
}

#[test]
fn test_is_clear() {
    let mut fb = FixedBitSet::with_capacity(0);
    assert!(fb.is_clear());

    fb.grow(1);
    assert!(fb.is_clear());

    fb.put(0);
    assert!(!fb.is_clear());

    fb.grow(42);
    fb.clear();
    assert!(fb.is_clear());

    fb.put(17);
    fb.put(19);
    assert!(!fb.is_clear());
}