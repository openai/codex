use super::{
    AlgorithmName, Buffer, BufferKindUser, FixedOutputCore, Reset, TruncSide, UpdateCore,
    VariableOutputCore,
};
use crate::HashMarker;
#[cfg(feature = "mac")]
use crate::MacMarker;
#[cfg(feature = "oid")]
use const_oid::{AssociatedOid, ObjectIdentifier};
use core::{fmt, marker::PhantomData};
use crypto_common::{
    generic_array::{ArrayLength, GenericArray},
    typenum::{IsLess, IsLessOrEqual, Le, LeEq, NonZero, U256},
    Block, BlockSizeUser, OutputSizeUser,
};

/// Dummy type used with [`CtVariableCoreWrapper`] in cases when
/// resulting hash does not have a known OID.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct NoOid;

/// Wrapper around [`VariableOutputCore`] which selects output size
/// at compile time.
#[derive(Clone)]
pub struct CtVariableCoreWrapper<T, OutSize, O = NoOid>
where
    T: VariableOutputCore,
    OutSize: ArrayLength<u8> + IsLessOrEqual<T::OutputSize>,
    LeEq<OutSize, T::OutputSize>: NonZero,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    inner: T,
    _out: PhantomData<(OutSize, O)>,
}

impl<T, OutSize, O> HashMarker for CtVariableCoreWrapper<T, OutSize, O>
where
    T: VariableOutputCore + HashMarker,
    OutSize: ArrayLength<u8> + IsLessOrEqual<T::OutputSize>,
    LeEq<OutSize, T::OutputSize>: NonZero,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
}

#[cfg(feature = "mac")]
impl<T, OutSize, O> MacMarker for CtVariableCoreWrapper<T, OutSize, O>
where
    T: VariableOutputCore + MacMarker,
    OutSize: ArrayLength<u8> + IsLessOrEqual<T::OutputSize>,
    LeEq<OutSize, T::OutputSize>: NonZero,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
}

impl<T, OutSize, O> BlockSizeUser for CtVariableCoreWrapper<T, OutSize, O>
where
    T: VariableOutputCore,
    OutSize: ArrayLength<u8> + IsLessOrEqual<T::OutputSize>,
    LeEq<OutSize, T::OutputSize>: NonZero,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    type BlockSize = T::BlockSize;
}

impl<T, OutSize, O> UpdateCore for CtVariableCoreWrapper<T, OutSize, O>
where
    T: VariableOutputCore,
    OutSize: ArrayLength<u8> + IsLessOrEqual<T::OutputSize>,
    LeEq<OutSize, T::OutputSize>: NonZero,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    #[inline]
    fn update_blocks(&mut self, blocks: &[Block<Self>]) {
        self.inner.update_blocks(blocks);
    }
}

impl<T, OutSize, O> OutputSizeUser for CtVariableCoreWrapper<T, OutSize, O>
where
    T: VariableOutputCore,
    OutSize: ArrayLength<u8> + IsLessOrEqual<T::OutputSize> + 'static,
    LeEq<OutSize, T::OutputSize>: NonZero,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    type OutputSize = OutSize;
}

impl<T, OutSize, O> BufferKindUser for CtVariableCoreWrapper<T, OutSize, O>
where
    T: VariableOutputCore,
    OutSize: ArrayLength<u8> + IsLessOrEqual<T::OutputSize>,
    LeEq<OutSize, T::OutputSize>: NonZero,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    type BufferKind = T::BufferKind;
}

impl<T, OutSize, O> FixedOutputCore for CtVariableCoreWrapper<T, OutSize, O>
where
    T: VariableOutputCore,
    OutSize: ArrayLength<u8> + IsLessOrEqual<T::OutputSize> + 'static,
    LeEq<OutSize, T::OutputSize>: NonZero,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    #[inline]
    fn finalize_fixed_core(
        &mut self,
        buffer: &mut Buffer<Self>,
        out: &mut GenericArray<u8, Self::OutputSize>,
    ) {
        let mut full_res = Default::default();
        self.inner.finalize_variable_core(buffer, &mut full_res);
        let n = out.len();
        let m = full_res.len() - n;
        match T::TRUNC_SIDE {
            TruncSide::Left => out.copy_from_slice(&full_res[..n]),
            TruncSide::Right => out.copy_from_slice(&full_res[m..]),
        }
    }
}

impl<T, OutSize, O> Default for CtVariableCoreWrapper<T, OutSize, O>
where
    T: VariableOutputCore,
    OutSize: ArrayLength<u8> + IsLessOrEqual<T::OutputSize>,
    LeEq<OutSize, T::OutputSize>: NonZero,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    #[inline]
    fn default() -> Self {
        Self {
            inner: T::new(OutSize::USIZE).unwrap(),
            _out: PhantomData,
        }
    }
}

impl<T, OutSize, O> Reset for CtVariableCoreWrapper<T, OutSize, O>
where
    T: VariableOutputCore,
    OutSize: ArrayLength<u8> + IsLessOrEqual<T::OutputSize>,
    LeEq<OutSize, T::OutputSize>: NonZero,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    #[inline]
    fn reset(&mut self) {
        *self = Default::default();
    }
}

impl<T, OutSize, O> AlgorithmName for CtVariableCoreWrapper<T, OutSize, O>
where
    T: VariableOutputCore + AlgorithmName,
    OutSize: ArrayLength<u8> + IsLessOrEqual<T::OutputSize>,
    LeEq<OutSize, T::OutputSize>: NonZero,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    fn write_alg_name(f: &mut fmt::Formatter<'_>) -> fmt::Result {
        T::write_alg_name(f)?;
        f.write_str("_")?;
        write!(f, "{}", OutSize::USIZE)
    }
}

#[cfg(feature = "oid")]
#[cfg_attr(docsrs, doc(cfg(feature = "oid")))]
impl<T, OutSize, O> AssociatedOid for CtVariableCoreWrapper<T, OutSize, O>
where
    T: VariableOutputCore,
    O: AssociatedOid,
    OutSize: ArrayLength<u8> + IsLessOrEqual<T::OutputSize>,
    LeEq<OutSize, T::OutputSize>: NonZero,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    const OID: ObjectIdentifier = O::OID;
}

/// Implement dummy type with hidden docs which is used to "carry" hasher
/// OID for [`CtVariableCoreWrapper`].
#[macro_export]
macro_rules! impl_oid_carrier {
    ($name:ident, $oid:literal) => {
        #[doc(hidden)]
        #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
        pub struct $name;

        #[cfg(feature = "oid")]
        impl AssociatedOid for $name {
            const OID: ObjectIdentifier = ObjectIdentifier::new_unwrap($oid);
        }
    };
}
