use alloc::{vec, vec::Vec};

/// The default buffer capacity that we use for the stream buffer.
const DEFAULT_BUFFER_CAPACITY: usize = 64 * (1 << 10); // 64 KB

/// A fairly simple roll buffer for supporting stream searches.
///
/// This buffer acts as a temporary place to store a fixed amount of data when
/// reading from a stream. Its central purpose is to allow "rolling" some
/// suffix of the data to the beginning of the buffer before refilling it with
/// more data from the stream. For example, let's say we are trying to match
/// "foobar" on a stream. When we report the match, we'd like to not only
/// report the correct offsets at which the match occurs, but also the matching
/// bytes themselves. So let's say our stream is a file with the following
/// contents: `test test foobar test test`. Now assume that we happen to read
/// the aforementioned file in two chunks: `test test foo` and `bar test test`.
/// Naively, it would not be possible to report a single contiguous `foobar`
/// match, but this roll buffer allows us to do that. Namely, after the second
/// read, the contents of the buffer should be `st foobar test test`, where the
/// search should ultimately resume immediately after `foo`. (The prefix `st `
/// is included because the roll buffer saves N bytes at the end of the buffer,
/// where N is the maximum possible length of a match.)
///
/// A lot of the logic for dealing with this is unfortunately split out between
/// this roll buffer and the `StreamChunkIter`.
///
/// Note also that this buffer is not actually required to just report matches.
/// Because a `Match` is just some offsets. But it *is* required for supporting
/// things like `try_stream_replace_all` because that needs some mechanism for
/// knowing which bytes in the stream correspond to a match and which don't. So
/// when a match occurs across two `read` calls, *something* needs to retain
/// the bytes from the previous `read` call because you don't know before the
/// second read call whether a match exists or not.
#[derive(Debug)]
pub(crate) struct Buffer {
    /// The raw buffer contents. This has a fixed size and never increases.
    buf: Vec<u8>,
    /// The minimum size of the buffer, which is equivalent to the maximum
    /// possible length of a match. This corresponds to the amount that we
    /// roll
    min: usize,
    /// The end of the contents of this buffer.
    end: usize,
}

impl Buffer {
    /// Create a new buffer for stream searching. The minimum buffer length
    /// given should be the size of the maximum possible match length.
    pub(crate) fn new(min_buffer_len: usize) -> Buffer {
        let min = core::cmp::max(1, min_buffer_len);
        // The minimum buffer amount is also the amount that we roll our
        // buffer in order to support incremental searching. To this end,
        // our actual capacity needs to be at least 1 byte bigger than our
        // minimum amount, otherwise we won't have any overlap. In actuality,
        // we want our buffer to be a bit bigger than that for performance
        // reasons, so we set a lower bound of `8 * min`.
        //
        // TODO: It would be good to find a way to test the streaming
        // implementation with the minimal buffer size. For now, we just
        // uncomment out the next line and comment out the subsequent line.
        // let capacity = 1 + min;
        let capacity = core::cmp::max(min * 8, DEFAULT_BUFFER_CAPACITY);
        Buffer { buf: vec![0; capacity], min, end: 0 }
    }

    /// Return the contents of this buffer.
    #[inline]
    pub(crate) fn buffer(&self) -> &[u8] {
        &self.buf[..self.end]
    }

    /// Return the minimum size of the buffer. The only way a buffer may be
    /// smaller than this is if the stream itself contains less than the
    /// minimum buffer amount.
    #[inline]
    pub(crate) fn min_buffer_len(&self) -> usize {
        self.min
    }

    /// Return all free capacity in this buffer.
    fn free_buffer(&mut self) -> &mut [u8] {
        &mut self.buf[self.end..]
    }

    /// Refill the contents of this buffer by reading as much as possible into
    /// this buffer's free capacity. If no more bytes could be read, then this
    /// returns false. Otherwise, this reads until it has filled the buffer
    /// past the minimum amount.
    pub(crate) fn fill<R: std::io::Read>(
        &mut self,
        mut rdr: R,
    ) -> std::io::Result<bool> {
        let mut readany = false;
        loop {
            let readlen = rdr.read(self.free_buffer())?;
            if readlen == 0 {
                return Ok(readany);
            }
            readany = true;
            self.end += readlen;
            if self.buffer().len() >= self.min {
                return Ok(true);
            }
        }
    }

    /// Roll the contents of the buffer so that the suffix of this buffer is
    /// moved to the front and all other contents are dropped. The size of the
    /// suffix corresponds precisely to the minimum buffer length.
    ///
    /// This should only be called when the entire contents of this buffer have
    /// been searched.
    pub(crate) fn roll(&mut self) {
        let roll_start = self
            .end
            .checked_sub(self.min)
            .expect("buffer capacity should be bigger than minimum amount");
        let roll_end = roll_start + self.min;

        assert!(roll_end <= self.end);
        self.buf.copy_within(roll_start..roll_end, 0);
        self.end = self.min;
    }
}
