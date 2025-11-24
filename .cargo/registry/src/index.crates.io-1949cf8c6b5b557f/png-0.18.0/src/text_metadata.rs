//! # Text chunks (tEXt/zTXt/iTXt) structs and functions
//!
//! The [PNG spec](https://www.w3.org/TR/2003/REC-PNG-20031110/#11textinfo) optionally allows for
//! embedded text chunks in the file. They may appear either before or after the image data
//! chunks. There are three kinds of text chunks.
//!  -   `tEXt`: This has a `keyword` and `text` field, and is ISO 8859-1 encoded.
//!  -   `zTXt`: This is semantically the same as `tEXt`, i.e. it has the same fields and
//!      encoding, but the `text` field is compressed before being written into the PNG file.
//!  -   `iTXt`: This chunk allows for its `text` field to be any valid UTF-8, and supports
//!      compression of the text field as well.
//!
//!  The `ISO 8859-1` encoding technically doesn't allow any control characters
//!  to be used, but in practice these values are encountered anyway. This can
//!  either be the extended `ISO-8859-1` encoding with control characters or the
//!  `Windows-1252` encoding. This crate assumes the `ISO-8859-1` encoding is
//!  used.
//!
//!  ## Reading text chunks
//!
//!  As a PNG is decoded, any text chunk encountered is appended the
//!  [`Info`](`crate::common::Info`) struct, in the `uncompressed_latin1_text`,
//!  `compressed_latin1_text`, and the `utf8_text` fields depending on whether the encountered
//!  chunk is `tEXt`, `zTXt`, or `iTXt`.
//!
//!  ```
//!  use std::fs::File;
//!  use std::io::BufReader;
//!  use std::iter::FromIterator;
//!  use std::path::PathBuf;
//!
//!  // Opening a png file that has a zTXt chunk
//!  let decoder = png::Decoder::new(
//!      BufReader::new(File::open("tests/text_chunk_examples/ztxt_example.png").unwrap())
//!  );
//!  let mut reader = decoder.read_info().unwrap();
//!  // If the text chunk is before the image data frames, `reader.info()` already contains the text.
//!  for text_chunk in &reader.info().compressed_latin1_text {
//!      println!("{:?}", text_chunk.keyword); // Prints the keyword
//!      println!("{:#?}", text_chunk); // Prints out the text chunk.
//!      // To get the uncompressed text, use the `get_text` method.
//!      println!("{}", text_chunk.get_text().unwrap());
//!  }
//!  ```
//!
//!  ## Writing text chunks
//!
//!  There are two ways to write text chunks: the first is to add the appropriate text structs directly to the encoder header before the header is written to file.
//!  To add a text chunk at any point in the stream, use the `write_text_chunk` method.
//!
//!  ```
//!  # use png::text_metadata::{ITXtChunk, ZTXtChunk};
//!  # use std::env;
//!  # use std::fs::File;
//!  # use std::io::BufWriter;
//!  # use std::iter::FromIterator;
//!  # use std::path::PathBuf;
//!  # let file = File::create(PathBuf::from_iter(["target", "text_chunk.png"])).unwrap();
//!  # let ref mut w = BufWriter::new(file);
//!  let mut encoder = png::Encoder::new(w, 2, 1); // Width is 2 pixels and height is 1.
//!  encoder.set_color(png::ColorType::Rgba);
//!  encoder.set_depth(png::BitDepth::Eight);
//!  // Adding text chunks to the header
//!  encoder
//!     .add_text_chunk(
//!         "Testing tEXt".to_string(),
//!         "This is a tEXt chunk that will appear before the IDAT chunks.".to_string(),
//!     )
//!     .unwrap();
//!  encoder
//!      .add_ztxt_chunk(
//!          "Testing zTXt".to_string(),
//!          "This is a zTXt chunk that is compressed in the png file.".to_string(),
//!      )
//!      .unwrap();
//!  encoder
//!      .add_itxt_chunk(
//!          "Testing iTXt".to_string(),
//!          "iTXt chunks support all of UTF8. Example: हिंदी.".to_string(),
//!      )
//!      .unwrap();
//!
//!  let mut writer = encoder.write_header().unwrap();
//!
//!  let data = [255, 0, 0, 255, 0, 0, 0, 255]; // An array containing a RGBA sequence. First pixel is red and second pixel is black.
//!  writer.write_image_data(&data).unwrap(); // Save
//!
//!  // We can add a tEXt/zTXt/iTXt at any point before the encoder is dropped from scope. These chunks will be at the end of the png file.
//!  let tail_ztxt_chunk = ZTXtChunk::new("Comment".to_string(), "A zTXt chunk after the image data.".to_string());
//!  writer.write_text_chunk(&tail_ztxt_chunk).unwrap();
//!
//!  // The fields of the text chunk are public, so they can be mutated before being written to the file.
//!  let mut tail_itxt_chunk = ITXtChunk::new("Author".to_string(), "सायंतन खान".to_string());
//!  tail_itxt_chunk.compressed = true;
//!  tail_itxt_chunk.language_tag = "hi".to_string();
//!  tail_itxt_chunk.translated_keyword = "लेखक".to_string();
//!  writer.write_text_chunk(&tail_itxt_chunk).unwrap();
//!  ```

#![warn(missing_docs)]

use crate::{chunk, encoder, DecodingError, EncodingError};
use fdeflate::BoundedDecompressionError;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::{convert::TryFrom, io::Write};

/// Default decompression limit for compressed text chunks.
pub const DECOMPRESSION_LIMIT: usize = 2097152; // 2 MiB

/// Text encoding errors that is wrapped by the standard EncodingError type
#[derive(Debug, Clone, Copy)]
pub(crate) enum TextEncodingError {
    /// Unrepresentable characters in string
    Unrepresentable,
    /// Keyword longer than 79 bytes or empty
    InvalidKeywordSize,
    /// Error encountered while compressing text
    CompressionError,
}

/// Text decoding error that is wrapped by the standard DecodingError type
#[derive(Debug, Clone, Copy)]
pub(crate) enum TextDecodingError {
    /// Unrepresentable characters in string
    Unrepresentable,
    /// Keyword longer than 79 bytes or empty
    InvalidKeywordSize,
    /// Missing null separator
    MissingNullSeparator,
    /// Compressed text cannot be uncompressed
    InflationError,
    /// Needs more space to decompress
    OutOfDecompressionSpace,
    /// Using an unspecified value for the compression method
    InvalidCompressionMethod,
    /// Using a byte that is not 0 or 255 as compression flag in iTXt chunk
    InvalidCompressionFlag,
    /// Missing the compression flag
    MissingCompressionFlag,
}

/// A generalized text chunk trait
pub trait EncodableTextChunk {
    /// Encode text chunk as `Vec<u8>` to a `Write`
    fn encode<W: Write>(&self, w: &mut W) -> Result<(), EncodingError>;
}

/// Struct representing a tEXt chunk
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TEXtChunk {
    /// Keyword field of the tEXt chunk. Needs to be between 1-79 bytes when encoded as Latin-1.
    pub keyword: String,
    /// Text field of tEXt chunk. Can be at most 2GB.
    pub text: String,
}

fn decode_iso_8859_1(text: &[u8]) -> String {
    text.iter().map(|&b| b as char).collect()
}

pub(crate) fn encode_iso_8859_1(text: &str) -> Result<Vec<u8>, TextEncodingError> {
    encode_iso_8859_1_iter(text).collect()
}

fn encode_iso_8859_1_into(buf: &mut Vec<u8>, text: &str) -> Result<(), TextEncodingError> {
    for b in encode_iso_8859_1_iter(text) {
        buf.push(b?);
    }
    Ok(())
}

fn encode_iso_8859_1_iter(text: &str) -> impl Iterator<Item = Result<u8, TextEncodingError>> + '_ {
    text.chars()
        .map(|c| u8::try_from(c as u32).map_err(|_| TextEncodingError::Unrepresentable))
}

fn decode_ascii(text: &[u8]) -> Result<&str, TextDecodingError> {
    if text.is_ascii() {
        // `from_utf8` cannot panic because we're already checked that `text` is ASCII-7.
        // And this is the only safe way to get ASCII-7 string from `&[u8]`.
        Ok(std::str::from_utf8(text).expect("unreachable"))
    } else {
        Err(TextDecodingError::Unrepresentable)
    }
}

impl TEXtChunk {
    /// Constructs a new TEXtChunk.
    /// Not sure whether it should take &str or String.
    pub fn new(keyword: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            keyword: keyword.into(),
            text: text.into(),
        }
    }

    /// Decodes a slice of bytes to a String using Latin-1 decoding.
    /// The decoder runs in strict mode, and any decoding errors are passed along to the caller.
    pub(crate) fn decode(
        keyword_slice: &[u8],
        text_slice: &[u8],
    ) -> Result<Self, TextDecodingError> {
        if keyword_slice.is_empty() || keyword_slice.len() > 79 {
            return Err(TextDecodingError::InvalidKeywordSize);
        }

        Ok(Self {
            keyword: decode_iso_8859_1(keyword_slice),
            text: decode_iso_8859_1(text_slice),
        })
    }
}

impl EncodableTextChunk for TEXtChunk {
    /// Encodes TEXtChunk to a Writer. The keyword and text are separated by a byte of zeroes.
    fn encode<W: Write>(&self, w: &mut W) -> Result<(), EncodingError> {
        let mut data = encode_iso_8859_1(&self.keyword)?;

        if data.is_empty() || data.len() > 79 {
            return Err(TextEncodingError::InvalidKeywordSize.into());
        }

        data.push(0);

        encode_iso_8859_1_into(&mut data, &self.text)?;

        encoder::write_chunk(w, chunk::tEXt, &data)
    }
}

/// Struct representing a zTXt chunk
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZTXtChunk {
    /// Keyword field of the tEXt chunk. Needs to be between 1-79 bytes when encoded as Latin-1.
    pub keyword: String,
    /// Text field of zTXt chunk. It is compressed by default, but can be uncompressed if necessary.
    text: OptCompressed,
}

/// Private enum encoding the compressed and uncompressed states of zTXt/iTXt text field.
#[derive(Clone, Debug, PartialEq, Eq)]
enum OptCompressed {
    /// Compressed version of text field. Can be at most 2GB.
    Compressed(Vec<u8>),
    /// Uncompressed text field.
    Uncompressed(String),
}

impl ZTXtChunk {
    /// Creates a new ZTXt chunk.
    pub fn new(keyword: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            keyword: keyword.into(),
            text: OptCompressed::Uncompressed(text.into()),
        }
    }

    pub(crate) fn decode(
        keyword_slice: &[u8],
        compression_method: u8,
        text_slice: &[u8],
    ) -> Result<Self, TextDecodingError> {
        if keyword_slice.is_empty() || keyword_slice.len() > 79 {
            return Err(TextDecodingError::InvalidKeywordSize);
        }

        if compression_method != 0 {
            return Err(TextDecodingError::InvalidCompressionMethod);
        }

        Ok(Self {
            keyword: decode_iso_8859_1(keyword_slice),
            text: OptCompressed::Compressed(text_slice.to_vec()),
        })
    }

    /// Decompresses the inner text, mutating its own state. Can only handle decompressed text up to `DECOMPRESSION_LIMIT` bytes.
    pub fn decompress_text(&mut self) -> Result<(), DecodingError> {
        self.decompress_text_with_limit(DECOMPRESSION_LIMIT)
    }

    /// Decompresses the inner text, mutating its own state. Can only handle decompressed text up to `limit` bytes.
    pub fn decompress_text_with_limit(&mut self, limit: usize) -> Result<(), DecodingError> {
        match &self.text {
            OptCompressed::Compressed(v) => {
                let uncompressed_raw = match fdeflate::decompress_to_vec_bounded(&v[..], limit) {
                    Ok(s) => s,
                    Err(BoundedDecompressionError::OutputTooLarge { .. }) => {
                        return Err(DecodingError::from(
                            TextDecodingError::OutOfDecompressionSpace,
                        ));
                    }
                    Err(_) => {
                        return Err(DecodingError::from(TextDecodingError::InflationError));
                    }
                };
                self.text = OptCompressed::Uncompressed(decode_iso_8859_1(&uncompressed_raw));
            }
            OptCompressed::Uncompressed(_) => {}
        };
        Ok(())
    }

    /// Decompresses the inner text, and returns it as a `String`.
    /// If decompression uses more the 2MiB, first call decompress with limit, and then this method.
    pub fn get_text(&self) -> Result<String, DecodingError> {
        match &self.text {
            OptCompressed::Compressed(v) => {
                let uncompressed_raw = fdeflate::decompress_to_vec(v)
                    .map_err(|_| DecodingError::from(TextDecodingError::InflationError))?;
                Ok(decode_iso_8859_1(&uncompressed_raw))
            }
            OptCompressed::Uncompressed(s) => Ok(s.clone()),
        }
    }

    /// Compresses the inner text, mutating its own state.
    pub fn compress_text(&mut self) -> Result<(), EncodingError> {
        match &self.text {
            OptCompressed::Uncompressed(s) => {
                let uncompressed_raw = encode_iso_8859_1(s)?;
                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
                encoder
                    .write_all(&uncompressed_raw)
                    .map_err(|_| EncodingError::from(TextEncodingError::CompressionError))?;
                self.text = OptCompressed::Compressed(
                    encoder
                        .finish()
                        .map_err(|_| EncodingError::from(TextEncodingError::CompressionError))?,
                );
            }
            OptCompressed::Compressed(_) => {}
        }

        Ok(())
    }
}

impl EncodableTextChunk for ZTXtChunk {
    fn encode<W: Write>(&self, w: &mut W) -> Result<(), EncodingError> {
        let mut data = encode_iso_8859_1(&self.keyword)?;

        if data.is_empty() || data.len() > 79 {
            return Err(TextEncodingError::InvalidKeywordSize.into());
        }

        // Null separator
        data.push(0);

        // Compression method: the only valid value is 0, as of 2021.
        data.push(0);

        match &self.text {
            OptCompressed::Compressed(v) => {
                data.extend_from_slice(&v[..]);
            }
            OptCompressed::Uncompressed(s) => {
                // This code may have a bug. Check for correctness.
                let uncompressed_raw = encode_iso_8859_1(s)?;
                let mut encoder = ZlibEncoder::new(data, Compression::fast());
                encoder
                    .write_all(&uncompressed_raw)
                    .map_err(|_| EncodingError::from(TextEncodingError::CompressionError))?;
                data = encoder
                    .finish()
                    .map_err(|_| EncodingError::from(TextEncodingError::CompressionError))?;
            }
        };

        encoder::write_chunk(w, chunk::zTXt, &data)
    }
}

/// Struct encoding an iTXt chunk
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ITXtChunk {
    /// The keyword field. This needs to be between 1-79 bytes when encoded as Latin-1.
    pub keyword: String,
    /// Indicates whether the text will be (or was) compressed in the PNG.
    pub compressed: bool,
    /// A hyphen separated list of languages that the keyword is translated to. This is ASCII-7 encoded.
    pub language_tag: String,
    /// Translated keyword. This is UTF-8 encoded.
    pub translated_keyword: String,
    /// Text field of iTXt chunk. It is compressed by default, but can be uncompressed if necessary.
    text: OptCompressed,
}

impl ITXtChunk {
    /// Constructs a new iTXt chunk. Leaves all but keyword and text to default values.
    pub fn new(keyword: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            keyword: keyword.into(),
            compressed: false,
            language_tag: "".to_string(),
            translated_keyword: "".to_string(),
            text: OptCompressed::Uncompressed(text.into()),
        }
    }

    pub(crate) fn decode(
        keyword_slice: &[u8],
        compression_flag: u8,
        compression_method: u8,
        language_tag_slice: &[u8],
        translated_keyword_slice: &[u8],
        text_slice: &[u8],
    ) -> Result<Self, TextDecodingError> {
        if keyword_slice.is_empty() || keyword_slice.len() > 79 {
            return Err(TextDecodingError::InvalidKeywordSize);
        }
        let keyword = decode_iso_8859_1(keyword_slice);

        let compressed = match compression_flag {
            0 => false,
            1 => true,
            _ => return Err(TextDecodingError::InvalidCompressionFlag),
        };

        if compressed && compression_method != 0 {
            return Err(TextDecodingError::InvalidCompressionMethod);
        }

        let language_tag = decode_ascii(language_tag_slice)?.to_owned();

        let translated_keyword = std::str::from_utf8(translated_keyword_slice)
            .map_err(|_| TextDecodingError::Unrepresentable)?
            .to_string();
        let text = if compressed {
            OptCompressed::Compressed(text_slice.to_vec())
        } else {
            OptCompressed::Uncompressed(
                String::from_utf8(text_slice.to_vec())
                    .map_err(|_| TextDecodingError::Unrepresentable)?,
            )
        };

        Ok(Self {
            keyword,
            compressed,
            language_tag,
            translated_keyword,
            text,
        })
    }

    /// Decompresses the inner text, mutating its own state. Can only handle decompressed text up to `DECOMPRESSION_LIMIT` bytes.
    pub fn decompress_text(&mut self) -> Result<(), DecodingError> {
        self.decompress_text_with_limit(DECOMPRESSION_LIMIT)
    }

    /// Decompresses the inner text, mutating its own state. Can only handle decompressed text up to `limit` bytes.
    pub fn decompress_text_with_limit(&mut self, limit: usize) -> Result<(), DecodingError> {
        match &self.text {
            OptCompressed::Compressed(v) => {
                let uncompressed_raw = match fdeflate::decompress_to_vec_bounded(v, limit) {
                    Ok(s) => s,
                    Err(BoundedDecompressionError::OutputTooLarge { .. }) => {
                        return Err(DecodingError::from(
                            TextDecodingError::OutOfDecompressionSpace,
                        ));
                    }
                    Err(_) => {
                        return Err(DecodingError::from(TextDecodingError::InflationError));
                    }
                };
                self.text = OptCompressed::Uncompressed(
                    String::from_utf8(uncompressed_raw)
                        .map_err(|_| TextDecodingError::Unrepresentable)?,
                );
            }
            OptCompressed::Uncompressed(_) => {}
        };
        Ok(())
    }

    /// Decompresses the inner text, and returns it as a `String`.
    /// If decompression takes more than 2 MiB, try `decompress_text_with_limit` followed by this method.
    pub fn get_text(&self) -> Result<String, DecodingError> {
        match &self.text {
            OptCompressed::Compressed(v) => {
                let uncompressed_raw = fdeflate::decompress_to_vec(v)
                    .map_err(|_| DecodingError::from(TextDecodingError::InflationError))?;
                String::from_utf8(uncompressed_raw)
                    .map_err(|_| TextDecodingError::Unrepresentable.into())
            }
            OptCompressed::Uncompressed(s) => Ok(s.clone()),
        }
    }

    /// Compresses the inner text, mutating its own state.
    pub fn compress_text(&mut self) -> Result<(), EncodingError> {
        match &self.text {
            OptCompressed::Uncompressed(s) => {
                let uncompressed_raw = s.as_bytes();
                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
                encoder
                    .write_all(uncompressed_raw)
                    .map_err(|_| EncodingError::from(TextEncodingError::CompressionError))?;
                self.text = OptCompressed::Compressed(
                    encoder
                        .finish()
                        .map_err(|_| EncodingError::from(TextEncodingError::CompressionError))?,
                );
            }
            OptCompressed::Compressed(_) => {}
        }

        Ok(())
    }
}

impl EncodableTextChunk for ITXtChunk {
    fn encode<W: Write>(&self, w: &mut W) -> Result<(), EncodingError> {
        // Keyword
        let mut data = encode_iso_8859_1(&self.keyword)?;

        if data.is_empty() || data.len() > 79 {
            return Err(TextEncodingError::InvalidKeywordSize.into());
        }

        // Null separator
        data.push(0);

        // Compression flag
        if self.compressed {
            data.push(1);
        } else {
            data.push(0);
        }

        // Compression method
        data.push(0);

        // Language tag
        if !self.language_tag.is_ascii() {
            return Err(EncodingError::from(TextEncodingError::Unrepresentable));
        }
        data.extend(self.language_tag.as_bytes());

        // Null separator
        data.push(0);

        // Translated keyword
        data.extend_from_slice(self.translated_keyword.as_bytes());

        // Null separator
        data.push(0);

        // Text
        if self.compressed {
            match &self.text {
                OptCompressed::Compressed(v) => {
                    data.extend_from_slice(&v[..]);
                }
                OptCompressed::Uncompressed(s) => {
                    let uncompressed_raw = s.as_bytes();
                    let mut encoder = ZlibEncoder::new(data, Compression::fast());
                    encoder
                        .write_all(uncompressed_raw)
                        .map_err(|_| EncodingError::from(TextEncodingError::CompressionError))?;
                    data = encoder
                        .finish()
                        .map_err(|_| EncodingError::from(TextEncodingError::CompressionError))?;
                }
            }
        } else {
            match &self.text {
                OptCompressed::Compressed(v) => {
                    let uncompressed_raw = fdeflate::decompress_to_vec(v)
                        .map_err(|_| EncodingError::from(TextEncodingError::CompressionError))?;
                    data.extend_from_slice(&uncompressed_raw[..]);
                }
                OptCompressed::Uncompressed(s) => {
                    data.extend_from_slice(s.as_bytes());
                }
            }
        }

        encoder::write_chunk(w, chunk::iTXt, &data)
    }
}
