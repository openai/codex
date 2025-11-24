// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use core::fmt::Display;

#[derive(Debug)]
pub enum VarZeroVecFormatError {
    /// The byte buffer was not in the appropriate format for VarZeroVec.
    Metadata,
    /// One of the values could not be decoded.
    Values(crate::ule::UleError),
}

impl core::error::Error for VarZeroVecFormatError {}

impl Display for VarZeroVecFormatError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Metadata => write!(f, "VarZeroVecFormatError: metadata"),
            Self::Values(e) => write!(f, "VarZeroVecFormatError: {e}"),
        }
    }
}
