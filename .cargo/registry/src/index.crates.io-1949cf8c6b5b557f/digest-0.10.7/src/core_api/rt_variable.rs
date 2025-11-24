use super::{AlgorithmName, TruncSide, UpdateCore, VariableOutputCore};
#[cfg(feature = "mac")]
use crate::MacMarker;
use crate::{HashMarker, InvalidBufferSize};
use crate::{InvalidOutputSize, Reset, Update, VariableOutput, VariableOutputReset};
use block_buffer::BlockBuffer;
use core::fmt;
use crypto_common::typenum::{IsLess, Le, NonZero, Unsigned, U256};

/// Wrapper around [`VariableOutputCore`] which selects output size
/// at run time.
#[derive(Clone)]
pub struct RtVariableCoreWrapper<T>
where
    T: VariableOutputCore + UpdateCore,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    core: T,
    buffer: BlockBuffer<T::BlockSize, T::BufferKind>,
    output_size: usize,
}

impl<T> RtVariableCoreWrapper<T>
where
    T: VariableOutputCore,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    #[inline]
    fn finalize_dirty(&mut self, out: &mut [u8]) -> Result<(), InvalidBufferSize> {
        let Self {
            core,
            buffer,
            output_size,
        } = self;
        if out.len() != *output_size || out.len() > Self::MAX_OUTPUT_SIZE {
            return Err(InvalidBufferSize);
        }
        let mut full_res = Default::default();
        core.finalize_variable_core(buffer, &mut full_res);
        let n = out.len();
        let m = full_res.len() - n;
        match T::TRUNC_SIDE {
            TruncSide::Left => out.copy_from_slice(&full_res[..n]),
            TruncSide::Right => out.copy_from_slice(&full_res[m..]),
        }
        Ok(())
    }
}

impl<T> HashMarker for RtVariableCoreWrapper<T>
where
    T: VariableOutputCore + HashMarker,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
}

#[cfg(feature = "mac")]
#[cfg_attr(docsrs, doc(cfg(feature = "mac")))]
impl<T> MacMarker for RtVariableCoreWrapper<T>
where
    T: VariableOutputCore + MacMarker,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
}

impl<T> Reset for RtVariableCoreWrapper<T>
where
    T: VariableOutputCore + UpdateCore + Reset,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    #[inline]
    fn reset(&mut self) {
        self.buffer.reset();
        self.core.reset();
    }
}

impl<T> Update for RtVariableCoreWrapper<T>
where
    T: VariableOutputCore + UpdateCore,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    #[inline]
    fn update(&mut self, input: &[u8]) {
        let Self { core, buffer, .. } = self;
        buffer.digest_blocks(input, |blocks| core.update_blocks(blocks));
    }
}

impl<T> VariableOutput for RtVariableCoreWrapper<T>
where
    T: VariableOutputCore + UpdateCore,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    const MAX_OUTPUT_SIZE: usize = T::OutputSize::USIZE;

    fn new(output_size: usize) -> Result<Self, InvalidOutputSize> {
        let buffer = Default::default();
        T::new(output_size).map(|core| Self {
            core,
            buffer,
            output_size,
        })
    }

    fn output_size(&self) -> usize {
        self.output_size
    }

    fn finalize_variable(mut self, out: &mut [u8]) -> Result<(), InvalidBufferSize> {
        self.finalize_dirty(out)
    }
}

impl<T> VariableOutputReset for RtVariableCoreWrapper<T>
where
    T: VariableOutputCore + UpdateCore + Reset,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    fn finalize_variable_reset(&mut self, out: &mut [u8]) -> Result<(), InvalidBufferSize> {
        self.finalize_dirty(out)?;
        self.core.reset();
        self.buffer.reset();
        Ok(())
    }
}

impl<T> fmt::Debug for RtVariableCoreWrapper<T>
where
    T: VariableOutputCore + UpdateCore + AlgorithmName,
    T::BlockSize: IsLess<U256>,
    Le<T::BlockSize, U256>: NonZero,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        T::write_alg_name(f)?;
        f.write_str(" { .. }")
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<T> std::io::Write for RtVariableCoreWrapper<T>
where
    T: VariableOutputCore + UpdateCore,
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
