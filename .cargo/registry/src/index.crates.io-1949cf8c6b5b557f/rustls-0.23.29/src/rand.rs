//! The single place where we generate random material for our own use.

use alloc::vec;
use alloc::vec::Vec;

use crate::crypto::SecureRandom;

/// Make a [`Vec<u8>`] of the given size containing random material.
pub(crate) fn random_vec(
    secure_random: &dyn SecureRandom,
    len: usize,
) -> Result<Vec<u8>, GetRandomFailed> {
    let mut v = vec![0; len];
    secure_random.fill(&mut v)?;
    Ok(v)
}

/// Return a uniformly random [`u32`].
pub(crate) fn random_u32(secure_random: &dyn SecureRandom) -> Result<u32, GetRandomFailed> {
    let mut buf = [0u8; 4];
    secure_random.fill(&mut buf)?;
    Ok(u32::from_be_bytes(buf))
}

/// Return a uniformly random [`u16`].
pub(crate) fn random_u16(secure_random: &dyn SecureRandom) -> Result<u16, GetRandomFailed> {
    let mut buf = [0u8; 2];
    secure_random.fill(&mut buf)?;
    Ok(u16::from_be_bytes(buf))
}

/// Random material generation failed.
#[derive(Debug)]
pub struct GetRandomFailed;
