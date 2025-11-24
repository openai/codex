use crate::{consts, sha256::compress256, sha512::compress512};
use core::{fmt, slice::from_ref};
use digest::{
    block_buffer::Eager,
    core_api::{
        AlgorithmName, Block, BlockSizeUser, Buffer, BufferKindUser, OutputSizeUser, TruncSide,
        UpdateCore, VariableOutputCore,
    },
    typenum::{Unsigned, U128, U32, U64},
    HashMarker, InvalidOutputSize, Output,
};

/// Core block-level SHA-256 hasher with variable output size.
///
/// Supports initialization only for 28 and 32 byte output sizes,
/// i.e. 224 and 256 bits respectively.
#[derive(Clone)]
pub struct Sha256VarCore {
    state: consts::State256,
    block_len: u64,
}

impl HashMarker for Sha256VarCore {}

impl BlockSizeUser for Sha256VarCore {
    type BlockSize = U64;
}

impl BufferKindUser for Sha256VarCore {
    type BufferKind = Eager;
}

impl UpdateCore for Sha256VarCore {
    #[inline]
    fn update_blocks(&mut self, blocks: &[Block<Self>]) {
        self.block_len += blocks.len() as u64;
        compress256(&mut self.state, blocks);
    }
}

impl OutputSizeUser for Sha256VarCore {
    type OutputSize = U32;
}

impl VariableOutputCore for Sha256VarCore {
    const TRUNC_SIDE: TruncSide = TruncSide::Left;

    #[inline]
    fn new(output_size: usize) -> Result<Self, InvalidOutputSize> {
        let state = match output_size {
            28 => consts::H256_224,
            32 => consts::H256_256,
            _ => return Err(InvalidOutputSize),
        };
        let block_len = 0;
        Ok(Self { state, block_len })
    }

    #[inline]
    fn finalize_variable_core(&mut self, buffer: &mut Buffer<Self>, out: &mut Output<Self>) {
        let bs = Self::BlockSize::U64;
        let bit_len = 8 * (buffer.get_pos() as u64 + bs * self.block_len);
        buffer.len64_padding_be(bit_len, |b| compress256(&mut self.state, from_ref(b)));

        for (chunk, v) in out.chunks_exact_mut(4).zip(self.state.iter()) {
            chunk.copy_from_slice(&v.to_be_bytes());
        }
    }
}

impl AlgorithmName for Sha256VarCore {
    #[inline]
    fn write_alg_name(f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Sha256")
    }
}

impl fmt::Debug for Sha256VarCore {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Sha256VarCore { ... }")
    }
}

/// Core block-level SHA-512 hasher with variable output size.
///
/// Supports initialization only for 28, 32, 48, and 64 byte output sizes,
/// i.e. 224, 256, 384, and 512 bits respectively.
#[derive(Clone)]
pub struct Sha512VarCore {
    state: consts::State512,
    block_len: u128,
}

impl HashMarker for Sha512VarCore {}

impl BlockSizeUser for Sha512VarCore {
    type BlockSize = U128;
}

impl BufferKindUser for Sha512VarCore {
    type BufferKind = Eager;
}

impl UpdateCore for Sha512VarCore {
    #[inline]
    fn update_blocks(&mut self, blocks: &[Block<Self>]) {
        self.block_len += blocks.len() as u128;
        compress512(&mut self.state, blocks);
    }
}

impl OutputSizeUser for Sha512VarCore {
    type OutputSize = U64;
}

impl VariableOutputCore for Sha512VarCore {
    const TRUNC_SIDE: TruncSide = TruncSide::Left;

    #[inline]
    fn new(output_size: usize) -> Result<Self, InvalidOutputSize> {
        let state = match output_size {
            28 => consts::H512_224,
            32 => consts::H512_256,
            48 => consts::H512_384,
            64 => consts::H512_512,
            _ => return Err(InvalidOutputSize),
        };
        let block_len = 0;
        Ok(Self { state, block_len })
    }

    #[inline]
    fn finalize_variable_core(&mut self, buffer: &mut Buffer<Self>, out: &mut Output<Self>) {
        let bs = Self::BlockSize::U64 as u128;
        let bit_len = 8 * (buffer.get_pos() as u128 + bs * self.block_len);
        buffer.len128_padding_be(bit_len, |b| compress512(&mut self.state, from_ref(b)));

        for (chunk, v) in out.chunks_exact_mut(8).zip(self.state.iter()) {
            chunk.copy_from_slice(&v.to_be_bytes());
        }
    }
}

impl AlgorithmName for Sha512VarCore {
    #[inline]
    fn write_alg_name(f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Sha512")
    }
}

impl fmt::Debug for Sha512VarCore {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Sha512VarCore { ... }")
    }
}
