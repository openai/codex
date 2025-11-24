use crate::encoder::compression::*;
use crate::error::TiffResult;
use std::io::{self, Seek, SeekFrom, Write};

pub fn write_tiff_header<W: Write>(writer: &mut TiffWriter<W>) -> TiffResult<()> {
    #[cfg(target_endian = "little")]
    let boi: u8 = 0x49;
    #[cfg(not(target_endian = "little"))]
    let boi: u8 = 0x4d;

    writer.writer.write_all(&[boi, boi])?;
    writer.writer.write_all(&42u16.to_ne_bytes())?;
    writer.offset += 4;

    Ok(())
}

/// Writes a BigTiff header, excluding the IFD offset field.
///
/// Writes the byte order, version number, offset byte size, and zero constant fields. Does
// _not_ write the offset to the first IFD, this should be done by the caller.
pub fn write_bigtiff_header<W: Write>(writer: &mut TiffWriter<W>) -> TiffResult<()> {
    #[cfg(target_endian = "little")]
    let boi: u8 = 0x49;
    #[cfg(not(target_endian = "little"))]
    let boi: u8 = 0x4d;

    // byte order indication
    writer.writer.write_all(&[boi, boi])?;
    // version number
    writer.writer.write_all(&43u16.to_ne_bytes())?;
    // bytesize of offsets (pointer size)
    writer.writer.write_all(&8u16.to_ne_bytes())?;
    // always 0
    writer.writer.write_all(&0u16.to_ne_bytes())?;

    // we wrote 8 bytes, so set the internal offset accordingly
    writer.offset += 8;

    Ok(())
}

pub struct TiffWriter<W> {
    writer: W,
    offset: u64,
    byte_count: u64,
    compressor: Compressor,
}

impl<W: Write> TiffWriter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            offset: 0,
            byte_count: 0,
            compressor: Compressor::default(),
        }
    }

    pub fn set_compression(&mut self, compressor: Compressor) {
        self.compressor = compressor;
    }

    pub fn reset_compression(&mut self) {
        self.compressor = Compressor::default();
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub(crate) fn previous_ifd_pointer<K: super::TiffKind>(&self) -> u64 {
        self.offset() - core::mem::size_of::<K::OffsetType>() as u64
    }

    pub fn last_written(&self) -> u64 {
        self.byte_count
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), io::Error> {
        self.byte_count = self.compressor.write_to(&mut self.writer, bytes)?;
        self.offset += self.byte_count;
        Ok(())
    }

    pub fn write_u8(&mut self, n: u8) -> Result<(), io::Error> {
        self.byte_count = self
            .compressor
            .write_to(&mut self.writer, &n.to_ne_bytes())?;
        self.offset += self.byte_count;
        Ok(())
    }

    pub fn write_i8(&mut self, n: i8) -> Result<(), io::Error> {
        self.byte_count = self
            .compressor
            .write_to(&mut self.writer, &n.to_ne_bytes())?;
        self.offset += self.byte_count;
        Ok(())
    }

    pub fn write_u16(&mut self, n: u16) -> Result<(), io::Error> {
        self.byte_count = self
            .compressor
            .write_to(&mut self.writer, &n.to_ne_bytes())?;
        self.offset += self.byte_count;

        Ok(())
    }

    pub fn write_i16(&mut self, n: i16) -> Result<(), io::Error> {
        self.byte_count = self
            .compressor
            .write_to(&mut self.writer, &n.to_ne_bytes())?;
        self.offset += self.byte_count;

        Ok(())
    }

    pub fn write_u32(&mut self, n: u32) -> Result<(), io::Error> {
        self.byte_count = self
            .compressor
            .write_to(&mut self.writer, &n.to_ne_bytes())?;
        self.offset += self.byte_count;

        Ok(())
    }

    pub fn write_i32(&mut self, n: i32) -> Result<(), io::Error> {
        self.byte_count = self
            .compressor
            .write_to(&mut self.writer, &n.to_ne_bytes())?;
        self.offset += self.byte_count;

        Ok(())
    }

    pub fn write_u64(&mut self, n: u64) -> Result<(), io::Error> {
        self.byte_count = self
            .compressor
            .write_to(&mut self.writer, &n.to_ne_bytes())?;
        self.offset += self.byte_count;

        Ok(())
    }

    pub fn write_i64(&mut self, n: i64) -> Result<(), io::Error> {
        self.byte_count = self
            .compressor
            .write_to(&mut self.writer, &n.to_ne_bytes())?;
        self.offset += self.byte_count;

        Ok(())
    }

    pub fn write_f32(&mut self, n: f32) -> Result<(), io::Error> {
        self.byte_count = self
            .compressor
            .write_to(&mut self.writer, &u32::to_ne_bytes(n.to_bits()))?;
        self.offset += self.byte_count;

        Ok(())
    }

    pub fn write_f64(&mut self, n: f64) -> Result<(), io::Error> {
        self.byte_count = self
            .compressor
            .write_to(&mut self.writer, &u64::to_ne_bytes(n.to_bits()))?;
        self.offset += self.byte_count;

        Ok(())
    }

    pub fn pad_word_boundary(&mut self) -> Result<(), io::Error> {
        if self.offset % 4 != 0 {
            let padding = [0, 0, 0];
            let padd_len = 4 - (self.offset % 4);
            self.writer.write_all(&padding[..padd_len as usize])?;
            self.offset += padd_len;
        }

        Ok(())
    }
}

impl<W: Seek> TiffWriter<W> {
    pub fn goto_offset(&mut self, offset: u64) -> Result<(), io::Error> {
        self.offset = offset;
        self.writer.seek(SeekFrom::Start(offset))?;
        Ok(())
    }
}
