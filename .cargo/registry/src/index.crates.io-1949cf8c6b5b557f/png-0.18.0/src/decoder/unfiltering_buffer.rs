use super::stream::{DecodingError, FormatErrorInner};
use super::zlib::UnfilterBuf;
use crate::common::BytesPerPixel;
use crate::filter::{unfilter, RowFilter};
use crate::Info;

// Buffer for temporarily holding decompressed, not-yet-`unfilter`-ed rows.
pub(crate) struct UnfilteringBuffer {
    /// Vec containing the uncompressed image data currently being processed.
    data_stream: Vec<u8>,
    /// Index in `data_stream` where the previous row starts.
    /// This excludes the filter type byte - it points at the first byte of actual pixel data.
    /// The pixel data is already-`unfilter`-ed.
    ///
    /// If `prev_start == current_start` then it means that there is no previous row.
    prev_start: usize,
    /// Index in `data_stream` where the current row starts.
    /// This points at the filter type byte of the current row (i.e. the actual pixel data starts at `current_start + 1`)
    /// The pixel data is not-yet-`unfilter`-ed.
    ///
    /// `current_start` can wrap around the length.
    current_start: usize,
    /// Logical length of data that must be preserved.
    filled: usize,
    /// Length of data that can be modified.
    available: usize,
    /// The number of bytes before we shift the buffer back.
    shift_back_limit: usize,
}

impl UnfilteringBuffer {
    pub const GROWTH_BYTES: usize = 8 * 1024;

    /// Asserts in debug builds that all the invariants hold.  No-op in release
    /// builds.  Intended to be called after creating or mutating `self` to
    /// ensure that the final state preserves the invariants.
    fn debug_assert_invariants(&self) {
        debug_assert!(self.prev_start <= self.current_start);
        debug_assert!(self.current_start <= self.available);
        debug_assert!(self.available <= self.filled);
        debug_assert!(self.filled <= self.data_stream.len());
    }

    /// Create a buffer tuned for filtering rows of the image type.
    pub fn new(info: &Info<'_>) -> Self {
        // We don't need all of `info` here so if that becomes a structural problem then these
        // derived constants can be extracted into a parameter struct. For instance they may be
        // adjusted according to platform hardware such as cache sizes.
        let data_stream_capacity = {
            let max_data = info
                .checked_raw_row_length()
                // In the current state this is really dependent on IDAT sizes and the compression
                // settings. We aim to avoid overallocation here, but that occurs in part due to
                // the algorithm for draining the buffer, which at the time of writing is at each
                // individual IDAT chunk boundary. So this is set for a quadratic image roughly
                // fitting into a single 4k chunk at compression.. A very arbitrary choice made
                // from (probably overfitting) a benchmark of that image size. With a different
                // algorithm we may come to different buffer uses and have to re-evaluate.
                .and_then(|v| v.checked_mul(info.height.min(128) as usize))
                // In the worst case this is additional room for use of unmasked SIMD moves. But
                // the other idea here is that the allocator generally aligns the buffer.
                .and_then(|v| checked_next_multiple_of(v, 256))
                .unwrap_or(usize::MAX);
            // We do not want to pre-allocate too much in case of a faulty image (no DOS by
            // pretending to be very very large) and also we want to avoid allocating more data
            // than we need for the image itself.
            max_data.min(128 * 1024)
        };

        let shift_back_limit = {
            // Prefer shifting by powers of two and only after having done some number of
            // lines that then become free at the end of the buffer.
            let rowlen_pot = info
                .checked_raw_row_length()
                // Ensure some number of rows are actually present before shifting back, i.e. next
                // time around we want to be able to decode them without reallocating the buffer.
                .and_then(|v| v.checked_mul(4))
                // And also, we should be able to use aligned memcopy on the whole thing. Well at
                // least that is the idea but the parameter is just benchmarking. Higher numbers
                // did not result in performance gains but lowers also, so this is fickle. Maybe
                // our shift back behavior can not be tuned very well.
                .and_then(|v| checked_next_multiple_of(v, 64))
                .unwrap_or(isize::MAX as usize);
            // But never shift back before we have a number of pages freed.
            rowlen_pot.max(128 * 1024)
        };

        let result = Self {
            data_stream: Vec::with_capacity(data_stream_capacity),
            prev_start: 0,
            current_start: 0,
            filled: 0,
            available: 0,
            shift_back_limit,
        };

        result.debug_assert_invariants();
        result
    }

    /// Called to indicate that there is no previous row (e.g. when the current
    /// row is the first scanline of a given Adam7 pass).
    pub fn reset_prev_row(&mut self) {
        self.prev_start = self.current_start;
        self.debug_assert_invariants();
    }

    pub fn reset_all(&mut self) {
        self.data_stream.clear();
        self.prev_start = 0;
        self.current_start = 0;
        self.filled = 0;
        self.available = 0;
    }

    /// Returns the previous (already `unfilter`-ed) row.
    pub fn prev_row(&self) -> &[u8] {
        &self.data_stream[self.prev_start..self.current_start]
    }

    /// Returns how many bytes of the current row are present in the buffer.
    pub fn curr_row_len(&self) -> usize {
        self.available - self.current_start
    }

    /// Returns a `&mut Vec<u8>` suitable for passing to
    /// `ReadDecoder.decode_image_data` or `StreamingDecoder.update`.
    ///
    /// Invariants of `self` depend on the assumption that the caller will only
    /// append new bytes to the returned vector (which is indeed the behavior of
    /// `ReadDecoder` and `StreamingDecoder`).  TODO: Consider protecting the
    /// invariants by returning an append-only view of the vector
    /// (`FnMut(&[u8])`??? or maybe `std::io::Write`???).
    pub fn as_unfilled_buffer(&mut self) -> UnfilterBuf<'_> {
        if self.prev_start >= self.shift_back_limit
            // Avoid the shift back if the buffer is still very empty. Consider how we got here: a
            // previous decompression filled the buffer, then we unfiltered, we're now refilling
            // the buffer again. The condition implies, the previous decompression filled at most
            // half the buffer. Likely the same will happen again so the following decompression
            // attempt will not yet be limited by the buffer length.
            && self.filled >= self.data_stream.len() / 2
        {
            // We have to relocate the data to the start of the buffer. Benchmarking suggests that
            // the codegen for an unbounded range is better / different than the one for a bounded
            // range. We prefer the former if the data overhead is not too high. `16` was
            // determined experimentally and might be system (memory) dependent. There's also the
            // question if we could be a little smarter and avoid crossing page boundaries when
            // that is not required. Alas, microbenchmarking TBD.
            if let Some(16..) = self.data_stream.len().checked_sub(self.filled) {
                self.data_stream
                    .copy_within(self.prev_start..self.filled, 0);
            } else {
                self.data_stream.copy_within(self.prev_start.., 0);
            }

            // The data kept its relative position to `filled` which now lands exactly at
            // the distance between prev_start and filled.
            self.current_start -= self.prev_start;
            self.available -= self.prev_start;
            self.filled -= self.prev_start;
            self.prev_start = 0;
        }

        if self.filled + Self::GROWTH_BYTES > self.data_stream.len() {
            self.data_stream.resize(self.filled + Self::GROWTH_BYTES, 0);
        }

        UnfilterBuf {
            buffer: &mut self.data_stream,
            filled: &mut self.filled,
            available: &mut self.available,
        }
    }

    /// Runs `unfilter` on the current row, and then shifts rows so that the current row becomes the previous row.
    ///
    /// Will panic if `self.curr_row_len() < rowlen`.
    pub fn unfilter_curr_row(
        &mut self,
        rowlen: usize,
        bpp: BytesPerPixel,
    ) -> Result<(), DecodingError> {
        debug_assert!(rowlen >= 2); // 1 byte for `FilterType` and at least 1 byte of pixel data.

        let (prev, row) = self.data_stream.split_at_mut(self.current_start);
        let prev: &[u8] = &prev[self.prev_start..];

        debug_assert!(prev.is_empty() || prev.len() == (rowlen - 1));

        // Get the filter type.
        let filter = RowFilter::from_u8(row[0]).ok_or(DecodingError::Format(
            FormatErrorInner::UnknownFilterMethod(row[0]).into(),
        ))?;
        let row = &mut row[1..rowlen];

        unfilter(filter, bpp, prev, row);

        self.prev_start = self.current_start + 1;
        self.current_start += rowlen;

        self.debug_assert_invariants();

        Ok(())
    }
}

fn checked_next_multiple_of(val: usize, factor: usize) -> Option<usize> {
    if factor == 0 {
        return None;
    }

    let remainder = val % factor;

    if remainder > 0 {
        val.checked_add(factor - remainder)
    } else {
        Some(val)
    }
}

#[test]
fn next_multiple_of_backport_testsuite() {
    assert_eq!(checked_next_multiple_of(1, 0), None);
    assert_eq!(checked_next_multiple_of(2, 0), None);
    assert_eq!(checked_next_multiple_of(1, 2), Some(2));
    assert_eq!(checked_next_multiple_of(2, 2), Some(2));
    assert_eq!(checked_next_multiple_of(2, 5), Some(5));
    assert_eq!(checked_next_multiple_of(1, usize::MAX), Some(usize::MAX));
    assert_eq!(checked_next_multiple_of(usize::MAX, 2), None);
}
