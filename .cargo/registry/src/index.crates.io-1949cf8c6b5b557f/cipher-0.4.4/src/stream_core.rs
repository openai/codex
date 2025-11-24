use crate::{ParBlocks, ParBlocksSizeUser, StreamCipherError};
use crypto_common::{
    generic_array::{ArrayLength, GenericArray},
    typenum::Unsigned,
    Block, BlockSizeUser,
};
use inout::{InOut, InOutBuf};

/// Trait implemented by stream cipher backends.
pub trait StreamBackend: ParBlocksSizeUser {
    /// Generate keystream block.
    fn gen_ks_block(&mut self, block: &mut Block<Self>);

    /// Generate keystream blocks in parallel.
    #[inline(always)]
    fn gen_par_ks_blocks(&mut self, blocks: &mut ParBlocks<Self>) {
        for block in blocks {
            self.gen_ks_block(block);
        }
    }

    /// Generate keystream blocks. Length of the buffer MUST be smaller
    /// than `Self::ParBlocksSize`.
    #[inline(always)]
    fn gen_tail_blocks(&mut self, blocks: &mut [Block<Self>]) {
        assert!(blocks.len() < Self::ParBlocksSize::USIZE);
        for block in blocks {
            self.gen_ks_block(block);
        }
    }
}

/// Trait for [`StreamBackend`] users.
///
/// This trait is used to define rank-2 closures.
pub trait StreamClosure: BlockSizeUser {
    /// Execute closure with the provided stream cipher backend.
    fn call<B: StreamBackend<BlockSize = Self::BlockSize>>(self, backend: &mut B);
}

/// Block-level synchronous stream ciphers.
pub trait StreamCipherCore: BlockSizeUser + Sized {
    /// Return number of remaining blocks before cipher wraps around.
    ///
    /// Returns `None` if number of remaining blocks can not be computed
    /// (e.g. in ciphers based on the sponge construction) or it's too big
    /// to fit into `usize`.
    fn remaining_blocks(&self) -> Option<usize>;

    /// Process data using backend provided to the rank-2 closure.
    fn process_with_backend(&mut self, f: impl StreamClosure<BlockSize = Self::BlockSize>);

    /// Write keystream block.
    ///
    /// WARNING: this method does not check number of remaining blocks!
    #[inline]
    fn write_keystream_block(&mut self, block: &mut Block<Self>) {
        self.process_with_backend(WriteBlockCtx { block });
    }

    /// Write keystream blocks.
    ///
    /// WARNING: this method does not check number of remaining blocks!
    #[inline]
    fn write_keystream_blocks(&mut self, blocks: &mut [Block<Self>]) {
        self.process_with_backend(WriteBlocksCtx { blocks });
    }

    /// Apply keystream block.
    ///
    /// WARNING: this method does not check number of remaining blocks!
    #[inline]
    fn apply_keystream_block_inout(&mut self, block: InOut<'_, '_, Block<Self>>) {
        self.process_with_backend(ApplyBlockCtx { block });
    }

    /// Apply keystream blocks.
    ///
    /// WARNING: this method does not check number of remaining blocks!
    #[inline]
    fn apply_keystream_blocks(&mut self, blocks: &mut [Block<Self>]) {
        self.process_with_backend(ApplyBlocksCtx {
            blocks: blocks.into(),
        });
    }

    /// Apply keystream blocks.
    ///
    /// WARNING: this method does not check number of remaining blocks!
    #[inline]
    fn apply_keystream_blocks_inout(&mut self, blocks: InOutBuf<'_, '_, Block<Self>>) {
        self.process_with_backend(ApplyBlocksCtx { blocks });
    }

    /// Try to apply keystream to data not divided into blocks.
    ///
    /// Consumes cipher since it may consume final keystream block only
    /// partially.
    ///
    /// Returns an error if number of remaining blocks is not sufficient
    /// for processing the input data.
    #[inline]
    fn try_apply_keystream_partial(
        mut self,
        mut buf: InOutBuf<'_, '_, u8>,
    ) -> Result<(), StreamCipherError> {
        if let Some(rem) = self.remaining_blocks() {
            let blocks = if buf.len() % Self::BlockSize::USIZE == 0 {
                buf.len() % Self::BlockSize::USIZE
            } else {
                buf.len() % Self::BlockSize::USIZE + 1
            };
            if blocks > rem {
                return Err(StreamCipherError);
            }
        }

        if buf.len() > Self::BlockSize::USIZE {
            let (blocks, tail) = buf.into_chunks();
            self.apply_keystream_blocks_inout(blocks);
            buf = tail;
        }
        let n = buf.len();
        if n == 0 {
            return Ok(());
        }
        let mut block = Block::<Self>::default();
        block[..n].copy_from_slice(buf.get_in());
        let t = InOutBuf::from_mut(&mut block);
        self.apply_keystream_blocks_inout(t);
        buf.get_out().copy_from_slice(&block[..n]);
        Ok(())
    }

    /// Try to apply keystream to data not divided into blocks.
    ///
    /// Consumes cipher since it may consume final keystream block only
    /// partially.
    ///
    /// # Panics
    /// If number of remaining blocks is not sufficient for processing the
    /// input data.
    #[inline]
    fn apply_keystream_partial(self, buf: InOutBuf<'_, '_, u8>) {
        self.try_apply_keystream_partial(buf).unwrap()
    }
}

// note: unfortunately, currently we can not write blanket impls of
// `BlockEncryptMut` and `BlockDecryptMut` for `T: StreamCipherCore`
// since it requires mutually exclusive traits, see:
// https://github.com/rust-lang/rfcs/issues/1053

/// Counter type usable with [`StreamCipherCore`].
///
/// This trait is implemented for `i32`, `u32`, `u64`, `u128`, and `usize`.
/// It's not intended to be implemented in third-party crates, but doing so
/// is not forbidden.
pub trait Counter:
    TryFrom<i32>
    + TryFrom<u32>
    + TryFrom<u64>
    + TryFrom<u128>
    + TryFrom<usize>
    + TryInto<i32>
    + TryInto<u32>
    + TryInto<u64>
    + TryInto<u128>
    + TryInto<usize>
{
}

/// Block-level seeking trait for stream ciphers.
pub trait StreamCipherSeekCore: StreamCipherCore {
    /// Counter type used inside stream cipher.
    type Counter: Counter;

    /// Get current block position.
    fn get_block_pos(&self) -> Self::Counter;

    /// Set block position.
    fn set_block_pos(&mut self, pos: Self::Counter);
}

macro_rules! impl_counter {
    {$($t:ty )*} => {
        $( impl Counter for $t { } )*
    };
}

impl_counter! { u32 u64 u128 }

/// Partition buffer into 2 parts: buffer of arrays and tail.
///
/// In case if `N` is less or equal to 1, buffer of arrays has length
/// of zero and tail is equal to `self`.
#[inline]
fn into_chunks<T, N: ArrayLength<T>>(buf: &mut [T]) -> (&mut [GenericArray<T, N>], &mut [T]) {
    use core::slice;
    if N::USIZE <= 1 {
        return (&mut [], buf);
    }
    let chunks_len = buf.len() / N::USIZE;
    let tail_pos = N::USIZE * chunks_len;
    let tail_len = buf.len() - tail_pos;
    unsafe {
        let ptr = buf.as_mut_ptr();
        let chunks = slice::from_raw_parts_mut(ptr as *mut GenericArray<T, N>, chunks_len);
        let tail = slice::from_raw_parts_mut(ptr.add(tail_pos), tail_len);
        (chunks, tail)
    }
}

struct WriteBlockCtx<'a, BS: ArrayLength<u8>> {
    block: &'a mut Block<Self>,
}
impl<'a, BS: ArrayLength<u8>> BlockSizeUser for WriteBlockCtx<'a, BS> {
    type BlockSize = BS;
}
impl<'a, BS: ArrayLength<u8>> StreamClosure for WriteBlockCtx<'a, BS> {
    #[inline(always)]
    fn call<B: StreamBackend<BlockSize = BS>>(self, backend: &mut B) {
        backend.gen_ks_block(self.block);
    }
}

struct WriteBlocksCtx<'a, BS: ArrayLength<u8>> {
    blocks: &'a mut [Block<Self>],
}
impl<'a, BS: ArrayLength<u8>> BlockSizeUser for WriteBlocksCtx<'a, BS> {
    type BlockSize = BS;
}
impl<'a, BS: ArrayLength<u8>> StreamClosure for WriteBlocksCtx<'a, BS> {
    #[inline(always)]
    fn call<B: StreamBackend<BlockSize = BS>>(self, backend: &mut B) {
        if B::ParBlocksSize::USIZE > 1 {
            let (chunks, tail) = into_chunks::<_, B::ParBlocksSize>(self.blocks);
            for chunk in chunks {
                backend.gen_par_ks_blocks(chunk);
            }
            backend.gen_tail_blocks(tail);
        } else {
            for block in self.blocks {
                backend.gen_ks_block(block);
            }
        }
    }
}

struct ApplyBlockCtx<'inp, 'out, BS: ArrayLength<u8>> {
    block: InOut<'inp, 'out, Block<Self>>,
}

impl<'inp, 'out, BS: ArrayLength<u8>> BlockSizeUser for ApplyBlockCtx<'inp, 'out, BS> {
    type BlockSize = BS;
}

impl<'inp, 'out, BS: ArrayLength<u8>> StreamClosure for ApplyBlockCtx<'inp, 'out, BS> {
    #[inline(always)]
    fn call<B: StreamBackend<BlockSize = BS>>(mut self, backend: &mut B) {
        let mut t = Default::default();
        backend.gen_ks_block(&mut t);
        self.block.xor_in2out(&t);
    }
}

struct ApplyBlocksCtx<'inp, 'out, BS: ArrayLength<u8>> {
    blocks: InOutBuf<'inp, 'out, Block<Self>>,
}

impl<'inp, 'out, BS: ArrayLength<u8>> BlockSizeUser for ApplyBlocksCtx<'inp, 'out, BS> {
    type BlockSize = BS;
}

impl<'inp, 'out, BS: ArrayLength<u8>> StreamClosure for ApplyBlocksCtx<'inp, 'out, BS> {
    #[inline(always)]
    #[allow(clippy::needless_range_loop)]
    fn call<B: StreamBackend<BlockSize = BS>>(self, backend: &mut B) {
        if B::ParBlocksSize::USIZE > 1 {
            let (chunks, mut tail) = self.blocks.into_chunks::<B::ParBlocksSize>();
            for mut chunk in chunks {
                let mut tmp = Default::default();
                backend.gen_par_ks_blocks(&mut tmp);
                chunk.xor_in2out(&tmp);
            }
            let n = tail.len();
            let mut buf = GenericArray::<_, B::ParBlocksSize>::default();
            let ks = &mut buf[..n];
            backend.gen_tail_blocks(ks);
            for i in 0..n {
                tail.get(i).xor_in2out(&ks[i]);
            }
        } else {
            for mut block in self.blocks {
                let mut t = Default::default();
                backend.gen_ks_block(&mut t);
                block.xor_in2out(&t);
            }
        }
    }
}
