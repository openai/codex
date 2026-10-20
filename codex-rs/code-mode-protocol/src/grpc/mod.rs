#![allow(clippy::double_must_use)] // Emitted by async_trait in the generated tonic service.

#[cfg(codex_bazel)]
pub use code_mode_proto::codex::code_mode::v1::*;

#[cfg(not(codex_bazel))]
tonic::include_proto!("codex.code_mode.v1");

pub const MAX_IDENTIFIER_BYTES: usize = 256;
