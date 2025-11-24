//! All IO functionality needed for TIFF decoding
use std::io::{self, BufRead, BufReader, Read, Seek, Take};

/// Byte order of the TIFF file.
#[derive(Clone, Copy, Debug)]
pub enum ByteOrder {
    /// little endian byte order
    LittleEndian,
    /// big endian byte order
    BigEndian,
}

/// Reader that is aware of the byte order.
#[derive(Debug)]
pub struct EndianReader<R> {
    reader: R,
    pub(crate) byte_order: ByteOrder,
}

impl<R: Read> EndianReader<R> {
    pub fn new(reader: R, byte_order: ByteOrder) -> Self {
        Self { reader, byte_order }
    }

    pub fn inner(&mut self) -> &mut R {
        &mut self.reader
    }

    pub fn goto_offset(&mut self, offset: u64) -> io::Result<()>
    where
        R: Seek,
    {
        self.reader.seek(io::SeekFrom::Start(offset))?;
        Ok(())
    }

    /// Reads an u16
    #[inline(always)]
    pub fn read_u16(&mut self) -> Result<u16, io::Error> {
        let mut n = [0u8; 2];
        self.reader.read_exact(&mut n)?;
        Ok(match self.byte_order {
            ByteOrder::LittleEndian => u16::from_le_bytes(n),
            ByteOrder::BigEndian => u16::from_be_bytes(n),
        })
    }

    /// Reads an i8
    #[inline(always)]
    pub fn read_i8(&mut self) -> Result<i8, io::Error> {
        let mut n = [0u8; 1];
        self.reader.read_exact(&mut n)?;
        Ok(match self.byte_order {
            ByteOrder::LittleEndian => i8::from_le_bytes(n),
            ByteOrder::BigEndian => i8::from_be_bytes(n),
        })
    }

    /// Reads an i16
    #[inline(always)]
    pub fn read_i16(&mut self) -> Result<i16, io::Error> {
        let mut n = [0u8; 2];
        self.reader.read_exact(&mut n)?;
        Ok(match self.byte_order {
            ByteOrder::LittleEndian => i16::from_le_bytes(n),
            ByteOrder::BigEndian => i16::from_be_bytes(n),
        })
    }

    /// Reads an u32
    #[inline(always)]
    pub fn read_u32(&mut self) -> Result<u32, io::Error> {
        let mut n = [0u8; 4];
        self.reader.read_exact(&mut n)?;
        Ok(match self.byte_order {
            ByteOrder::LittleEndian => u32::from_le_bytes(n),
            ByteOrder::BigEndian => u32::from_be_bytes(n),
        })
    }

    /// Reads an i32
    #[inline(always)]
    pub fn read_i32(&mut self) -> Result<i32, io::Error> {
        let mut n = [0u8; 4];
        self.reader.read_exact(&mut n)?;
        Ok(match self.byte_order {
            ByteOrder::LittleEndian => i32::from_le_bytes(n),
            ByteOrder::BigEndian => i32::from_be_bytes(n),
        })
    }

    /// Reads an u64
    #[inline(always)]
    pub fn read_u64(&mut self) -> Result<u64, io::Error> {
        let mut n = [0u8; 8];
        self.reader.read_exact(&mut n)?;
        Ok(match self.byte_order {
            ByteOrder::LittleEndian => u64::from_le_bytes(n),
            ByteOrder::BigEndian => u64::from_be_bytes(n),
        })
    }

    /// Reads an i64
    #[inline(always)]
    pub fn read_i64(&mut self) -> Result<i64, io::Error> {
        let mut n = [0u8; 8];
        self.reader.read_exact(&mut n)?;
        Ok(match self.byte_order {
            ByteOrder::LittleEndian => i64::from_le_bytes(n),
            ByteOrder::BigEndian => i64::from_be_bytes(n),
        })
    }

    /// Reads an f32
    #[inline(always)]
    pub fn read_f32(&mut self) -> Result<f32, io::Error> {
        let mut n = [0u8; 4];
        self.reader.read_exact(&mut n)?;
        Ok(f32::from_bits(match self.byte_order {
            ByteOrder::LittleEndian => u32::from_le_bytes(n),
            ByteOrder::BigEndian => u32::from_be_bytes(n),
        }))
    }

    /// Reads an f64
    #[inline(always)]
    pub fn read_f64(&mut self) -> Result<f64, io::Error> {
        let mut n = [0u8; 8];
        self.reader.read_exact(&mut n)?;
        Ok(f64::from_bits(match self.byte_order {
            ByteOrder::LittleEndian => u64::from_le_bytes(n),
            ByteOrder::BigEndian => u64::from_be_bytes(n),
        }))
    }
}

//
// # READERS
//

/// Type alias for the deflate Reader
#[cfg(feature = "deflate")]
pub type DeflateReader<R> = flate2::read::ZlibDecoder<R>;

//
// ## LZW Reader
//

/// Reader that decompresses LZW streams
#[cfg(feature = "lzw")]
pub struct LZWReader<R: Read> {
    reader: BufReader<Take<R>>,
    decoder: weezl::decode::Decoder,
}

#[cfg(feature = "lzw")]
impl<R: Read> LZWReader<R> {
    /// Wraps a reader
    pub fn new(reader: R, compressed_length: usize) -> LZWReader<R> {
        let configuration =
            weezl::decode::Configuration::with_tiff_size_switch(weezl::BitOrder::Msb, 8)
                .with_yield_on_full_buffer(true);
        Self {
            reader: BufReader::with_capacity(
                (32 * 1024).min(compressed_length),
                reader.take(u64::try_from(compressed_length).unwrap()),
            ),
            decoder: configuration.build(),
        }
    }
}

#[cfg(feature = "lzw")]
impl<R: Read> Read for LZWReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let result = self.decoder.decode_bytes(self.reader.fill_buf()?, buf);
            self.reader.consume(result.consumed_in);

            match result.status {
                Ok(weezl::LzwStatus::Ok) => {
                    if result.consumed_out == 0 {
                        continue;
                    } else {
                        return Ok(result.consumed_out);
                    }
                }
                Ok(weezl::LzwStatus::NoProgress) => {
                    assert_eq!(result.consumed_in, 0);
                    assert_eq!(result.consumed_out, 0);
                    assert!(self.reader.buffer().is_empty());
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "no lzw end code found",
                    ));
                }
                Ok(weezl::LzwStatus::Done) => {
                    return Ok(result.consumed_out);
                }
                Err(err) => return Err(io::Error::new(io::ErrorKind::InvalidData, err)),
            }
        }
    }
}

//
// ## PackBits Reader
//

/// Internal state machine for the PackBitsReader.
enum PackBitsReaderState {
    Header,
    Literal,
    Repeat { value: u8 },
}

/// Reader that unpacks Apple's `PackBits` format
pub struct PackBitsReader<R: Read> {
    reader: Take<R>,
    state: PackBitsReaderState,
    count: usize,
}

impl<R: Read> PackBitsReader<R> {
    /// Wraps a reader
    pub fn new(reader: R, length: u64) -> Self {
        Self {
            reader: reader.take(length),
            state: PackBitsReaderState::Header,
            count: 0,
        }
    }
}

impl<R: Read> Read for PackBitsReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        while let PackBitsReaderState::Header = self.state {
            if self.reader.limit() == 0 {
                return Ok(0);
            }
            let mut header: [u8; 1] = [0];
            self.reader.read_exact(&mut header)?;
            let h = header[0] as i8;
            if (-127..=-1).contains(&h) {
                let mut data: [u8; 1] = [0];
                self.reader.read_exact(&mut data)?;
                self.state = PackBitsReaderState::Repeat { value: data[0] };
                self.count = (1 - h as isize) as usize;
            } else if h >= 0 {
                self.state = PackBitsReaderState::Literal;
                self.count = h as usize + 1;
            } else {
                // h = -128 is a no-op.
            }
        }

        let length = buf.len().min(self.count);
        let actual = match self.state {
            PackBitsReaderState::Literal => self.reader.read(&mut buf[..length])?,
            PackBitsReaderState::Repeat { value } => {
                for b in &mut buf[..length] {
                    *b = value;
                }

                length
            }
            PackBitsReaderState::Header => unreachable!(),
        };

        self.count -= actual;
        if self.count == 0 {
            self.state = PackBitsReaderState::Header;
        }
        Ok(actual)
    }
}

#[cfg(feature = "fax")]
pub struct Group4Reader<R: Read> {
    decoder: fax34::decoder::Group4Decoder<io::Bytes<io::BufReader<io::Take<R>>>>,
    line_buf: io::Cursor<Vec<u8>>,
    height: u16,
    width: u16,
    y: u16,
}

#[cfg(feature = "fax")]
impl<R: Read> Group4Reader<R> {
    pub fn new(
        dimensions: (u32, u32),
        reader: R,
        compressed_length: u64,
    ) -> crate::TiffResult<Self> {
        let width = u16::try_from(dimensions.0)?;
        let height = u16::try_from(dimensions.1)?;

        Ok(Self {
            decoder: fax34::decoder::Group4Decoder::new(
                io::BufReader::new(reader.take(compressed_length)).bytes(),
                width,
            )?,
            line_buf: io::Cursor::new(Vec::with_capacity(width.into())),
            width,
            height,
            y: 0,
        })
    }
}

#[cfg(feature = "fax")]
impl<R: Read> Read for Group4Reader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Either we have not read any line or we are at the end of a line.
        if self.line_buf.position() as usize == self.line_buf.get_ref().len()
            && self.y < self.height
        {
            let next = self.decoder.advance().map_err(std::io::Error::other)?;

            match next {
                fax34::decoder::DecodeStatus::End => (),
                fax34::decoder::DecodeStatus::Incomplete => {
                    self.y += 1;

                    // We known `transitions` yields exactly `self.width` items (per doc).
                    // FIXME: performance. We do not need an individual pixel iterator, filling
                    // memory especially for long runs can be much quicker. The `transitions` are
                    // the positions at which each run-length ends, i.e. the prefix sum of run
                    // lengths. Runs in fax4 start with white.
                    let transitions = fax34::decoder::pels(self.decoder.transition(), self.width);

                    let buffer = self.line_buf.get_mut();
                    buffer.resize(usize::from(self.width).div_ceil(8), 0u8);

                    let target = &mut buffer[..];

                    // Note: it may seem strange to treat black as 0b1 and white as 0b0 despite all
                    // our streams by default decoding as-if PhotometricInterpretation::BlackIsMin.
                    // This is however consistent with libtiff. It seems that fax4's "White"
                    // differs from what libtiff thinks of as "White". For content, a line of data
                    // is in runlength encoding of white-black-white-black always starting with
                    // white. In libtiff, the loop always does both colors in one go and the
                    // structure is:
                    //
                    // ```
                    // for (; runs < erun; runs += 2)
                    //   // white run
                    //       do { *lp++ = 0L; } while (…)
                    //   // black run
                    //       do { *lp++ = -1L; } while (…)
                    // ```
                    //
                    // So indeed the Fax4::White run is implemented by filling with zeros.
                    let mut bits = transitions.map(|c| match c {
                        fax34::Color::Black => true,
                        fax34::Color::White => false,
                    });

                    // Assemble bits in MSB as per our library representation for buffer.
                    for byte in target {
                        let mut val = 0;

                        for (idx, bit) in bits.by_ref().take(8).enumerate() {
                            val |= u8::from(bit) << (7 - idx % 8);
                        }

                        *byte = val;
                    }

                    self.line_buf.set_position(0);
                }
            }
        }

        self.line_buf.read(buf)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_packbits() {
        let encoded = vec![
            0xFE, 0xAA, 0x02, 0x80, 0x00, 0x2A, 0xFD, 0xAA, 0x03, 0x80, 0x00, 0x2A, 0x22, 0xF7,
            0xAA,
        ];
        let encoded_len = encoded.len();

        let buff = io::Cursor::new(encoded);
        let mut decoder = PackBitsReader::new(buff, encoded_len as u64);

        let mut decoded = Vec::new();
        decoder.read_to_end(&mut decoded).unwrap();

        let expected = vec![
            0xAA, 0xAA, 0xAA, 0x80, 0x00, 0x2A, 0xAA, 0xAA, 0xAA, 0xAA, 0x80, 0x00, 0x2A, 0x22,
            0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
        ];
        assert_eq!(decoded, expected);
    }
}
