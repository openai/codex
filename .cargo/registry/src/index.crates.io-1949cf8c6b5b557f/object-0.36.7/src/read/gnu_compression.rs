use crate::read::{self, Error, ReadError as _};
use crate::{endian, CompressedFileRange, CompressionFormat, ReadRef, U32Bytes};

// Attempt to parse the the CompressedFileRange for a section using the GNU-style
// inline compression header format. This is used by the Go compiler in Mach-O files
// as well as by the GNU linker in some ELF files.
pub(super) fn compressed_file_range<'data, R: ReadRef<'data>>(
    file_data: R,
    section_offset: u64,
    section_size: u64,
) -> read::Result<CompressedFileRange> {
    let mut offset = section_offset;
    // Assume ZLIB-style uncompressed data is no more than 4GB to avoid accidentally
    // huge allocations. This also reduces the chance of accidentally matching on a
    // .debug_str that happens to start with "ZLIB".
    let header = file_data
        .read_bytes(&mut offset, 8)
        .read_error("GNU compressed section is too short")?;
    if header != b"ZLIB\0\0\0\0" {
        return Err(Error("Invalid GNU compressed section header"));
    }
    let uncompressed_size = file_data
        .read::<U32Bytes<_>>(&mut offset)
        .read_error("GNU compressed section is too short")?
        .get(endian::BigEndian)
        .into();
    let compressed_size = section_size
        .checked_sub(offset - section_offset)
        .read_error("GNU compressed section is too short")?;
    Ok(CompressedFileRange {
        format: CompressionFormat::Zlib,
        offset,
        compressed_size,
        uncompressed_size,
    })
}
