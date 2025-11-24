//! Low-level traits operating on blocks and wrappers around them.
//!
//! Usage of traits in this module in user code is discouraged. Instead use
//! core algorithm wrapped by the wrapper types, which implement the
//! higher-level traits.
use crate::InvalidOutputSize;

pub use crypto_common::{AlgorithmName, Block, BlockSizeUser, OutputSizeUser, Reset};

use block_buffer::{BlockBuffer, BufferKind};
use crypto_common::{
    typenum::{IsLess, Le, NonZero, U256},
    Output,
};

mod ct_variable;
mod rt_variable;
mod wrapper;
mod xof_reader;

pub use ct_variable::CtVariableCoreWrapper;
pub use rt_variable::RtVariableCoreWrapper;
pub use wrapper::{CoreProxy, CoreWrapper};
pub use xof_reader::XofReaderCoreWrapper;

/// Buffer type used by type which implements [`BufferKindUser`].
pub type Buffer<S> =
    BlockBuffer<<S as BlockSizeUser>::BlockSize, <S as BufferKindUser>::BufferKind>;

/// Types which consume data in blocks.
pub trait UpdateCore: BlockSizeUser {
    /// Update state using the provided data blocks.
    fn update_blocks(&mut self, blocks: &[Block<Self>]);
}

/// Types which use [`BlockBuffer`] functionality.
pub trait BufferKindUser: BlockSizeUser {
    /// Block buffer kind over which type operates.
    type BufferKind: BufferKind;
}

/// Core trait for hash functions with fixed output size.
pub trait FixedOutputCore: UpdateCore + BufferKindUser + OutputSizeUser
where
    Self::BlockSize: IsLess<U256>,
    Le<Self::BlockSize, U256>: NonZero,
{
    /// Finalize state using remaining data stored in the provided block buffer,
    /// write result into provided array and leave `self` in a dirty state.
    fn finalize_fixed_core(&mut self, buffer: &mut Buffer<Self>, out: &mut Output<Self>);
}

/// Core trait for hash functions with extendable (XOF) output size.
pub trait ExtendableOutputCore: UpdateCore + BufferKindUser
where
    Self::BlockSize: IsLess<U256>,
    Le<Self::BlockSize, U256>: NonZero,
{
    /// XOF reader core state.
    type ReaderCore: XofReaderCore;

    /// Retrieve XOF reader using remaining data stored in the block buffer
    /// and leave hasher in a dirty state.
    fn finalize_xof_core(&mut self, buffer: &mut Buffer<Self>) -> Self::ReaderCore;
}

/// Core reader trait for extendable-output function (XOF) result.
pub trait XofReaderCore: BlockSizeUser {
    /// Read next XOF block.
    fn read_block(&mut self) -> Block<Self>;
}

/// Core trait for hash functions with variable output size.
///
/// Maximum output size is equal to [`OutputSizeUser::OutputSize`].
/// Users are expected to truncate result returned by the
/// [`finalize_variable_core`] to `output_size` passed to the [`new`] method
/// during construction. Truncation side is defined by the [`TRUNC_SIDE`]
/// associated constant.
///
/// [`finalize_variable_core`]: VariableOutputCore::finalize_variable_core
/// [`new`]: VariableOutputCore::new
/// [`TRUNC_SIDE`]: VariableOutputCore::TRUNC_SIDE
pub trait VariableOutputCore: UpdateCore + OutputSizeUser + BufferKindUser + Sized
where
    Self::BlockSize: IsLess<U256>,
    Le<Self::BlockSize, U256>: NonZero,
{
    /// Side which should be used in a truncated result.
    const TRUNC_SIDE: TruncSide;

    /// Initialize hasher state for given output size.
    ///
    /// Returns [`InvalidOutputSize`] if `output_size` is not valid for
    /// the algorithm, e.g. if it's bigger than the [`OutputSize`]
    /// associated type.
    ///
    /// [`OutputSize`]: OutputSizeUser::OutputSize
    fn new(output_size: usize) -> Result<Self, InvalidOutputSize>;

    /// Finalize hasher and write full hashing result into the `out` buffer.
    ///
    /// The result must be truncated to `output_size` used during hasher
    /// construction. Truncation side is defined by the [`TRUNC_SIDE`]
    /// associated constant.
    ///
    /// [`TRUNC_SIDE`]: VariableOutputCore::TRUNC_SIDE
    fn finalize_variable_core(&mut self, buffer: &mut Buffer<Self>, out: &mut Output<Self>);
}

/// Type which used for defining truncation side in the [`VariableOutputCore`]
/// trait.
#[derive(Copy, Clone, Debug)]
pub enum TruncSide {
    /// Truncate left side, i.e. `&out[..n]`.
    Left,
    /// Truncate right side, i.e. `&out[m..]`.
    Right,
}
