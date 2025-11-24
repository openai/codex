use simd_adler32::Adler32;
use std::io::{self, Seek, SeekFrom, Write};

use crate::tables::{
    BITMASKS, HUFFMAN_CODES, HUFFMAN_LENGTHS, LENGTH_TO_LEN_EXTRA, LENGTH_TO_SYMBOL,
};

/// Compressor that produces fdeflate compressed streams.
pub struct Compressor<W: Write> {
    checksum: Adler32,
    buffer: u64,
    nbits: u8,
    writer: W,
}
impl<W: Write> Compressor<W> {
    fn write_bits(&mut self, bits: u64, nbits: u8) -> io::Result<()> {
        debug_assert!(nbits <= 64);

        self.buffer |= bits << self.nbits;
        self.nbits += nbits;

        if self.nbits >= 64 {
            self.writer.write_all(&self.buffer.to_le_bytes())?;
            self.nbits -= 64;
            self.buffer = bits.checked_shr((nbits - self.nbits) as u32).unwrap_or(0);
        }
        debug_assert!(self.nbits < 64);
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.nbits % 8 != 0 {
            self.write_bits(0, 8 - self.nbits % 8)?;
        }
        if self.nbits > 0 {
            self.writer
                .write_all(&self.buffer.to_le_bytes()[..self.nbits as usize / 8])
                .unwrap();
            self.buffer = 0;
            self.nbits = 0;
        }
        Ok(())
    }

    fn write_run(&mut self, mut run: u32) -> io::Result<()> {
        self.write_bits(HUFFMAN_CODES[0] as u64, HUFFMAN_LENGTHS[0])?;
        run -= 1;

        while run >= 258 {
            self.write_bits(HUFFMAN_CODES[285] as u64, HUFFMAN_LENGTHS[285] + 1)?;
            run -= 258;
        }

        if run > 4 {
            let sym = LENGTH_TO_SYMBOL[run as usize - 3] as usize;
            self.write_bits(HUFFMAN_CODES[sym] as u64, HUFFMAN_LENGTHS[sym])?;

            let len_extra = LENGTH_TO_LEN_EXTRA[run as usize - 3];
            let extra = ((run - 3) & BITMASKS[len_extra as usize]) as u64;
            self.write_bits(extra, len_extra + 1)?;
        } else {
            debug_assert_eq!(HUFFMAN_CODES[0], 0);
            self.write_bits(0, run as u8 * HUFFMAN_LENGTHS[0])?;
        }

        Ok(())
    }

    /// Create a new Compressor.
    pub fn new(writer: W) -> io::Result<Self> {
        let mut compressor = Self {
            checksum: Adler32::new(),
            buffer: 0,
            nbits: 0,
            writer,
        };
        compressor.write_headers()?;
        Ok(compressor)
    }

    fn write_headers(&mut self) -> io::Result<()> {
        const HEADER: [u8; 54] = [
            120, 1, 237, 192, 3, 160, 36, 89, 150, 198, 241, 255, 119, 238, 141, 200, 204, 167,
            114, 75, 99, 174, 109, 219, 182, 109, 219, 182, 109, 219, 182, 109, 105, 140, 158, 150,
            74, 175, 158, 50, 51, 34, 238, 249, 118, 183, 106, 122, 166, 135, 59, 107, 213, 15,
        ];
        self.writer.write_all(&HEADER[..53]).unwrap();
        self.write_bits(HEADER[53] as u64, 5)?;

        Ok(())
    }

    /// Write data to the compressor.
    pub fn write_data(&mut self, data: &[u8]) -> io::Result<()> {
        self.checksum.write(data);

        let mut run = 0;
        let mut chunks = data.chunks_exact(8);
        for chunk in &mut chunks {
            let ichunk = u64::from_le_bytes(chunk.try_into().unwrap());

            if ichunk == 0 {
                run += 8;
                continue;
            } else if run > 0 {
                let run_extra = ichunk.trailing_zeros() / 8;
                self.write_run(run + run_extra)?;
                run = 0;

                if run_extra > 0 {
                    run = ichunk.leading_zeros() / 8;
                    for &b in &chunk[run_extra as usize..8 - run as usize] {
                        self.write_bits(
                            HUFFMAN_CODES[b as usize] as u64,
                            HUFFMAN_LENGTHS[b as usize],
                        )?;
                    }
                    continue;
                }
            }

            let run_start = ichunk.leading_zeros() / 8;
            if run_start > 0 {
                for &b in &chunk[..8 - run_start as usize] {
                    self.write_bits(
                        HUFFMAN_CODES[b as usize] as u64,
                        HUFFMAN_LENGTHS[b as usize],
                    )?;
                }
                run = run_start;
                continue;
            }

            let n0 = HUFFMAN_LENGTHS[chunk[0] as usize];
            let n1 = HUFFMAN_LENGTHS[chunk[1] as usize];
            let n2 = HUFFMAN_LENGTHS[chunk[2] as usize];
            let n3 = HUFFMAN_LENGTHS[chunk[3] as usize];
            let bits = HUFFMAN_CODES[chunk[0] as usize] as u64
                | ((HUFFMAN_CODES[chunk[1] as usize] as u64) << n0)
                | ((HUFFMAN_CODES[chunk[2] as usize] as u64) << (n0 + n1))
                | ((HUFFMAN_CODES[chunk[3] as usize] as u64) << (n0 + n1 + n2));
            self.write_bits(bits, n0 + n1 + n2 + n3)?;

            let n4 = HUFFMAN_LENGTHS[chunk[4] as usize];
            let n5 = HUFFMAN_LENGTHS[chunk[5] as usize];
            let n6 = HUFFMAN_LENGTHS[chunk[6] as usize];
            let n7 = HUFFMAN_LENGTHS[chunk[7] as usize];
            let bits2 = HUFFMAN_CODES[chunk[4] as usize] as u64
                | ((HUFFMAN_CODES[chunk[5] as usize] as u64) << n4)
                | ((HUFFMAN_CODES[chunk[6] as usize] as u64) << (n4 + n5))
                | ((HUFFMAN_CODES[chunk[7] as usize] as u64) << (n4 + n5 + n6));
            self.write_bits(bits2, n4 + n5 + n6 + n7)?;
        }

        if run > 0 {
            self.write_run(run)?;
        }

        for &b in chunks.remainder() {
            self.write_bits(
                HUFFMAN_CODES[b as usize] as u64,
                HUFFMAN_LENGTHS[b as usize],
            )?;
        }

        Ok(())
    }

    /// Write the remainder of the stream and return the inner writer.
    pub fn finish(mut self) -> io::Result<W> {
        // Write end of block
        self.write_bits(HUFFMAN_CODES[256] as u64, HUFFMAN_LENGTHS[256])?;
        self.flush()?;

        // Write Adler32 checksum
        let checksum: u32 = self.checksum.finish();
        self.writer
            .write_all(checksum.to_be_bytes().as_ref())
            .unwrap();
        Ok(self.writer)
    }
}

/// Compressor that only writes the stored blocks.
///
/// This is useful for writing files that are not compressed, but still need to be wrapped in a
/// zlib stream.
pub struct StoredOnlyCompressor<W> {
    writer: W,
    checksum: Adler32,
    block_bytes: u16,
}
impl<W: Write + Seek> StoredOnlyCompressor<W> {
    /// Creates a new `StoredOnlyCompressor` that writes to the given writer.
    pub fn new(mut writer: W) -> io::Result<Self> {
        writer.write_all(&[0x78, 0x01])?; // zlib header
        writer.write_all(&[0; 5])?; // placeholder stored block header

        Ok(Self {
            writer,
            checksum: Adler32::new(),
            block_bytes: 0,
        })
    }

    fn set_block_header(&mut self, size: u16, last: bool) -> io::Result<()> {
        self.writer.seek(SeekFrom::Current(-(size as i64 + 5)))?;
        self.writer.write_all(&[
            last as u8,
            (size & 0xFF) as u8,
            ((size >> 8) & 0xFF) as u8,
            (!size & 0xFF) as u8,
            ((!size >> 8) & 0xFF) as u8,
        ])?;
        self.writer.seek(SeekFrom::Current(size as i64))?;

        Ok(())
    }

    /// Writes the given data to the underlying writer.
    pub fn write_data(&mut self, mut data: &[u8]) -> io::Result<()> {
        self.checksum.write(data);
        while !data.is_empty() {
            if self.block_bytes == u16::MAX {
                self.set_block_header(u16::MAX, false)?;
                self.writer.write_all(&[0; 5])?; // placeholder stored block header
                self.block_bytes = 0;
            }

            let prefix_bytes = data.len().min((u16::MAX - self.block_bytes) as usize);
            self.writer.write_all(&data[..prefix_bytes])?;
            self.block_bytes += prefix_bytes as u16;
            data = &data[prefix_bytes..];
        }

        Ok(())
    }

    /// Finish writing the final block and return the underlying writer.
    pub fn finish(mut self) -> io::Result<W> {
        self.set_block_header(self.block_bytes, true)?;

        // Write Adler32 checksum
        let checksum: u32 = self.checksum.finish();
        self.writer
            .write_all(checksum.to_be_bytes().as_ref())
            .unwrap();

        Ok(self.writer)
    }
}
impl<W> StoredOnlyCompressor<W> {
    /// Return the number of bytes that will be written to the output stream
    /// for the given input size. Because this compressor only writes stored blocks,
    /// the output size is always slightly *larger* than the input size.
    pub fn compressed_size(raw_size: usize) -> usize {
        (raw_size.saturating_sub(1) / u16::MAX as usize) * (u16::MAX as usize + 5)
            + (raw_size % u16::MAX as usize + 5)
            + 6
    }
}

/// Compresses the given data.
pub fn compress_to_vec(input: &[u8]) -> Vec<u8> {
    let mut compressor = Compressor::new(Vec::with_capacity(input.len() / 4)).unwrap();
    compressor.write_data(input).unwrap();
    compressor.finish().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    fn roundtrip(data: &[u8]) {
        let compressed = compress_to_vec(data);
        let decompressed = miniz_oxide::inflate::decompress_to_vec_zlib(&compressed).unwrap();
        assert_eq!(&decompressed, data);
    }

    #[test]
    fn it_works() {
        roundtrip(b"Hello world!");
    }

    #[test]
    fn constant() {
        roundtrip(&vec![0; 2048]);
        roundtrip(&vec![5; 2048]);
        roundtrip(&vec![128; 2048]);
        roundtrip(&vec![254; 2048]);
    }

    #[test]
    fn random() {
        let mut rng = rand::thread_rng();
        let mut data = vec![0; 2048];
        for _ in 0..10 {
            for byte in &mut data {
                *byte = rng.gen();
            }
            roundtrip(&data);
        }
    }
}
