use crate::{
    errors::StreamCipherError, Block, OverflowError, SeekNum, StreamCipher, StreamCipherCore,
    StreamCipherSeek, StreamCipherSeekCore,
};
use crypto_common::{
    typenum::{IsLess, Le, NonZero, Unsigned, U256},
    BlockSizeUser, Iv, IvSizeUser, Key, KeyInit, KeyIvInit, KeySizeUser,
};
use inout::InOutBuf;
#[cfg(feature = "zeroize")]
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Wrapper around [`StreamCipherCore`] implementations.
///
/// It handles data buffering and implements the slice-based traits.
#[derive(Clone, Default)]
pub struct StreamCipherCoreWrapper<T: BlockSizeUser>
where
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    core: T,
    buffer: Block<T>,
    pos: u8,
}

impl<T: StreamCipherCore> StreamCipherCoreWrapper<T>
where
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    /// Return reference to the core type.
    pub fn get_core(&self) -> &T {
        &self.core
    }

    /// Return reference to the core type.
    pub fn from_core(core: T) -> Self {
        Self {
            core,
            buffer: Default::default(),
            pos: 0,
        }
    }

    /// Return current cursor position.
    #[inline]
    fn get_pos(&self) -> usize {
        let pos = self.pos as usize;
        if T::BlockSize::USIZE == 0 {
            panic!("Block size can not be equal to zero");
        }
        if pos >= T::BlockSize::USIZE {
            debug_assert!(false);
            // SAFETY: `pos` is set only to values smaller than block size
            unsafe { core::hint::unreachable_unchecked() }
        }
        self.pos as usize
    }

    /// Return size of the internal buffer in bytes.
    #[inline]
    fn size(&self) -> usize {
        T::BlockSize::USIZE
    }

    #[inline]
    fn set_pos_unchecked(&mut self, pos: usize) {
        debug_assert!(pos < T::BlockSize::USIZE);
        self.pos = pos as u8;
    }

    /// Return number of remaining bytes in the internal buffer.
    #[inline]
    fn remaining(&self) -> usize {
        self.size() - self.get_pos()
    }

    fn check_remaining(&self, dlen: usize) -> Result<(), StreamCipherError> {
        let rem_blocks = match self.core.remaining_blocks() {
            Some(v) => v,
            None => return Ok(()),
        };

        let bytes = if self.pos == 0 {
            dlen
        } else {
            let rem = self.remaining();
            if dlen > rem {
                dlen - rem
            } else {
                return Ok(());
            }
        };
        let bs = T::BlockSize::USIZE;
        let blocks = if bytes % bs == 0 {
            bytes / bs
        } else {
            bytes / bs + 1
        };
        if blocks > rem_blocks {
            Err(StreamCipherError)
        } else {
            Ok(())
        }
    }
}

impl<T: StreamCipherCore> StreamCipher for StreamCipherCoreWrapper<T>
where
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    #[inline]
    fn try_apply_keystream_inout(
        &mut self,
        mut data: InOutBuf<'_, '_, u8>,
    ) -> Result<(), StreamCipherError> {
        self.check_remaining(data.len())?;

        let pos = self.get_pos();
        if pos != 0 {
            let rem = &self.buffer[pos..];
            let n = data.len();
            if n < rem.len() {
                data.xor_in2out(&rem[..n]);
                self.set_pos_unchecked(pos + n);
                return Ok(());
            }
            let (mut left, right) = data.split_at(rem.len());
            data = right;
            left.xor_in2out(rem);
        }

        let (blocks, mut leftover) = data.into_chunks();
        self.core.apply_keystream_blocks_inout(blocks);

        let n = leftover.len();
        if n != 0 {
            self.core.write_keystream_block(&mut self.buffer);
            leftover.xor_in2out(&self.buffer[..n]);
        }
        self.set_pos_unchecked(n);

        Ok(())
    }
}

impl<T: StreamCipherSeekCore> StreamCipherSeek for StreamCipherCoreWrapper<T>
where
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    fn try_current_pos<SN: SeekNum>(&self) -> Result<SN, OverflowError> {
        let Self { core, pos, .. } = self;
        SN::from_block_byte(core.get_block_pos(), *pos, T::BlockSize::U8)
    }

    fn try_seek<SN: SeekNum>(&mut self, new_pos: SN) -> Result<(), StreamCipherError> {
        let Self { core, buffer, pos } = self;
        let (block_pos, byte_pos) = new_pos.into_block_byte(T::BlockSize::U8)?;
        core.set_block_pos(block_pos);
        if byte_pos != 0 {
            self.core.write_keystream_block(buffer);
        }
        *pos = byte_pos;
        Ok(())
    }
}

// Note: ideally we would only implement the InitInner trait and everything
// else would be handled by blanket impls, but unfortunately it will
// not work properly without mutually exclusive traits, see:
// https://github.com/rust-lang/rfcs/issues/1053

impl<T: KeySizeUser + BlockSizeUser> KeySizeUser for StreamCipherCoreWrapper<T>
where
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    type KeySize = T::KeySize;
}

impl<T: IvSizeUser + BlockSizeUser> IvSizeUser for StreamCipherCoreWrapper<T>
where
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    type IvSize = T::IvSize;
}

impl<T: KeyIvInit + BlockSizeUser> KeyIvInit for StreamCipherCoreWrapper<T>
where
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    #[inline]
    fn new(key: &Key<Self>, iv: &Iv<Self>) -> Self {
        Self {
            core: T::new(key, iv),
            buffer: Default::default(),
            pos: 0,
        }
    }
}

impl<T: KeyInit + BlockSizeUser> KeyInit for StreamCipherCoreWrapper<T>
where
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    #[inline]
    fn new(key: &Key<Self>) -> Self {
        Self {
            core: T::new(key),
            buffer: Default::default(),
            pos: 0,
        }
    }
}

#[cfg(feature = "zeroize")]
impl<T> Drop for StreamCipherCoreWrapper<T>
where
    T: BlockSizeUser,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    fn drop(&mut self) {
        self.buffer.zeroize();
        self.pos.zeroize();
    }
}

#[cfg(feature = "zeroize")]
impl<T> ZeroizeOnDrop for StreamCipherCoreWrapper<T>
where
    T: BlockSizeUser + ZeroizeOnDrop,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
}
