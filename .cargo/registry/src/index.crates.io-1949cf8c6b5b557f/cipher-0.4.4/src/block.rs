//! Traits used to define functionality of [block ciphers][1] and [modes of operation][2].
//!
//! # About block ciphers
//!
//! Block ciphers are keyed, deterministic permutations of a fixed-sized input
//! "block" providing a reversible transformation to/from an encrypted output.
//! They are one of the fundamental structural components of [symmetric cryptography][3].
//!
//! [1]: https://en.wikipedia.org/wiki/Block_cipher
//! [2]: https://en.wikipedia.org/wiki/Block_cipher_mode_of_operation
//! [3]: https://en.wikipedia.org/wiki/Symmetric-key_algorithm

use crate::{ParBlocks, ParBlocksSizeUser};
#[cfg(all(feature = "block-padding", feature = "alloc"))]
use alloc::{vec, vec::Vec};
#[cfg(feature = "block-padding")]
use inout::{
    block_padding::{Padding, UnpadError},
    InOutBufReserved, PadError,
};
use inout::{InOut, InOutBuf, NotEqualError};

pub use crypto_common::{generic_array::ArrayLength, typenum::Unsigned, Block, BlockSizeUser};

/// Marker trait for block ciphers.
pub trait BlockCipher: BlockSizeUser {}

/// Trait implemented by block cipher encryption and decryption backends.
pub trait BlockBackend: ParBlocksSizeUser {
    /// Process single inout block.
    fn proc_block(&mut self, block: InOut<'_, '_, Block<Self>>);

    /// Process inout blocks in parallel.
    #[inline(always)]
    fn proc_par_blocks(&mut self, mut blocks: InOut<'_, '_, ParBlocks<Self>>) {
        for i in 0..Self::ParBlocksSize::USIZE {
            self.proc_block(blocks.get(i));
        }
    }

    /// Process buffer of inout blocks. Length of the buffer MUST be smaller
    /// than `Self::ParBlocksSize`.
    #[inline(always)]
    fn proc_tail_blocks(&mut self, blocks: InOutBuf<'_, '_, Block<Self>>) {
        assert!(blocks.len() < Self::ParBlocksSize::USIZE);
        for block in blocks {
            self.proc_block(block);
        }
    }

    /// Process single block in-place.
    #[inline(always)]
    fn proc_block_inplace(&mut self, block: &mut Block<Self>) {
        self.proc_block(block.into());
    }

    /// Process blocks in parallel in-place.
    #[inline(always)]
    fn proc_par_blocks_inplace(&mut self, blocks: &mut ParBlocks<Self>) {
        self.proc_par_blocks(blocks.into());
    }

    /// Process buffer of blocks in-place. Length of the buffer MUST be smaller
    /// than `Self::ParBlocksSize`.
    #[inline(always)]
    fn proc_tail_blocks_inplace(&mut self, blocks: &mut [Block<Self>]) {
        self.proc_tail_blocks(blocks.into());
    }
}

/// Trait for [`BlockBackend`] users.
///
/// This trait is used to define rank-2 closures.
pub trait BlockClosure: BlockSizeUser {
    /// Execute closure with the provided block cipher backend.
    fn call<B: BlockBackend<BlockSize = Self::BlockSize>>(self, backend: &mut B);
}

/// Encrypt-only functionality for block ciphers.
pub trait BlockEncrypt: BlockSizeUser + Sized {
    /// Encrypt data using backend provided to the rank-2 closure.
    fn encrypt_with_backend(&self, f: impl BlockClosure<BlockSize = Self::BlockSize>);

    /// Encrypt single `inout` block.
    #[inline]
    fn encrypt_block_inout(&self, block: InOut<'_, '_, Block<Self>>) {
        self.encrypt_with_backend(BlockCtx { block });
    }

    /// Encrypt `inout` blocks.
    #[inline]
    fn encrypt_blocks_inout(&self, blocks: InOutBuf<'_, '_, Block<Self>>) {
        self.encrypt_with_backend(BlocksCtx { blocks });
    }

    /// Encrypt single block in-place.
    #[inline]
    fn encrypt_block(&self, block: &mut Block<Self>) {
        let block = block.into();
        self.encrypt_with_backend(BlockCtx { block });
    }

    /// Encrypt `in_block` and write result to `out_block`.
    #[inline]
    fn encrypt_block_b2b(&self, in_block: &Block<Self>, out_block: &mut Block<Self>) {
        let block = (in_block, out_block).into();
        self.encrypt_with_backend(BlockCtx { block });
    }

    /// Encrypt blocks in-place.
    #[inline]
    fn encrypt_blocks(&self, blocks: &mut [Block<Self>]) {
        let blocks = blocks.into();
        self.encrypt_with_backend(BlocksCtx { blocks });
    }

    /// Encrypt blocks buffer-to-buffer.
    ///
    /// Returns [`NotEqualError`] if provided `in_blocks` and `out_blocks`
    /// have different lengths.
    #[inline]
    fn encrypt_blocks_b2b(
        &self,
        in_blocks: &[Block<Self>],
        out_blocks: &mut [Block<Self>],
    ) -> Result<(), NotEqualError> {
        InOutBuf::new(in_blocks, out_blocks)
            .map(|blocks| self.encrypt_with_backend(BlocksCtx { blocks }))
    }

    /// Pad input and encrypt. Returns resulting ciphertext slice.
    ///
    /// Returns [`PadError`] if length of output buffer is not sufficient.
    #[cfg(feature = "block-padding")]
    #[cfg_attr(docsrs, doc(cfg(feature = "block-padding")))]
    #[inline]
    fn encrypt_padded_inout<'inp, 'out, P: Padding<Self::BlockSize>>(
        &self,
        data: InOutBufReserved<'inp, 'out, u8>,
    ) -> Result<&'out [u8], PadError> {
        let mut buf = data.into_padded_blocks::<P, Self::BlockSize>()?;
        self.encrypt_blocks_inout(buf.get_blocks());
        if let Some(block) = buf.get_tail_block() {
            self.encrypt_block_inout(block);
        }
        Ok(buf.into_out())
    }

    /// Pad input and encrypt in-place. Returns resulting ciphertext slice.
    ///
    /// Returns [`PadError`] if length of output buffer is not sufficient.
    #[cfg(feature = "block-padding")]
    #[cfg_attr(docsrs, doc(cfg(feature = "block-padding")))]
    #[inline]
    fn encrypt_padded<'a, P: Padding<Self::BlockSize>>(
        &self,
        buf: &'a mut [u8],
        msg_len: usize,
    ) -> Result<&'a [u8], PadError> {
        let buf = InOutBufReserved::from_mut_slice(buf, msg_len).map_err(|_| PadError)?;
        self.encrypt_padded_inout::<P>(buf)
    }

    /// Pad input and encrypt buffer-to-buffer. Returns resulting ciphertext slice.
    ///
    /// Returns [`PadError`] if length of output buffer is not sufficient.
    #[cfg(feature = "block-padding")]
    #[cfg_attr(docsrs, doc(cfg(feature = "block-padding")))]
    #[inline]
    fn encrypt_padded_b2b<'a, P: Padding<Self::BlockSize>>(
        &self,
        msg: &[u8],
        out_buf: &'a mut [u8],
    ) -> Result<&'a [u8], PadError> {
        let buf = InOutBufReserved::from_slices(msg, out_buf).map_err(|_| PadError)?;
        self.encrypt_padded_inout::<P>(buf)
    }

    /// Pad input and encrypt into a newly allocated Vec. Returns resulting ciphertext Vec.
    #[cfg(all(feature = "block-padding", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "block-padding", feature = "alloc"))))]
    #[inline]
    fn encrypt_padded_vec<P: Padding<Self::BlockSize>>(&self, msg: &[u8]) -> Vec<u8> {
        let mut out = allocate_out_vec::<Self>(msg.len());
        let len = self
            .encrypt_padded_b2b::<P>(msg, &mut out)
            .expect("enough space for encrypting is allocated")
            .len();
        out.truncate(len);
        out
    }
}

/// Decrypt-only functionality for block ciphers.
pub trait BlockDecrypt: BlockSizeUser {
    /// Decrypt data using backend provided to the rank-2 closure.
    fn decrypt_with_backend(&self, f: impl BlockClosure<BlockSize = Self::BlockSize>);

    /// Decrypt single `inout` block.
    #[inline]
    fn decrypt_block_inout(&self, block: InOut<'_, '_, Block<Self>>) {
        self.decrypt_with_backend(BlockCtx { block });
    }

    /// Decrypt `inout` blocks.
    #[inline]
    fn decrypt_blocks_inout(&self, blocks: InOutBuf<'_, '_, Block<Self>>) {
        self.decrypt_with_backend(BlocksCtx { blocks });
    }

    /// Decrypt single block in-place.
    #[inline]
    fn decrypt_block(&self, block: &mut Block<Self>) {
        let block = block.into();
        self.decrypt_with_backend(BlockCtx { block });
    }

    /// Decrypt `in_block` and write result to `out_block`.
    #[inline]
    fn decrypt_block_b2b(&self, in_block: &Block<Self>, out_block: &mut Block<Self>) {
        let block = (in_block, out_block).into();
        self.decrypt_with_backend(BlockCtx { block });
    }

    /// Decrypt blocks in-place.
    #[inline]
    fn decrypt_blocks(&self, blocks: &mut [Block<Self>]) {
        let blocks = blocks.into();
        self.decrypt_with_backend(BlocksCtx { blocks });
    }

    /// Decrypt blocks buffer-to-buffer.
    ///
    /// Returns [`NotEqualError`] if provided `in_blocks` and `out_blocks`
    /// have different lengths.
    #[inline]
    fn decrypt_blocks_b2b(
        &self,
        in_blocks: &[Block<Self>],
        out_blocks: &mut [Block<Self>],
    ) -> Result<(), NotEqualError> {
        InOutBuf::new(in_blocks, out_blocks)
            .map(|blocks| self.decrypt_with_backend(BlocksCtx { blocks }))
    }

    /// Decrypt input and unpad it. Returns resulting ciphertext slice.
    ///
    /// Returns [`UnpadError`] if padding is malformed or if input length is
    /// not multiple of `Self::BlockSize`.
    #[cfg(feature = "block-padding")]
    #[cfg_attr(docsrs, doc(cfg(feature = "block-padding")))]
    #[inline]
    fn decrypt_padded_inout<'inp, 'out, P: Padding<Self::BlockSize>>(
        &self,
        data: InOutBuf<'inp, 'out, u8>,
    ) -> Result<&'out [u8], UnpadError> {
        let (mut blocks, tail) = data.into_chunks();
        if !tail.is_empty() {
            return Err(UnpadError);
        }
        self.decrypt_blocks_inout(blocks.reborrow());
        P::unpad_blocks(blocks.into_out())
    }

    /// Decrypt input and unpad it in-place. Returns resulting ciphertext slice.
    ///
    /// Returns [`UnpadError`] if padding is malformed or if input length is
    /// not multiple of `Self::BlockSize`.
    #[cfg(feature = "block-padding")]
    #[cfg_attr(docsrs, doc(cfg(feature = "block-padding")))]
    #[inline]
    fn decrypt_padded<'a, P: Padding<Self::BlockSize>>(
        &self,
        buf: &'a mut [u8],
    ) -> Result<&'a [u8], UnpadError> {
        self.decrypt_padded_inout::<P>(buf.into())
    }

    /// Decrypt input and unpad it buffer-to-buffer. Returns resulting
    /// ciphertext slice.
    ///
    /// Returns [`UnpadError`] if padding is malformed or if input length is
    /// not multiple of `Self::BlockSize`.
    #[cfg(feature = "block-padding")]
    #[cfg_attr(docsrs, doc(cfg(feature = "block-padding")))]
    #[inline]
    fn decrypt_padded_b2b<'a, P: Padding<Self::BlockSize>>(
        &self,
        in_buf: &[u8],
        out_buf: &'a mut [u8],
    ) -> Result<&'a [u8], UnpadError> {
        if out_buf.len() < in_buf.len() {
            return Err(UnpadError);
        }
        let n = in_buf.len();
        // note: `new` always returns `Ok` here
        let buf = InOutBuf::new(in_buf, &mut out_buf[..n]).map_err(|_| UnpadError)?;
        self.decrypt_padded_inout::<P>(buf)
    }

    /// Decrypt input and unpad it in a newly allocated Vec. Returns resulting
    /// ciphertext Vec.
    ///
    /// Returns [`UnpadError`] if padding is malformed or if input length is
    /// not multiple of `Self::BlockSize`.
    #[cfg(all(feature = "block-padding", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "block-padding", feature = "alloc"))))]
    #[inline]
    fn decrypt_padded_vec<P: Padding<Self::BlockSize>>(
        &self,
        buf: &[u8],
    ) -> Result<Vec<u8>, UnpadError> {
        let mut out = vec![0; buf.len()];
        let len = self.decrypt_padded_b2b::<P>(buf, &mut out)?.len();
        out.truncate(len);
        Ok(out)
    }
}

/// Encrypt-only functionality for block ciphers and modes with mutable access to `self`.
///
/// The main use case for this trait is blocks modes, but it also can be used
/// for hardware cryptographic engines which require `&mut self` access to an
/// underlying hardware peripheral.
pub trait BlockEncryptMut: BlockSizeUser + Sized {
    /// Encrypt data using backend provided to the rank-2 closure.
    fn encrypt_with_backend_mut(&mut self, f: impl BlockClosure<BlockSize = Self::BlockSize>);

    /// Encrypt single `inout` block.
    #[inline]
    fn encrypt_block_inout_mut(&mut self, block: InOut<'_, '_, Block<Self>>) {
        self.encrypt_with_backend_mut(BlockCtx { block });
    }

    /// Encrypt `inout` blocks.
    #[inline]
    fn encrypt_blocks_inout_mut(&mut self, blocks: InOutBuf<'_, '_, Block<Self>>) {
        self.encrypt_with_backend_mut(BlocksCtx { blocks });
    }

    /// Encrypt single block in-place.
    #[inline]
    fn encrypt_block_mut(&mut self, block: &mut Block<Self>) {
        let block = block.into();
        self.encrypt_with_backend_mut(BlockCtx { block });
    }

    /// Encrypt `in_block` and write result to `out_block`.
    #[inline]
    fn encrypt_block_b2b_mut(&mut self, in_block: &Block<Self>, out_block: &mut Block<Self>) {
        let block = (in_block, out_block).into();
        self.encrypt_with_backend_mut(BlockCtx { block });
    }

    /// Encrypt blocks in-place.
    #[inline]
    fn encrypt_blocks_mut(&mut self, blocks: &mut [Block<Self>]) {
        let blocks = blocks.into();
        self.encrypt_with_backend_mut(BlocksCtx { blocks });
    }

    /// Encrypt blocks buffer-to-buffer.
    ///
    /// Returns [`NotEqualError`] if provided `in_blocks` and `out_blocks`
    /// have different lengths.
    #[inline]
    fn encrypt_blocks_b2b_mut(
        &mut self,
        in_blocks: &[Block<Self>],
        out_blocks: &mut [Block<Self>],
    ) -> Result<(), NotEqualError> {
        InOutBuf::new(in_blocks, out_blocks)
            .map(|blocks| self.encrypt_with_backend_mut(BlocksCtx { blocks }))
    }

    /// Pad input and encrypt. Returns resulting ciphertext slice.
    ///
    /// Returns [`PadError`] if length of output buffer is not sufficient.
    #[cfg(feature = "block-padding")]
    #[cfg_attr(docsrs, doc(cfg(feature = "block-padding")))]
    #[inline]
    fn encrypt_padded_inout_mut<'inp, 'out, P: Padding<Self::BlockSize>>(
        mut self,
        data: InOutBufReserved<'inp, 'out, u8>,
    ) -> Result<&'out [u8], PadError> {
        let mut buf = data.into_padded_blocks::<P, Self::BlockSize>()?;
        self.encrypt_blocks_inout_mut(buf.get_blocks());
        if let Some(block) = buf.get_tail_block() {
            self.encrypt_block_inout_mut(block);
        }
        Ok(buf.into_out())
    }

    /// Pad input and encrypt in-place. Returns resulting ciphertext slice.
    ///
    /// Returns [`PadError`] if length of output buffer is not sufficient.
    #[cfg(feature = "block-padding")]
    #[cfg_attr(docsrs, doc(cfg(feature = "block-padding")))]
    #[inline]
    fn encrypt_padded_mut<P: Padding<Self::BlockSize>>(
        self,
        buf: &mut [u8],
        msg_len: usize,
    ) -> Result<&[u8], PadError> {
        let buf = InOutBufReserved::from_mut_slice(buf, msg_len).map_err(|_| PadError)?;
        self.encrypt_padded_inout_mut::<P>(buf)
    }

    /// Pad input and encrypt buffer-to-buffer. Returns resulting ciphertext slice.
    ///
    /// Returns [`PadError`] if length of output buffer is not sufficient.
    #[cfg(feature = "block-padding")]
    #[cfg_attr(docsrs, doc(cfg(feature = "block-padding")))]
    #[inline]
    fn encrypt_padded_b2b_mut<'a, P: Padding<Self::BlockSize>>(
        self,
        msg: &[u8],
        out_buf: &'a mut [u8],
    ) -> Result<&'a [u8], PadError> {
        let buf = InOutBufReserved::from_slices(msg, out_buf).map_err(|_| PadError)?;
        self.encrypt_padded_inout_mut::<P>(buf)
    }

    /// Pad input and encrypt into a newly allocated Vec. Returns resulting ciphertext Vec.
    #[cfg(all(feature = "block-padding", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "block-padding", feature = "alloc"))))]
    #[inline]
    fn encrypt_padded_vec_mut<P: Padding<Self::BlockSize>>(self, msg: &[u8]) -> Vec<u8> {
        let mut out = allocate_out_vec::<Self>(msg.len());
        let len = self
            .encrypt_padded_b2b_mut::<P>(msg, &mut out)
            .expect("enough space for encrypting is allocated")
            .len();
        out.truncate(len);
        out
    }
}

/// Decrypt-only functionality for block ciphers and modes with mutable access to `self`.
///
/// The main use case for this trait is blocks modes, but it also can be used
/// for hardware cryptographic engines which require `&mut self` access to an
/// underlying hardware peripheral.
pub trait BlockDecryptMut: BlockSizeUser + Sized {
    /// Decrypt data using backend provided to the rank-2 closure.
    fn decrypt_with_backend_mut(&mut self, f: impl BlockClosure<BlockSize = Self::BlockSize>);

    /// Decrypt single `inout` block.
    #[inline]
    fn decrypt_block_inout_mut(&mut self, block: InOut<'_, '_, Block<Self>>) {
        self.decrypt_with_backend_mut(BlockCtx { block });
    }

    /// Decrypt `inout` blocks.
    #[inline]
    fn decrypt_blocks_inout_mut(&mut self, blocks: InOutBuf<'_, '_, Block<Self>>) {
        self.decrypt_with_backend_mut(BlocksCtx { blocks });
    }

    /// Decrypt single block in-place.
    #[inline]
    fn decrypt_block_mut(&mut self, block: &mut Block<Self>) {
        let block = block.into();
        self.decrypt_with_backend_mut(BlockCtx { block });
    }

    /// Decrypt `in_block` and write result to `out_block`.
    #[inline]
    fn decrypt_block_b2b_mut(&mut self, in_block: &Block<Self>, out_block: &mut Block<Self>) {
        let block = (in_block, out_block).into();
        self.decrypt_with_backend_mut(BlockCtx { block });
    }

    /// Decrypt blocks in-place.
    #[inline]
    fn decrypt_blocks_mut(&mut self, blocks: &mut [Block<Self>]) {
        let blocks = blocks.into();
        self.decrypt_with_backend_mut(BlocksCtx { blocks });
    }

    /// Decrypt blocks buffer-to-buffer.
    ///
    /// Returns [`NotEqualError`] if provided `in_blocks` and `out_blocks`
    /// have different lengths.
    #[inline]
    fn decrypt_blocks_b2b_mut(
        &mut self,
        in_blocks: &[Block<Self>],
        out_blocks: &mut [Block<Self>],
    ) -> Result<(), NotEqualError> {
        InOutBuf::new(in_blocks, out_blocks)
            .map(|blocks| self.decrypt_with_backend_mut(BlocksCtx { blocks }))
    }

    /// Decrypt input and unpad it. Returns resulting ciphertext slice.
    ///
    /// Returns [`UnpadError`] if padding is malformed or if input length is
    /// not multiple of `Self::BlockSize`.
    #[cfg(feature = "block-padding")]
    #[cfg_attr(docsrs, doc(cfg(feature = "block-padding")))]
    #[inline]
    fn decrypt_padded_inout_mut<'inp, 'out, P: Padding<Self::BlockSize>>(
        mut self,
        data: InOutBuf<'inp, 'out, u8>,
    ) -> Result<&'out [u8], UnpadError> {
        let (mut blocks, tail) = data.into_chunks();
        if !tail.is_empty() {
            return Err(UnpadError);
        }
        self.decrypt_blocks_inout_mut(blocks.reborrow());
        P::unpad_blocks(blocks.into_out())
    }

    /// Decrypt input and unpad it in-place. Returns resulting ciphertext slice.
    ///
    /// Returns [`UnpadError`] if padding is malformed or if input length is
    /// not multiple of `Self::BlockSize`.
    #[cfg(feature = "block-padding")]
    #[cfg_attr(docsrs, doc(cfg(feature = "block-padding")))]
    #[inline]
    fn decrypt_padded_mut<P: Padding<Self::BlockSize>>(
        self,
        buf: &mut [u8],
    ) -> Result<&[u8], UnpadError> {
        self.decrypt_padded_inout_mut::<P>(buf.into())
    }

    /// Decrypt input and unpad it buffer-to-buffer. Returns resulting
    /// ciphertext slice.
    ///
    /// Returns [`UnpadError`] if padding is malformed or if input length is
    /// not multiple of `Self::BlockSize`.
    #[cfg(feature = "block-padding")]
    #[cfg_attr(docsrs, doc(cfg(feature = "block-padding")))]
    #[inline]
    fn decrypt_padded_b2b_mut<'a, P: Padding<Self::BlockSize>>(
        self,
        in_buf: &[u8],
        out_buf: &'a mut [u8],
    ) -> Result<&'a [u8], UnpadError> {
        if out_buf.len() < in_buf.len() {
            return Err(UnpadError);
        }
        let n = in_buf.len();
        // note: `new` always returns `Ok` here
        let buf = InOutBuf::new(in_buf, &mut out_buf[..n]).map_err(|_| UnpadError)?;
        self.decrypt_padded_inout_mut::<P>(buf)
    }

    /// Decrypt input and unpad it in a newly allocated Vec. Returns resulting
    /// ciphertext Vec.
    ///
    /// Returns [`UnpadError`] if padding is malformed or if input length is
    /// not multiple of `Self::BlockSize`.
    #[cfg(all(feature = "block-padding", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "block-padding", feature = "alloc"))))]
    #[inline]
    fn decrypt_padded_vec_mut<P: Padding<Self::BlockSize>>(
        self,
        buf: &[u8],
    ) -> Result<Vec<u8>, UnpadError> {
        let mut out = vec![0; buf.len()];
        let len = self.decrypt_padded_b2b_mut::<P>(buf, &mut out)?.len();
        out.truncate(len);
        Ok(out)
    }
}

impl<Alg: BlockEncrypt> BlockEncryptMut for Alg {
    fn encrypt_with_backend_mut(&mut self, f: impl BlockClosure<BlockSize = Self::BlockSize>) {
        self.encrypt_with_backend(f);
    }
}

impl<Alg: BlockDecrypt> BlockDecryptMut for Alg {
    fn decrypt_with_backend_mut(&mut self, f: impl BlockClosure<BlockSize = Self::BlockSize>) {
        self.decrypt_with_backend(f);
    }
}

impl<Alg: BlockCipher> BlockCipher for &Alg {}

impl<Alg: BlockEncrypt> BlockEncrypt for &Alg {
    fn encrypt_with_backend(&self, f: impl BlockClosure<BlockSize = Self::BlockSize>) {
        Alg::encrypt_with_backend(self, f);
    }
}

impl<Alg: BlockDecrypt> BlockDecrypt for &Alg {
    fn decrypt_with_backend(&self, f: impl BlockClosure<BlockSize = Self::BlockSize>) {
        Alg::decrypt_with_backend(self, f);
    }
}

/// Closure used in methods which operate over separate blocks.
struct BlockCtx<'inp, 'out, BS: ArrayLength<u8>> {
    block: InOut<'inp, 'out, Block<Self>>,
}

impl<'inp, 'out, BS: ArrayLength<u8>> BlockSizeUser for BlockCtx<'inp, 'out, BS> {
    type BlockSize = BS;
}

impl<'inp, 'out, BS: ArrayLength<u8>> BlockClosure for BlockCtx<'inp, 'out, BS> {
    #[inline(always)]
    fn call<B: BlockBackend<BlockSize = BS>>(self, backend: &mut B) {
        backend.proc_block(self.block);
    }
}

/// Closure used in methods which operate over slice of blocks.
struct BlocksCtx<'inp, 'out, BS: ArrayLength<u8>> {
    blocks: InOutBuf<'inp, 'out, Block<Self>>,
}

impl<'inp, 'out, BS: ArrayLength<u8>> BlockSizeUser for BlocksCtx<'inp, 'out, BS> {
    type BlockSize = BS;
}

impl<'inp, 'out, BS: ArrayLength<u8>> BlockClosure for BlocksCtx<'inp, 'out, BS> {
    #[inline(always)]
    fn call<B: BlockBackend<BlockSize = BS>>(self, backend: &mut B) {
        if B::ParBlocksSize::USIZE > 1 {
            let (chunks, tail) = self.blocks.into_chunks();
            for chunk in chunks {
                backend.proc_par_blocks(chunk);
            }
            backend.proc_tail_blocks(tail);
        } else {
            for block in self.blocks {
                backend.proc_block(block);
            }
        }
    }
}

#[cfg(all(feature = "block-padding", feature = "alloc"))]
fn allocate_out_vec<BS: BlockSizeUser>(len: usize) -> Vec<u8> {
    let bs = BS::BlockSize::USIZE;
    vec![0; bs * (len / bs + 1)]
}

/// Implement simple block backend
#[macro_export]
macro_rules! impl_simple_block_encdec {
    (
        <$($N:ident$(:$b0:ident$(+$b:ident)*)?),*>
        $cipher:ident, $block_size:ty, $state:ident, $block:ident,
        encrypt: $enc_block:block
        decrypt: $dec_block:block
    ) => {
        impl<$($N$(:$b0$(+$b)*)?),*> $crate::BlockSizeUser for $cipher<$($N),*> {
            type BlockSize = $block_size;
        }

        impl<$($N$(:$b0$(+$b)*)?),*> $crate::BlockEncrypt for $cipher<$($N),*> {
            fn encrypt_with_backend(&self, f: impl $crate::BlockClosure<BlockSize = $block_size>) {
                struct EncBack<'a, $($N$(:$b0$(+$b)*)?),* >(&'a $cipher<$($N),*>);

                impl<'a, $($N$(:$b0$(+$b)*)?),* > $crate::BlockSizeUser for EncBack<'a, $($N),*> {
                    type BlockSize = $block_size;
                }

                impl<'a, $($N$(:$b0$(+$b)*)?),* > $crate::ParBlocksSizeUser for EncBack<'a, $($N),*> {
                    type ParBlocksSize = $crate::consts::U1;
                }

                impl<'a, $($N$(:$b0$(+$b)*)?),* > $crate::BlockBackend for EncBack<'a, $($N),*> {
                    #[inline(always)]
                    fn proc_block(
                        &mut self,
                        mut $block: $crate::inout::InOut<'_, '_, $crate::Block<Self>>
                    ) {
                        let $state: &$cipher<$($N),*> = self.0;
                        $enc_block
                    }
                }

                f.call(&mut EncBack(self))
            }
        }

        impl<$($N$(:$b0$(+$b)*)?),*> $crate::BlockDecrypt for $cipher<$($N),*> {
            fn decrypt_with_backend(&self, f: impl $crate::BlockClosure<BlockSize = $block_size>) {
                struct DecBack<'a, $($N$(:$b0$(+$b)*)?),* >(&'a $cipher<$($N),*>);

                impl<'a, $($N$(:$b0$(+$b)*)?),* > $crate::BlockSizeUser for DecBack<'a, $($N),*> {
                    type BlockSize = $block_size;
                }

                impl<'a, $($N$(:$b0$(+$b)*)?),* > $crate::ParBlocksSizeUser for DecBack<'a, $($N),*> {
                    type ParBlocksSize = $crate::consts::U1;
                }

                impl<'a, $($N$(:$b0$(+$b)*)?),* > $crate::BlockBackend for DecBack<'a, $($N),*> {
                    #[inline(always)]
                    fn proc_block(
                        &mut self,
                        mut $block: $crate::inout::InOut<'_, '_, $crate::Block<Self>>
                    ) {
                        let $state: &$cipher<$($N),*> = self.0;
                        $dec_block
                    }
                }

                f.call(&mut DecBack(self))
            }
        }
    };
    (
        $cipher:ident, $block_size:ty, $state:ident, $block:ident,
        encrypt: $enc_block:block
        decrypt: $dec_block:block
    ) => {
        $crate::impl_simple_block_encdec!(
            <> $cipher, $block_size, $state, $block,
            encrypt: $enc_block
            decrypt: $dec_block
        );
    };
}
