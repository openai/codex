use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::{cmp, mem};
#[cfg(feature = "std")]
use std::io;
#[cfg(feature = "std")]
use std::io::Read;

#[cfg(feature = "std")]
use crate::msgs::message::OutboundChunks;

/// This is a byte buffer that is built from a deque of byte vectors.
///
/// This avoids extra copies when appending a new byte vector,
/// at the expense of more complexity when reading out.
pub(crate) struct ChunkVecBuffer {
    /// How many bytes have been consumed in the first chunk.
    ///
    /// Invariant: zero if `chunks.is_empty()`
    /// Invariant: 0 <= `prefix_used` < `chunks[0].len()`
    prefix_used: usize,

    chunks: VecDeque<Vec<u8>>,

    /// The total upper limit (in bytes) of this object.
    limit: Option<usize>,
}

impl ChunkVecBuffer {
    pub(crate) fn new(limit: Option<usize>) -> Self {
        Self {
            prefix_used: 0,
            chunks: VecDeque::new(),
            limit,
        }
    }

    /// Sets the upper limit on how many bytes this
    /// object can store.
    ///
    /// Setting a lower limit than the currently stored
    /// data is not an error.
    ///
    /// A [`None`] limit is interpreted as no limit.
    pub(crate) fn set_limit(&mut self, new_limit: Option<usize>) {
        self.limit = new_limit;
    }

    /// If we're empty
    pub(crate) fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// How many bytes we're storing
    pub(crate) fn len(&self) -> usize {
        self.chunks
            .iter()
            .fold(0usize, |acc, chunk| acc + chunk.len())
            - self.prefix_used
    }

    /// For a proposed append of `len` bytes, how many
    /// bytes should we actually append to adhere to the
    /// currently set `limit`?
    pub(crate) fn apply_limit(&self, len: usize) -> usize {
        if let Some(limit) = self.limit {
            let space = limit.saturating_sub(self.len());
            cmp::min(len, space)
        } else {
            len
        }
    }

    /// Take and append the given `bytes`.
    pub(crate) fn append(&mut self, bytes: Vec<u8>) -> usize {
        let len = bytes.len();

        if !bytes.is_empty() {
            if self.chunks.is_empty() {
                debug_assert_eq!(self.prefix_used, 0);
            }

            self.chunks.push_back(bytes);
        }

        len
    }

    /// Take one of the chunks from this object.
    ///
    /// This function returns `None` if the object `is_empty`.
    pub(crate) fn pop(&mut self) -> Option<Vec<u8>> {
        let mut first = self.chunks.pop_front();

        if let Some(first) = &mut first {
            // slice off `prefix_used` if needed (uncommon)
            let prefix = mem::take(&mut self.prefix_used);
            first.drain(0..prefix);
        }

        first
    }

    #[cfg(read_buf)]
    /// Read data out of this object, writing it into `cursor`.
    pub(crate) fn read_buf(&mut self, mut cursor: core::io::BorrowedCursor<'_>) -> io::Result<()> {
        while !self.is_empty() && cursor.capacity() > 0 {
            let chunk = &self.chunks[0][self.prefix_used..];
            let used = cmp::min(chunk.len(), cursor.capacity());
            cursor.append(&chunk[..used]);
            self.consume(used);
        }

        Ok(())
    }

    /// Inspect the first chunk from this object.
    pub(crate) fn peek(&self) -> Option<&[u8]> {
        self.chunks
            .front()
            .map(|ch| ch.as_slice())
    }
}

#[cfg(feature = "std")]
impl ChunkVecBuffer {
    pub(crate) fn is_full(&self) -> bool {
        self.limit
            .map(|limit| self.len() > limit)
            .unwrap_or_default()
    }

    /// Append a copy of `bytes`, perhaps a prefix if
    /// we're near the limit.
    pub(crate) fn append_limited_copy(&mut self, payload: OutboundChunks<'_>) -> usize {
        let take = self.apply_limit(payload.len());
        self.append(payload.split_at(take).0.to_vec());
        take
    }

    /// Read data out of this object, writing it into `buf`
    /// and returning how many bytes were written there.
    pub(crate) fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut offs = 0;

        while offs < buf.len() && !self.is_empty() {
            let used = (&self.chunks[0][self.prefix_used..]).read(&mut buf[offs..])?;

            self.consume(used);
            offs += used;
        }

        Ok(offs)
    }

    pub(crate) fn consume_first_chunk(&mut self, used: usize) {
        // this backs (infallible) `BufRead::consume`, where `used` is
        // user-supplied.
        assert!(
            used <= self
                .chunk()
                .map(|ch| ch.len())
                .unwrap_or_default(),
            "illegal `BufRead::consume` usage",
        );
        self.consume(used);
    }

    fn consume(&mut self, used: usize) {
        // first, mark the rightmost extent of the used buffer
        self.prefix_used += used;

        // then reduce `prefix_used` by discarding wholly-covered
        // buffers
        while let Some(buf) = self.chunks.front() {
            if self.prefix_used < buf.len() {
                return;
            } else {
                self.prefix_used -= buf.len();
                self.chunks.pop_front();
            }
        }

        debug_assert_eq!(
            self.prefix_used, 0,
            "attempted to `ChunkVecBuffer::consume` more than available"
        );
    }

    /// Read data out of this object, passing it `wr`
    pub(crate) fn write_to(&mut self, wr: &mut dyn io::Write) -> io::Result<usize> {
        if self.is_empty() {
            return Ok(0);
        }

        let mut prefix = self.prefix_used;
        let mut bufs = [io::IoSlice::new(&[]); 64];
        for (iov, chunk) in bufs.iter_mut().zip(self.chunks.iter()) {
            *iov = io::IoSlice::new(&chunk[prefix..]);
            prefix = 0;
        }
        let len = cmp::min(bufs.len(), self.chunks.len());
        let bufs = &bufs[..len];
        let used = wr.write_vectored(bufs)?;
        let available_bytes = bufs.iter().map(|ch| ch.len()).sum();

        if used > available_bytes {
            // This is really unrecoverable, since the amount of data written
            // is now unknown.  Consume all the potentially-written data in
            // case the caller ignores the error.
            // See <https://github.com/rustls/rustls/issues/2316> for background.
            self.consume(available_bytes);
            return Err(io::Error::new(
                io::ErrorKind::Other,
                std::format!("illegal write_vectored return value ({used} > {available_bytes})"),
            ));
        }
        self.consume(used);
        Ok(used)
    }

    /// Returns the first contiguous chunk of data, or None if empty.
    pub(crate) fn chunk(&self) -> Option<&[u8]> {
        self.chunks
            .front()
            .map(|chunk| &chunk[self.prefix_used..])
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use alloc::vec;
    use alloc::vec::Vec;

    use super::ChunkVecBuffer;

    #[test]
    fn short_append_copy_with_limit() {
        let mut cvb = ChunkVecBuffer::new(Some(12));
        assert_eq!(cvb.append_limited_copy(b"hello"[..].into()), 5);
        assert_eq!(cvb.append_limited_copy(b"world"[..].into()), 5);
        assert_eq!(cvb.append_limited_copy(b"hello"[..].into()), 2);
        assert_eq!(cvb.append_limited_copy(b"world"[..].into()), 0);

        let mut buf = [0u8; 12];
        assert_eq!(cvb.read(&mut buf).unwrap(), 12);
        assert_eq!(buf.to_vec(), b"helloworldhe".to_vec());
    }

    #[test]
    fn read_byte_by_byte() {
        let mut cvb = ChunkVecBuffer::new(None);
        cvb.append(b"test fixture data".to_vec());
        assert!(!cvb.is_empty());
        for expect in b"test fixture data" {
            let mut byte = [0];
            assert_eq!(cvb.read(&mut byte).unwrap(), 1);
            assert_eq!(byte[0], *expect);
        }

        assert_eq!(cvb.read(&mut [0]).unwrap(), 0);
    }

    #[test]
    fn every_possible_chunk_interleaving() {
        let input = (0..=0xffu8)
            .cycle()
            .take(4096)
            .collect::<Vec<u8>>();

        for input_chunk_len in 1..64usize {
            for output_chunk_len in 1..65usize {
                std::println!("check input={input_chunk_len} output={output_chunk_len}");
                let mut cvb = ChunkVecBuffer::new(None);
                for chunk in input.chunks(input_chunk_len) {
                    cvb.append(chunk.to_vec());
                }

                assert_eq!(cvb.len(), input.len());
                let mut buf = vec![0u8; output_chunk_len];

                for expect in input.chunks(output_chunk_len) {
                    assert_eq!(expect.len(), cvb.read(&mut buf).unwrap());
                    assert_eq!(expect, &buf[..expect.len()]);
                }

                assert_eq!(cvb.read(&mut [0]).unwrap(), 0);
            }
        }
    }

    #[cfg(read_buf)]
    #[test]
    fn read_buf() {
        use core::io::BorrowedBuf;
        use core::mem::MaybeUninit;

        {
            let mut cvb = ChunkVecBuffer::new(None);
            cvb.append(b"test ".to_vec());
            cvb.append(b"fixture ".to_vec());
            cvb.append(b"data".to_vec());

            let mut buf = [MaybeUninit::<u8>::uninit(); 8];
            let mut buf: BorrowedBuf<'_> = buf.as_mut_slice().into();
            cvb.read_buf(buf.unfilled()).unwrap();
            assert_eq!(buf.filled(), b"test fix");
            buf.clear();
            cvb.read_buf(buf.unfilled()).unwrap();
            assert_eq!(buf.filled(), b"ture dat");
            buf.clear();
            cvb.read_buf(buf.unfilled()).unwrap();
            assert_eq!(buf.filled(), b"a");
        }

        {
            let mut cvb = ChunkVecBuffer::new(None);
            cvb.append(b"short message".to_vec());

            let mut buf = [MaybeUninit::<u8>::uninit(); 1024];
            let mut buf: BorrowedBuf<'_> = buf.as_mut_slice().into();
            cvb.read_buf(buf.unfilled()).unwrap();
            assert_eq!(buf.filled(), b"short message");
        }
    }
}

#[cfg(bench)]
mod benchmarks {
    use alloc::vec;

    use super::ChunkVecBuffer;

    #[bench]
    fn read_one_byte_from_large_message(b: &mut test::Bencher) {
        b.iter(|| {
            let mut cvb = ChunkVecBuffer::new(None);
            cvb.append(vec![0u8; 16_384]);
            assert_eq!(1, cvb.read(&mut [0u8]).unwrap());
        });
    }

    #[bench]
    fn read_all_individual_from_large_message(b: &mut test::Bencher) {
        b.iter(|| {
            let mut cvb = ChunkVecBuffer::new(None);
            cvb.append(vec![0u8; 16_384]);
            loop {
                if let Ok(0) = cvb.read(&mut [0u8]) {
                    break;
                }
            }
        });
    }

    #[bench]
    fn read_half_bytes_from_large_message(b: &mut test::Bencher) {
        b.iter(|| {
            let mut cvb = ChunkVecBuffer::new(None);
            cvb.append(vec![0u8; 16_384]);
            assert_eq!(8192, cvb.read(&mut [0u8; 8192]).unwrap());
            assert_eq!(8192, cvb.read(&mut [0u8; 8192]).unwrap());
        });
    }

    #[bench]
    fn read_entire_large_message(b: &mut test::Bencher) {
        b.iter(|| {
            let mut cvb = ChunkVecBuffer::new(None);
            cvb.append(vec![0u8; 16_384]);
            assert_eq!(16_384, cvb.read(&mut [0u8; 16_384]).unwrap());
        });
    }
}
