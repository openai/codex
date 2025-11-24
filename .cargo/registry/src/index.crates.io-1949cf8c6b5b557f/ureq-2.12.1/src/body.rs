use crate::stream::Stream;
use std::fmt;
use std::io::{self, copy, empty, Cursor, Read, Write};

#[cfg(feature = "charset")]
use crate::response::DEFAULT_CHARACTER_SET;
#[cfg(feature = "charset")]
use encoding_rs::Encoding;

/// The different kinds of bodies to send.
///
/// *Internal API*
pub(crate) enum Payload<'a> {
    Empty,
    Text(&'a str, String),
    Reader(Box<dyn Read + 'a>),
    Bytes(&'a [u8]),
}

impl fmt::Debug for Payload<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Payload::Empty => write!(f, "Empty"),
            Payload::Text(t, _) => write!(f, "{}", t),
            Payload::Reader(_) => write!(f, "Reader"),
            Payload::Bytes(v) => write!(f, "{:?}", v),
        }
    }
}

#[allow(clippy::derivable_impls)]
impl Default for Payload<'_> {
    fn default() -> Self {
        Payload::Empty
    }
}

/// The size of the body.
///
/// *Internal API*
#[derive(Debug)]
pub(crate) enum BodySize {
    Empty,
    Unknown,
    Known(u64),
}

/// Payloads are turned into this type where we can hold both a size and the reader.
///
/// *Internal API*
pub(crate) struct SizedReader<'a> {
    pub size: BodySize,
    pub reader: Box<dyn Read + 'a>,
}

impl fmt::Debug for SizedReader<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SizedReader[size={:?},reader]", self.size)
    }
}

impl<'a> SizedReader<'a> {
    fn new(size: BodySize, reader: Box<dyn Read + 'a>) -> Self {
        SizedReader { size, reader }
    }
}

impl<'a> Payload<'a> {
    pub fn into_read(self) -> SizedReader<'a> {
        match self {
            Payload::Empty => SizedReader::new(BodySize::Empty, Box::new(empty())),
            Payload::Text(text, _charset) => {
                #[cfg(feature = "charset")]
                let bytes = {
                    let encoding = Encoding::for_label(_charset.as_bytes())
                        .or_else(|| Encoding::for_label(DEFAULT_CHARACTER_SET.as_bytes()))
                        .unwrap();
                    encoding.encode(text).0
                };
                #[cfg(not(feature = "charset"))]
                let bytes = text.as_bytes();
                let len = bytes.len();
                let cursor = Cursor::new(bytes);
                SizedReader::new(BodySize::Known(len as u64), Box::new(cursor))
            }
            Payload::Reader(read) => SizedReader::new(BodySize::Unknown, read),
            Payload::Bytes(bytes) => {
                let len = bytes.len();
                let cursor = Cursor::new(bytes);
                SizedReader::new(BodySize::Known(len as u64), Box::new(cursor))
            }
        }
    }
}

const CHUNK_MAX_SIZE: usize = 0x4000; // Maximum size of a TLS fragment
const CHUNK_HEADER_MAX_SIZE: usize = 6; // four hex digits plus "\r\n"
const CHUNK_FOOTER_SIZE: usize = 2; // "\r\n"
const CHUNK_MAX_PAYLOAD_SIZE: usize = CHUNK_MAX_SIZE - CHUNK_HEADER_MAX_SIZE - CHUNK_FOOTER_SIZE;

// copy_chunks() improves over chunked_transfer's Encoder + io::copy with the
// following performance optimizations:
// 1) It avoid copying memory.
// 2) chunked_transfer's Encoder issues 4 separate write() per chunk. This is costly
//    overhead. Instead, we do a single write() per chunk.
// The measured benefit on a Linux machine is a 50% reduction in CPU usage on a https connection.
fn copy_chunked<R: Read, W: Write>(reader: &mut R, writer: &mut W) -> io::Result<u64> {
    // The chunk layout is:
    // header:header_max_size | payload:max_payload_size | footer:footer_size
    let mut chunk = Vec::with_capacity(CHUNK_MAX_SIZE);
    let mut written = 0;
    loop {
        // We first read the payload
        chunk.resize(CHUNK_HEADER_MAX_SIZE, 0);
        let payload_size = reader
            .take(CHUNK_MAX_PAYLOAD_SIZE as u64)
            .read_to_end(&mut chunk)?;

        // Then write the header
        let header_str = format!("{:x}\r\n", payload_size);
        let header = header_str.as_bytes();
        assert!(header.len() <= CHUNK_HEADER_MAX_SIZE);
        let start_index = CHUNK_HEADER_MAX_SIZE - header.len();
        (&mut chunk[start_index..]).write_all(header).unwrap();

        // And add the footer
        chunk.extend_from_slice(b"\r\n");

        // Finally Write the chunk
        writer.write_all(&chunk[start_index..])?;
        written += payload_size as u64;

        // On EOF, we wrote a 0 sized chunk. This is what the chunked encoding protocol requires.
        if payload_size == 0 {
            return Ok(written);
        }
    }
}

/// Helper to send a body, either as chunked or not.
pub(crate) fn send_body(
    mut body: SizedReader,
    do_chunk: bool,
    stream: &mut Stream,
) -> io::Result<()> {
    if do_chunk {
        copy_chunked(&mut body.reader, stream)?;
    } else {
        copy(&mut body.reader, stream)?;
    };

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copy_chunked() {
        let mut source = Vec::<u8>::new();
        source.resize(CHUNK_MAX_PAYLOAD_SIZE, 33);
        source.extend_from_slice(b"hello world");

        let mut dest = Vec::<u8>::new();
        copy_chunked(&mut &source[..], &mut dest).unwrap();

        let mut dest_expected = Vec::<u8>::new();
        dest_expected.extend_from_slice(format!("{:x}\r\n", CHUNK_MAX_PAYLOAD_SIZE).as_bytes());
        dest_expected.resize(dest_expected.len() + CHUNK_MAX_PAYLOAD_SIZE, 33);
        dest_expected.extend_from_slice(b"\r\n");

        dest_expected.extend_from_slice(b"b\r\nhello world\r\n");
        dest_expected.extend_from_slice(b"0\r\n\r\n");

        assert_eq!(dest, dest_expected);
    }
}
