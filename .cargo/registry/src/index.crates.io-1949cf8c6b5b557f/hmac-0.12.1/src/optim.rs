use super::{get_der_key, IPAD, OPAD};
use core::{fmt, slice};
#[cfg(feature = "reset")]
use digest::Reset;
use digest::{
    block_buffer::Eager,
    core_api::{
        AlgorithmName, Block, BlockSizeUser, Buffer, BufferKindUser, CoreProxy, CoreWrapper,
        FixedOutputCore, OutputSizeUser, UpdateCore,
    },
    crypto_common::{Key, KeySizeUser},
    generic_array::typenum::{IsLess, Le, NonZero, U256},
    HashMarker, InvalidLength, KeyInit, MacMarker, Output,
};

/// Generic HMAC instance.
pub type Hmac<D> = CoreWrapper<HmacCore<D>>;

/// Generic core HMAC instance, which operates over blocks.
pub struct HmacCore<D>
where
    D: CoreProxy,
    D::Core: HashMarker
        + UpdateCore
        + FixedOutputCore
        + BufferKindUser<BufferKind = Eager>
        + Default
        + Clone,
    <D::Core as BlockSizeUser>::BlockSize: IsLess<U256>,
    Le<<D::Core as BlockSizeUser>::BlockSize, U256>: NonZero,
{
    digest: D::Core,
    opad_digest: D::Core,
    #[cfg(feature = "reset")]
    ipad_digest: D::Core,
}

impl<D> Clone for HmacCore<D>
where
    D: CoreProxy,
    D::Core: HashMarker
        + UpdateCore
        + FixedOutputCore
        + BufferKindUser<BufferKind = Eager>
        + Default
        + Clone,
    <D::Core as BlockSizeUser>::BlockSize: IsLess<U256>,
    Le<<D::Core as BlockSizeUser>::BlockSize, U256>: NonZero,
{
    fn clone(&self) -> Self {
        Self {
            digest: self.digest.clone(),
            opad_digest: self.opad_digest.clone(),
            #[cfg(feature = "reset")]
            ipad_digest: self.ipad_digest.clone(),
        }
    }
}

impl<D> MacMarker for HmacCore<D>
where
    D: CoreProxy,
    D::Core: HashMarker
        + UpdateCore
        + FixedOutputCore
        + BufferKindUser<BufferKind = Eager>
        + Default
        + Clone,
    <D::Core as BlockSizeUser>::BlockSize: IsLess<U256>,
    Le<<D::Core as BlockSizeUser>::BlockSize, U256>: NonZero,
{
}

impl<D> BufferKindUser for HmacCore<D>
where
    D: CoreProxy,
    D::Core: HashMarker
        + UpdateCore
        + FixedOutputCore
        + BufferKindUser<BufferKind = Eager>
        + Default
        + Clone,
    <D::Core as BlockSizeUser>::BlockSize: IsLess<U256>,
    Le<<D::Core as BlockSizeUser>::BlockSize, U256>: NonZero,
{
    type BufferKind = Eager;
}

impl<D> KeySizeUser for HmacCore<D>
where
    D: CoreProxy,
    D::Core: HashMarker
        + UpdateCore
        + FixedOutputCore
        + BufferKindUser<BufferKind = Eager>
        + Default
        + Clone,
    <D::Core as BlockSizeUser>::BlockSize: IsLess<U256>,
    Le<<D::Core as BlockSizeUser>::BlockSize, U256>: NonZero,
{
    type KeySize = <<D as CoreProxy>::Core as BlockSizeUser>::BlockSize;
}

impl<D> BlockSizeUser for HmacCore<D>
where
    D: CoreProxy,
    D::Core: HashMarker
        + UpdateCore
        + FixedOutputCore
        + BufferKindUser<BufferKind = Eager>
        + Default
        + Clone,
    <D::Core as BlockSizeUser>::BlockSize: IsLess<U256>,
    Le<<D::Core as BlockSizeUser>::BlockSize, U256>: NonZero,
{
    type BlockSize = <<D as CoreProxy>::Core as BlockSizeUser>::BlockSize;
}

impl<D> OutputSizeUser for HmacCore<D>
where
    D: CoreProxy,
    D::Core: HashMarker
        + UpdateCore
        + FixedOutputCore
        + BufferKindUser<BufferKind = Eager>
        + Default
        + Clone,
    <D::Core as BlockSizeUser>::BlockSize: IsLess<U256>,
    Le<<D::Core as BlockSizeUser>::BlockSize, U256>: NonZero,
{
    type OutputSize = <<D as CoreProxy>::Core as OutputSizeUser>::OutputSize;
}

impl<D> KeyInit for HmacCore<D>
where
    D: CoreProxy,
    D::Core: HashMarker
        + UpdateCore
        + FixedOutputCore
        + BufferKindUser<BufferKind = Eager>
        + Default
        + Clone,
    <D::Core as BlockSizeUser>::BlockSize: IsLess<U256>,
    Le<<D::Core as BlockSizeUser>::BlockSize, U256>: NonZero,
{
    #[inline(always)]
    fn new(key: &Key<Self>) -> Self {
        Self::new_from_slice(key.as_slice()).unwrap()
    }

    #[inline(always)]
    fn new_from_slice(key: &[u8]) -> Result<Self, InvalidLength> {
        let mut buf = get_der_key::<CoreWrapper<D::Core>>(key);
        for b in buf.iter_mut() {
            *b ^= IPAD;
        }
        let mut digest = D::Core::default();
        digest.update_blocks(slice::from_ref(&buf));

        for b in buf.iter_mut() {
            *b ^= IPAD ^ OPAD;
        }

        let mut opad_digest = D::Core::default();
        opad_digest.update_blocks(slice::from_ref(&buf));

        Ok(Self {
            #[cfg(feature = "reset")]
            ipad_digest: digest.clone(),
            opad_digest,
            digest,
        })
    }
}

impl<D> UpdateCore for HmacCore<D>
where
    D: CoreProxy,
    D::Core: HashMarker
        + UpdateCore
        + FixedOutputCore
        + BufferKindUser<BufferKind = Eager>
        + Default
        + Clone,
    <D::Core as BlockSizeUser>::BlockSize: IsLess<U256>,
    Le<<D::Core as BlockSizeUser>::BlockSize, U256>: NonZero,
{
    #[inline(always)]
    fn update_blocks(&mut self, blocks: &[Block<Self>]) {
        self.digest.update_blocks(blocks);
    }
}

impl<D> FixedOutputCore for HmacCore<D>
where
    D: CoreProxy,
    D::Core: HashMarker
        + UpdateCore
        + FixedOutputCore
        + BufferKindUser<BufferKind = Eager>
        + Default
        + Clone,
    <D::Core as BlockSizeUser>::BlockSize: IsLess<U256>,
    Le<<D::Core as BlockSizeUser>::BlockSize, U256>: NonZero,
{
    #[inline(always)]
    fn finalize_fixed_core(&mut self, buffer: &mut Buffer<Self>, out: &mut Output<Self>) {
        let mut hash = Output::<D::Core>::default();
        self.digest.finalize_fixed_core(buffer, &mut hash);
        // finalize_fixed_core should reset the buffer as well, but
        // to be extra safe we reset it explicitly again.
        buffer.reset();
        #[cfg(not(feature = "reset"))]
        let h = &mut self.opad_digest;
        #[cfg(feature = "reset")]
        let mut h = self.opad_digest.clone();
        buffer.digest_blocks(&hash, |b| h.update_blocks(b));
        h.finalize_fixed_core(buffer, out);
    }
}

#[cfg(feature = "reset")]
#[cfg_attr(docsrs, doc(cfg(feature = "reset")))]
impl<D> Reset for HmacCore<D>
where
    D: CoreProxy,
    D::Core: HashMarker
        + UpdateCore
        + FixedOutputCore
        + BufferKindUser<BufferKind = Eager>
        + Default
        + Clone,
    <D::Core as BlockSizeUser>::BlockSize: IsLess<U256>,
    Le<<D::Core as BlockSizeUser>::BlockSize, U256>: NonZero,
{
    #[inline(always)]
    fn reset(&mut self) {
        self.digest = self.ipad_digest.clone();
    }
}

impl<D> AlgorithmName for HmacCore<D>
where
    D: CoreProxy,
    D::Core: HashMarker
        + AlgorithmName
        + UpdateCore
        + FixedOutputCore
        + BufferKindUser<BufferKind = Eager>
        + Default
        + Clone,
    <D::Core as BlockSizeUser>::BlockSize: IsLess<U256>,
    Le<<D::Core as BlockSizeUser>::BlockSize, U256>: NonZero,
{
    fn write_alg_name(f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Hmac<")?;
        <D::Core as AlgorithmName>::write_alg_name(f)?;
        f.write_str(">")
    }
}

impl<D> fmt::Debug for HmacCore<D>
where
    D: CoreProxy,
    D::Core: HashMarker
        + AlgorithmName
        + UpdateCore
        + FixedOutputCore
        + BufferKindUser<BufferKind = Eager>
        + Default
        + Clone,
    <D::Core as BlockSizeUser>::BlockSize: IsLess<U256>,
    Le<<D::Core as BlockSizeUser>::BlockSize, U256>: NonZero,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HmacCore<")?;
        <D::Core as AlgorithmName>::write_alg_name(f)?;
        f.write_str("> { ... }")
    }
}
