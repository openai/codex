//! Archive definitions.
//!
//! These definitions are independent of read/write support, although we do implement
//! some traits useful for those.

use crate::pod::Pod;

/// File identification bytes stored at the beginning of the file.
pub const MAGIC: [u8; 8] = *b"!<arch>\n";

/// File identification bytes at the beginning of AIX big archive.
pub const AIX_BIG_MAGIC: [u8; 8] = *b"<bigaf>\n";

/// File identification bytes stored at the beginning of a thin archive.
///
/// A thin archive only contains a symbol table and file names.
pub const THIN_MAGIC: [u8; 8] = *b"!<thin>\n";

/// The terminator for each archive member header.
pub const TERMINATOR: [u8; 2] = *b"`\n";

/// The header at the start of an archive member.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Header {
    /// The file name.
    pub name: [u8; 16],
    /// File modification timestamp in decimal.
    pub date: [u8; 12],
    /// User ID in decimal.
    pub uid: [u8; 6],
    /// Group ID in decimal.
    pub gid: [u8; 6],
    /// File mode in octal.
    pub mode: [u8; 8],
    /// File size in decimal.
    pub size: [u8; 10],
    /// Must be equal to `TERMINATOR`.
    pub terminator: [u8; 2],
}

/// The header at the start of an AIX big archive member, without name.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct AixHeader {
    /// File member size in decimal.
    pub size: [u8; 20],
    /// Next member offset in decimal.
    pub nxtmem: [u8; 20],
    /// Previous member offset in decimal.
    pub prvmem: [u8; 20],
    /// File member date in decimal.
    pub date: [u8; 12],
    /// File member user id in decimal.
    pub uid: [u8; 12],
    /// File member group id in decimal.
    pub gid: [u8; 12],
    /// File member mode in octal.
    pub mode: [u8; 12],
    /// File member name length in decimal.
    pub namlen: [u8; 4],
}

/// The AIX big archive's fixed length header at file beginning.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct AixFileHeader {
    /// Archive magic string.
    pub magic: [u8; 8],
    /// Offset of member table.
    pub memoff: [u8; 20],
    /// Offset of global symbol table.
    pub gstoff: [u8; 20],
    /// Offset of global symbol table for 64-bit objects.
    pub gst64off: [u8; 20],
    /// Offset of first member.
    pub fstmoff: [u8; 20],
    /// Offset of last member.
    pub lstmoff: [u8; 20],
    /// Offset of first member on free list.
    pub freeoff: [u8; 20],
}

/// Offset of a member in an AIX big archive.
///
/// This is used in the member index.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct AixMemberOffset(pub [u8; 20]);

unsafe_impl_pod!(Header, AixHeader, AixFileHeader, AixMemberOffset,);
