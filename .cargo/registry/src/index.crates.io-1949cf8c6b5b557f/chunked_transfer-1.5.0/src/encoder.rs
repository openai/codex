use std::io::Result as IoResult;
use std::io::Write;

/// Splits the incoming data into HTTP chunks.
///
/// # Example
///
/// ```
/// use chunked_transfer::Encoder;
/// use std::io::Write;
///
/// let mut decoded = "hello world";
/// let mut encoded: Vec<u8> = vec![];
///
/// {
///     let mut encoder = Encoder::with_chunks_size(&mut encoded, 5);
///     encoder.write_all(decoded.as_bytes());
/// }
///
/// assert_eq!(encoded, b"5\r\nhello\r\n5\r\n worl\r\n1\r\nd\r\n0\r\n\r\n");
/// ```
pub struct Encoder<W>
where
    W: Write,
{
    // where to send the result
    output: W,

    // size of each chunk
    chunks_size: usize,

    // data waiting to be sent is stored here
    // This will always be at least 6 bytes long. The first 6 bytes
    // are reserved for the chunk size and \r\n.
    buffer: Vec<u8>,

    // Flushes the internal buffer after each write. This might be useful
    // if data should be sent immediately to downstream consumers
    flush_after_write: bool,
}

const MAX_CHUNK_SIZE: usize = std::u32::MAX as usize;
// This accounts for four hex digits (enough to hold a u32) plus two bytes
// for the \r\n
const MAX_HEADER_SIZE: usize = 6;

impl<W> Encoder<W>
where
    W: Write,
{
    pub fn new(output: W) -> Encoder<W> {
        Encoder::with_chunks_size(output, 8192)
    }

    /// Gets a reference to the underlying value in this encoder.
    pub fn get_ref(&self) -> &W {
        &self.output
    }

    /// Gets a mutable reference to the underlying value in this encoder.
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.output
    }

    pub fn with_chunks_size(output: W, chunks: usize) -> Encoder<W> {
        let chunks_size = chunks.min(MAX_CHUNK_SIZE);
        let mut encoder = Encoder {
            output,
            chunks_size,
            buffer: vec![0; MAX_HEADER_SIZE],
            flush_after_write: false,
        };
        encoder.reset_buffer();
        encoder
    }

    pub fn with_flush_after_write(output: W) -> Encoder<W> {
        let mut encoder = Encoder {
            output,
            chunks_size: 8192,
            buffer: vec![0; MAX_HEADER_SIZE],
            flush_after_write: true,
        };
        encoder.reset_buffer();
        encoder
    }

    fn reset_buffer(&mut self) {
        // Reset buffer, still leaving space for the chunk size. That space
        // will be populated once we know the size of the chunk.
        self.buffer.truncate(MAX_HEADER_SIZE);
    }

    fn is_buffer_empty(&self) -> bool {
        self.buffer.len() == MAX_HEADER_SIZE
    }

    fn buffer_len(&self) -> usize {
        self.buffer.len() - MAX_HEADER_SIZE
    }

    fn send(&mut self) -> IoResult<()> {
        // Never send an empty buffer, because that would be interpreted
        // as the end of the stream, which we indicate explicitly on drop.
        if self.is_buffer_empty() {
            return Ok(());
        }
        // Prepend the length and \r\n to the buffer.
        let prelude = format!("{:x}\r\n", self.buffer_len());
        let prelude = prelude.as_bytes();

        // This should never happen because MAX_CHUNK_SIZE of u32::MAX
        // can always be encoded in 4 hex bytes.
        assert!(
            prelude.len() <= MAX_HEADER_SIZE,
            "invariant failed: prelude longer than MAX_HEADER_SIZE"
        );

        // Copy the prelude into the buffer. For small chunks, this won't necessarily
        // take up all the space that was reserved for the prelude.
        let offset = MAX_HEADER_SIZE - prelude.len();
        self.buffer[offset..MAX_HEADER_SIZE].clone_from_slice(prelude);

        // Append the chunk-finishing \r\n to the buffer.
        self.buffer.write_all(b"\r\n")?;

        self.output.write_all(&self.buffer[offset..])?;
        self.reset_buffer();

        Ok(())
    }
}

impl<W> Write for Encoder<W>
where
    W: Write,
{
    fn write(&mut self, data: &[u8]) -> IoResult<usize> {
        let remaining_buffer_space = self.chunks_size - self.buffer_len();
        let bytes_to_buffer = std::cmp::min(remaining_buffer_space, data.len());
        self.buffer.extend_from_slice(&data[0..bytes_to_buffer]);
        let more_to_write: bool = bytes_to_buffer < data.len();
        if self.flush_after_write || more_to_write {
            self.send()?;
        }

        // If we didn't write the whole thing, keep working on it.
        if more_to_write {
            self.write_all(&data[bytes_to_buffer..])?;
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        self.send()
    }
}

impl<W> Drop for Encoder<W>
where
    W: Write,
{
    fn drop(&mut self) {
        self.flush().ok();
        write!(self.output, "0\r\n\r\n").ok();
    }
}

#[cfg(test)]
mod test {
    use super::Encoder;
    use std::io;
    use std::io::Write;
    use std::str::from_utf8;

    #[test]
    fn test() {
        let mut source = io::Cursor::new("hello world".to_string().into_bytes());
        let mut dest: Vec<u8> = vec![];

        {
            let mut encoder = Encoder::with_chunks_size(dest.by_ref(), 5);
            io::copy(&mut source, &mut encoder).unwrap();
            assert!(!encoder.is_buffer_empty());
        }

        let output = from_utf8(&dest).unwrap();

        assert_eq!(output, "5\r\nhello\r\n5\r\n worl\r\n1\r\nd\r\n0\r\n\r\n");
    }
    #[test]
    fn flush_after_write() {
        let mut source = io::Cursor::new("hello world".to_string().into_bytes());
        let mut dest: Vec<u8> = vec![];

        {
            let mut encoder = Encoder::with_flush_after_write(dest.by_ref());
            io::copy(&mut source, &mut encoder).unwrap();
            // The internal buffer has been flushed.
            assert!(encoder.is_buffer_empty());
        }

        let output = from_utf8(&dest).unwrap();

        assert_eq!(output, "b\r\nhello world\r\n0\r\n\r\n");
    }
}
