use super::{
    AlgorithmName, Buffer, BufferKindUser, ExtendableOutputCore, FixedOutputCore, OutputSizeUser,
    Reset, UpdateCore, XofReaderCoreWrapper,
};
use crate::{
    ExtendableOutput, ExtendableOutputReset, FixedOutput, FixedOutputReset, HashMarker, Update,
};
use block_buffer::BlockBuffer;
use core::fmt;
use crypto_common::{
    typenum::{IsLess, Le, NonZero, U256},
    BlockSizeUser, InvalidLength, Key, KeyInit, KeySizeUser, Output,
};

#[cfg(feature = "mac")]
use crate::MacMarker;
#[cfg(feature = "oid")]
use const_oid::{AssociatedOid, ObjectIdentifier};

/// Wrapper around [`BufferKindUser`].
///
/// It handles data buffering and implements the slice-based traits.
#[derive(Clone, Default)]
pub struct CoreWrapper<T>
where
    T: BufferKindUser,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    core: T,
    buffer: BlockBuffer<T::BlockSize, T::BufferKind>,
}

impl<T> HashMarker for CoreWrapper<T>
where
    T: BufferKindUser + HashMarker,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
}

#[cfg(feature = "mac")]
#[cfg_attr(docsrs, doc(cfg(feature = "mac")))]
impl<T> MacMarker for CoreWrapper<T>
where
    T: BufferKindUser + MacMarker,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
}

// this blanket impl is needed for HMAC
impl<T> BlockSizeUser for CoreWrapper<T>
where
    T: BufferKindUser + HashMarker,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    type BlockSize = T::BlockSize;
}

impl<T> CoreWrapper<T>
where
    T: BufferKindUser,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    /// Create new wrapper from `core`.
    #[inline]
    pub fn from_core(core: T) -> Self {
        let buffer = Default::default();
        Self { core, buffer }
    }

    /// Decompose wrapper into inner parts.
    #[inline]
    pub fn decompose(self) -> (T, Buffer<T>) {
        let Self { core, buffer } = self;
        (core, buffer)
    }
}

impl<T> KeySizeUser for CoreWrapper<T>
where
    T: BufferKindUser + KeySizeUser,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    type KeySize = T::KeySize;
}

impl<T> KeyInit for CoreWrapper<T>
where
    T: BufferKindUser + KeyInit,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    #[inline]
    fn new(key: &Key<Self>) -> Self {
        Self {
            core: T::new(key),
            buffer: Default::default(),
        }
    }

    #[inline]
    fn new_from_slice(key: &[u8]) -> Result<Self, InvalidLength> {
        Ok(Self {
            core: T::new_from_slice(key)?,
            buffer: Default::default(),
        })
    }
}

impl<T> fmt::Debug for CoreWrapper<T>
where
    T: BufferKindUser + AlgorithmName,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        T::write_alg_name(f)?;
        f.write_str(" { .. }")
    }
}

impl<T> Reset for CoreWrapper<T>
where
    T: BufferKindUser + Reset,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    #[inline]
    fn reset(&mut self) {
        self.core.reset();
        self.buffer.reset();
    }
}

impl<T> Update for CoreWrapper<T>
where
    T: BufferKindUser + UpdateCore,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    #[inline]
    fn update(&mut self, input: &[u8]) {
        let Self { core, buffer } = self;
        buffer.digest_blocks(input, |blocks| core.update_blocks(blocks));
    }
}

impl<T> OutputSizeUser for CoreWrapper<T>
where
    T: BufferKindUser + OutputSizeUser,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    type OutputSize = T::OutputSize;
}

impl<T> FixedOutput for CoreWrapper<T>
where
    T: FixedOutputCore,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    #[inline]
    fn finalize_into(mut self, out: &mut Output<Self>) {
        let Self { core, buffer } = &mut self;
        core.finalize_fixed_core(buffer, out);
    }
}

impl<T> FixedOutputReset for CoreWrapper<T>
where
    T: FixedOutputCore + Reset,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    #[inline]
    fn finalize_into_reset(&mut self, out: &mut Output<Self>) {
        let Self { core, buffer } = self;
        core.finalize_fixed_core(buffer, out);
        core.reset();
        buffer.reset();
    }
}

impl<T> ExtendableOutput for CoreWrapper<T>
where
    T: ExtendableOutputCore,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
    <T::ReaderCore as BlockSizeUser>::BlockSize: IsLess<U256>,
    Le<<T::ReaderCore as BlockSizeUser>::BlockSize, U256>: NonZero,
{
    type Reader = XofReaderCoreWrapper<T::ReaderCore>;

    #[inline]
    fn finalize_xof(self) -> Self::Reader {
        let (mut core, mut buffer) = self.decompose();
        let core = core.finalize_xof_core(&mut buffer);
        let buffer = Default::default();
        Self::Reader { core, buffer }
    }
}

impl<T> ExtendableOutputReset for CoreWrapper<T>
where
    T: ExtendableOutputCore + Reset,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
    <T::ReaderCore as BlockSizeUser>::BlockSize: IsLess<U256>,
    Le<<T::ReaderCore as BlockSizeUser>::BlockSize, U256>: NonZero,
{
    #[inline]
    fn finalize_xof_reset(&mut self) -> Self::Reader {
        let Self { core, buffer } = self;
        let reader_core = core.finalize_xof_core(buffer);
        core.reset();
        buffer.reset();
        let buffer = Default::default();
        Self::Reader {
            core: reader_core,
            buffer,
        }
    }
}

#[cfg(feature = "oid")]
#[cfg_attr(docsrs, doc(cfg(feature = "oid")))]
impl<T> AssociatedOid for CoreWrapper<T>
where
    T: BufferKindUser + AssociatedOid,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    const OID: ObjectIdentifier = T::OID;
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<T> std::io::Write for CoreWrapper<T>
where
    T: BufferKindUser + UpdateCore,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Update::update(self, buf);
        Ok(buf.len())
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A proxy trait to a core type implemented by [`CoreWrapper`]
// TODO: replace with an inherent associated type on stabilization:
// https://github.com/rust-lang/rust/issues/8995
pub trait CoreProxy: sealed::Sealed {
    /// Type wrapped by [`CoreWrapper`].
    type Core;
}

mod sealed {
    pub trait Sealed {}
}

impl<T> sealed::Sealed for CoreWrapper<T>
where
    T: BufferKindUser,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
}

impl<T> CoreProxy for CoreWrapper<T>
where
    T: BufferKindUser,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    type Core = T;
}
