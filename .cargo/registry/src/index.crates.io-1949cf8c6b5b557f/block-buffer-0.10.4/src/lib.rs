//! Fixed size buffer for block processing of data.
#![no_std]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/media/6ee8e381/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/media/6ee8e381/logo.svg"
)]
#![warn(missing_docs, rust_2018_idioms)]

pub use generic_array;

use core::{fmt, marker::PhantomData, slice};
use generic_array::{
    typenum::{IsLess, Le, NonZero, U256},
    ArrayLength, GenericArray,
};

mod sealed;

/// Block on which `BlockBuffer` operates.
pub type Block<BlockSize> = GenericArray<u8, BlockSize>;

/// Trait for buffer kinds.
pub trait BufferKind: sealed::Sealed {}

/// Eager block buffer kind, which guarantees that buffer position
/// always lies in the range of `0..BlockSize`.
#[derive(Copy, Clone, Debug, Default)]
pub struct Eager {}

/// Lazy block buffer kind, which guarantees that buffer position
/// always lies in the range of `0..=BlockSize`.
#[derive(Copy, Clone, Debug, Default)]
pub struct Lazy {}

impl BufferKind for Eager {}
impl BufferKind for Lazy {}

/// Eager block buffer.
pub type EagerBuffer<B> = BlockBuffer<B, Eager>;
/// Lazy block buffer.
pub type LazyBuffer<B> = BlockBuffer<B, Lazy>;

/// Block buffer error.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Error;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.write_str("Block buffer error")
    }
}

/// Buffer for block processing of data.
#[derive(Debug)]
pub struct BlockBuffer<BlockSize, Kind>
where
    BlockSize: ArrayLength<u8> + IsLess<U256>,
    Le<BlockSize, U256>: NonZero,
    Kind: BufferKind,
{
    buffer: Block<BlockSize>,
    pos: u8,
    _pd: PhantomData<Kind>,
}

impl<BlockSize, Kind> Default for BlockBuffer<BlockSize, Kind>
where
    BlockSize: ArrayLength<u8> + IsLess<U256>,
    Le<BlockSize, U256>: NonZero,
    Kind: BufferKind,
{
    fn default() -> Self {
        if BlockSize::USIZE == 0 {
            panic!("Block size can not be equal to zero");
        }
        Self {
            buffer: Default::default(),
            pos: 0,
            _pd: PhantomData,
        }
    }
}

impl<BlockSize, Kind> Clone for BlockBuffer<BlockSize, Kind>
where
    BlockSize: ArrayLength<u8> + IsLess<U256>,
    Le<BlockSize, U256>: NonZero,
    Kind: BufferKind,
{
    fn clone(&self) -> Self {
        Self {
            buffer: self.buffer.clone(),
            pos: self.pos,
            _pd: PhantomData,
        }
    }
}

impl<BlockSize, Kind> BlockBuffer<BlockSize, Kind>
where
    BlockSize: ArrayLength<u8> + IsLess<U256>,
    Le<BlockSize, U256>: NonZero,
    Kind: BufferKind,
{
    /// Create new buffer from slice.
    ///
    /// # Panics
    /// If slice length is not valid for used buffer kind.
    #[inline(always)]
    pub fn new(buf: &[u8]) -> Self {
        Self::try_new(buf).unwrap()
    }

    /// Create new buffer from slice.
    ///
    /// Returns an error if slice length is not valid for used buffer kind.
    #[inline(always)]
    pub fn try_new(buf: &[u8]) -> Result<Self, Error> {
        if BlockSize::USIZE == 0 {
            panic!("Block size can not be equal to zero");
        }
        let pos = buf.len();
        if !Kind::invariant(pos, BlockSize::USIZE) {
            return Err(Error);
        }
        let mut buffer = Block::<BlockSize>::default();
        buffer[..pos].copy_from_slice(buf);
        Ok(Self {
            buffer,
            pos: pos as u8,
            _pd: PhantomData,
        })
    }

    /// Digest data in `input` in blocks of size `BlockSize` using
    /// the `compress` function, which accepts slice of blocks.
    #[inline]
    pub fn digest_blocks(
        &mut self,
        mut input: &[u8],
        mut compress: impl FnMut(&[Block<BlockSize>]),
    ) {
        let pos = self.get_pos();
        // using `self.remaining()` for some reason
        // prevents panic elimination
        let rem = self.size() - pos;
        let n = input.len();
        // Note that checking condition `pos + n < BlockSize` is
        // equivalent to checking `n < rem`, where `rem` is equal
        // to `BlockSize - pos`. Using the latter allows us to work
        // around compiler accounting for possible overflow of
        // `pos + n` which results in it inserting unreachable
        // panic branches. Using `unreachable_unchecked` in `get_pos`
        // we convince compiler that `BlockSize - pos` never underflows.
        if Kind::invariant(n, rem) {
            // double slicing allows to remove panic branches
            self.buffer[pos..][..n].copy_from_slice(input);
            self.set_pos_unchecked(pos + n);
            return;
        }
        if pos != 0 {
            let (left, right) = input.split_at(rem);
            input = right;
            self.buffer[pos..].copy_from_slice(left);
            compress(slice::from_ref(&self.buffer));
        }

        let (blocks, leftover) = Kind::split_blocks(input);
        if !blocks.is_empty() {
            compress(blocks);
        }

        let n = leftover.len();
        self.buffer[..n].copy_from_slice(leftover);
        self.set_pos_unchecked(n);
    }

    /// Reset buffer by setting cursor position to zero.
    #[inline(always)]
    pub fn reset(&mut self) {
        self.set_pos_unchecked(0);
    }

    /// Pad remaining data with zeros and return resulting block.
    #[inline(always)]
    pub fn pad_with_zeros(&mut self) -> &mut Block<BlockSize> {
        let pos = self.get_pos();
        self.buffer[pos..].iter_mut().for_each(|b| *b = 0);
        self.set_pos_unchecked(0);
        &mut self.buffer
    }

    /// Return current cursor position.
    #[inline(always)]
    pub fn get_pos(&self) -> usize {
        let pos = self.pos as usize;
        if !Kind::invariant(pos, BlockSize::USIZE) {
            debug_assert!(false);
            // SAFETY: `pos` never breaks the invariant
            unsafe {
                core::hint::unreachable_unchecked();
            }
        }
        pos
    }

    /// Return slice of data stored inside the buffer.
    #[inline(always)]
    pub fn get_data(&self) -> &[u8] {
        &self.buffer[..self.get_pos()]
    }

    /// Set buffer content and cursor position.
    ///
    /// # Panics
    /// If `pos` is bigger or equal to block size.
    #[inline]
    pub fn set(&mut self, buf: Block<BlockSize>, pos: usize) {
        assert!(Kind::invariant(pos, BlockSize::USIZE));
        self.buffer = buf;
        self.set_pos_unchecked(pos);
    }

    /// Return size of the internal buffer in bytes.
    #[inline(always)]
    pub fn size(&self) -> usize {
        BlockSize::USIZE
    }

    /// Return number of remaining bytes in the internal buffer.
    #[inline(always)]
    pub fn remaining(&self) -> usize {
        self.size() - self.get_pos()
    }

    #[inline(always)]
    fn set_pos_unchecked(&mut self, pos: usize) {
        debug_assert!(Kind::invariant(pos, BlockSize::USIZE));
        self.pos = pos as u8;
    }
}

impl<BlockSize> BlockBuffer<BlockSize, Eager>
where
    BlockSize: ArrayLength<u8> + IsLess<U256>,
    Le<BlockSize, U256>: NonZero,
{
    /// Set `data` to generated blocks.
    #[inline]
    pub fn set_data(
        &mut self,
        mut data: &mut [u8],
        mut process_blocks: impl FnMut(&mut [Block<BlockSize>]),
    ) {
        let pos = self.get_pos();
        let r = self.remaining();
        let n = data.len();
        if pos != 0 {
            if n < r {
                // double slicing allows to remove panic branches
                data.copy_from_slice(&self.buffer[pos..][..n]);
                self.set_pos_unchecked(pos + n);
                return;
            }
            let (left, right) = data.split_at_mut(r);
            data = right;
            left.copy_from_slice(&self.buffer[pos..]);
        }

        let (blocks, leftover) = to_blocks_mut(data);
        process_blocks(blocks);

        let n = leftover.len();
        if n != 0 {
            let mut block = Default::default();
            process_blocks(slice::from_mut(&mut block));
            leftover.copy_from_slice(&block[..n]);
            self.buffer = block;
        }
        self.set_pos_unchecked(n);
    }

    /// Compress remaining data after padding it with `delim`, zeros and
    /// the `suffix` bytes. If there is not enough unused space, `compress`
    /// will be called twice.
    ///
    /// # Panics
    /// If suffix length is bigger than block size.
    #[inline(always)]
    pub fn digest_pad(
        &mut self,
        delim: u8,
        suffix: &[u8],
        mut compress: impl FnMut(&Block<BlockSize>),
    ) {
        if suffix.len() > BlockSize::USIZE {
            panic!("suffix is too long");
        }
        let pos = self.get_pos();
        self.buffer[pos] = delim;
        for b in &mut self.buffer[pos + 1..] {
            *b = 0;
        }

        let n = self.size() - suffix.len();
        if self.size() - pos - 1 < suffix.len() {
            compress(&self.buffer);
            let mut block = Block::<BlockSize>::default();
            block[n..].copy_from_slice(suffix);
            compress(&block);
        } else {
            self.buffer[n..].copy_from_slice(suffix);
            compress(&self.buffer);
        }
        self.set_pos_unchecked(0)
    }

    /// Pad message with 0x80, zeros and 64-bit message length using
    /// big-endian byte order.
    #[inline]
    pub fn len64_padding_be(&mut self, data_len: u64, compress: impl FnMut(&Block<BlockSize>)) {
        self.digest_pad(0x80, &data_len.to_be_bytes(), compress);
    }

    /// Pad message with 0x80, zeros and 64-bit message length using
    /// little-endian byte order.
    #[inline]
    pub fn len64_padding_le(&mut self, data_len: u64, compress: impl FnMut(&Block<BlockSize>)) {
        self.digest_pad(0x80, &data_len.to_le_bytes(), compress);
    }

    /// Pad message with 0x80, zeros and 128-bit message length using
    /// big-endian byte order.
    #[inline]
    pub fn len128_padding_be(&mut self, data_len: u128, compress: impl FnMut(&Block<BlockSize>)) {
        self.digest_pad(0x80, &data_len.to_be_bytes(), compress);
    }
}

/// Split message into mutable slice of parallel blocks, blocks, and leftover bytes.
#[inline(always)]
fn to_blocks_mut<N: ArrayLength<u8>>(data: &mut [u8]) -> (&mut [Block<N>], &mut [u8]) {
    let nb = data.len() / N::USIZE;
    let (left, right) = data.split_at_mut(nb * N::USIZE);
    let p = left.as_mut_ptr() as *mut Block<N>;
    // SAFETY: we guarantee that `blocks` does not point outside of `data`, and `p` is valid for
    // mutation
    let blocks = unsafe { slice::from_raw_parts_mut(p, nb) };
    (blocks, right)
}
