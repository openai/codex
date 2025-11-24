//! Chunk types and functions
#![allow(dead_code)]
#![allow(non_upper_case_globals)]
use core::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkType(pub [u8; 4]);

// -- Critical chunks --

/// Image header
pub const IHDR: ChunkType = ChunkType(*b"IHDR");
/// Palette
pub const PLTE: ChunkType = ChunkType(*b"PLTE");
/// Image data
pub const IDAT: ChunkType = ChunkType(*b"IDAT");
/// Image trailer
pub const IEND: ChunkType = ChunkType(*b"IEND");

// -- Ancillary chunks --

/// Transparency
pub const tRNS: ChunkType = ChunkType(*b"tRNS");
/// Background colour
pub const bKGD: ChunkType = ChunkType(*b"bKGD");
/// Image last-modification time
pub const tIME: ChunkType = ChunkType(*b"tIME");
/// Physical pixel dimensions
pub const pHYs: ChunkType = ChunkType(*b"pHYs");
/// Source system's pixel chromaticities
pub const cHRM: ChunkType = ChunkType(*b"cHRM");
/// Source system's gamma value
pub const gAMA: ChunkType = ChunkType(*b"gAMA");
/// sRGB color space chunk
pub const sRGB: ChunkType = ChunkType(*b"sRGB");
/// ICC profile chunk
pub const iCCP: ChunkType = ChunkType(*b"iCCP");
/// Coding-independent code points for video signal type identification chunk
pub const cICP: ChunkType = ChunkType(*b"cICP");
/// Mastering Display Color Volume chunk
pub const mDCV: ChunkType = ChunkType(*b"mDCV");
/// Content Light Level Information chunk
pub const cLLI: ChunkType = ChunkType(*b"cLLI");
/// EXIF metadata chunk
pub const eXIf: ChunkType = ChunkType(*b"eXIf");
/// Latin-1 uncompressed textual data
pub const tEXt: ChunkType = ChunkType(*b"tEXt");
/// Latin-1 compressed textual data
pub const zTXt: ChunkType = ChunkType(*b"zTXt");
/// UTF-8 textual data
pub const iTXt: ChunkType = ChunkType(*b"iTXt");
// Significant bits
pub const sBIT: ChunkType = ChunkType(*b"sBIT");

// -- Extension chunks --

/// Animation control
pub const acTL: ChunkType = ChunkType(*b"acTL");
/// Frame control
pub const fcTL: ChunkType = ChunkType(*b"fcTL");
/// Frame data
pub const fdAT: ChunkType = ChunkType(*b"fdAT");

// -- Chunk type determination --

/// Returns true if the chunk is critical.
pub fn is_critical(ChunkType(type_): ChunkType) -> bool {
    type_[0] & 32 == 0
}

/// Returns true if the chunk is private.
pub fn is_private(ChunkType(type_): ChunkType) -> bool {
    type_[1] & 32 != 0
}

/// Checks whether the reserved bit of the chunk name is set.
/// If it is set the chunk name is invalid.
pub fn reserved_set(ChunkType(type_): ChunkType) -> bool {
    type_[2] & 32 != 0
}

/// Returns true if the chunk is safe to copy if unknown.
pub fn safe_to_copy(ChunkType(type_): ChunkType) -> bool {
    type_[3] & 32 != 0
}

impl fmt::Debug for ChunkType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        struct DebugType([u8; 4]);

        impl fmt::Debug for DebugType {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                for &c in &self.0[..] {
                    write!(f, "{}", char::from(c).escape_debug())?;
                }
                Ok(())
            }
        }

        f.debug_struct("ChunkType")
            .field("type", &DebugType(self.0))
            .field("critical", &is_critical(*self))
            .field("private", &is_private(*self))
            .field("reserved", &reserved_set(*self))
            .field("safecopy", &safe_to_copy(*self))
            .finish()
    }
}
