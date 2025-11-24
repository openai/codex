//! Implementation for `miniz_oxide` rust backend.

use std::convert::TryInto;
use std::fmt;

use miniz_oxide::deflate::core::CompressorOxide;
use miniz_oxide::inflate::stream::InflateState;
pub use miniz_oxide::*;

pub const MZ_NO_FLUSH: isize = MZFlush::None as isize;
pub const MZ_PARTIAL_FLUSH: isize = MZFlush::Partial as isize;
pub const MZ_SYNC_FLUSH: isize = MZFlush::Sync as isize;
pub const MZ_FULL_FLUSH: isize = MZFlush::Full as isize;
pub const MZ_FINISH: isize = MZFlush::Finish as isize;

use super::*;
use crate::mem;

// miniz_oxide doesn't provide any error messages (yet?)
#[derive(Clone, Default)]
pub struct ErrorMessage;

impl ErrorMessage {
    pub fn get(&self) -> Option<&str> {
        None
    }
}

fn format_from_bool(zlib_header: bool) -> DataFormat {
    if zlib_header {
        DataFormat::Zlib
    } else {
        DataFormat::Raw
    }
}

pub struct Inflate {
    inner: Box<InflateState>,
    total_in: u64,
    total_out: u64,
}

impl fmt::Debug for Inflate {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "miniz_oxide inflate internal state. total_in: {}, total_out: {}",
            self.total_in, self.total_out,
        )
    }
}

impl From<FlushDecompress> for MZFlush {
    fn from(value: FlushDecompress) -> Self {
        match value {
            FlushDecompress::None => Self::None,
            FlushDecompress::Sync => Self::Sync,
            FlushDecompress::Finish => Self::Finish,
        }
    }
}

impl InflateBackend for Inflate {
    fn make(zlib_header: bool, _window_bits: u8) -> Self {
        let format = format_from_bool(zlib_header);

        Inflate {
            inner: InflateState::new_boxed(format),
            total_in: 0,
            total_out: 0,
        }
    }

    fn decompress(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        flush: FlushDecompress,
    ) -> Result<Status, DecompressError> {
        let mz_flush = flush.into();
        let res = inflate::stream::inflate(&mut self.inner, input, output, mz_flush);
        self.total_in += res.bytes_consumed as u64;
        self.total_out += res.bytes_written as u64;

        match res.status {
            Ok(status) => match status {
                MZStatus::Ok => Ok(Status::Ok),
                MZStatus::StreamEnd => Ok(Status::StreamEnd),
                MZStatus::NeedDict => {
                    mem::decompress_need_dict(self.inner.decompressor().adler32().unwrap_or(0))
                }
            },
            Err(status) => match status {
                MZError::Buf => Ok(Status::BufError),
                _ => mem::decompress_failed(ErrorMessage),
            },
        }
    }

    fn reset(&mut self, zlib_header: bool) {
        self.inner.reset(format_from_bool(zlib_header));
        self.total_in = 0;
        self.total_out = 0;
    }
}

impl Backend for Inflate {
    #[inline]
    fn total_in(&self) -> u64 {
        self.total_in
    }

    #[inline]
    fn total_out(&self) -> u64 {
        self.total_out
    }
}

pub struct Deflate {
    inner: Box<CompressorOxide>,
    total_in: u64,
    total_out: u64,
}

impl fmt::Debug for Deflate {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "miniz_oxide deflate internal state. total_in: {}, total_out: {}",
            self.total_in, self.total_out,
        )
    }
}

impl From<FlushCompress> for MZFlush {
    fn from(value: FlushCompress) -> Self {
        match value {
            FlushCompress::None => Self::None,
            FlushCompress::Partial | FlushCompress::Sync => Self::Sync,
            FlushCompress::Full => Self::Full,
            FlushCompress::Finish => Self::Finish,
        }
    }
}

impl DeflateBackend for Deflate {
    fn make(level: Compression, zlib_header: bool, _window_bits: u8) -> Self {
        // Check in case the integer value changes at some point.
        debug_assert!(level.level() <= 10);

        let mut inner: Box<CompressorOxide> = Box::default();
        let format = format_from_bool(zlib_header);
        inner.set_format_and_level(format, level.level().try_into().unwrap_or(1));

        Deflate {
            inner,
            total_in: 0,
            total_out: 0,
        }
    }

    fn compress(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        flush: FlushCompress,
    ) -> Result<Status, CompressError> {
        let mz_flush = flush.into();
        let res = deflate::stream::deflate(&mut self.inner, input, output, mz_flush);
        self.total_in += res.bytes_consumed as u64;
        self.total_out += res.bytes_written as u64;

        match res.status {
            Ok(status) => match status {
                MZStatus::Ok => Ok(Status::Ok),
                MZStatus::StreamEnd => Ok(Status::StreamEnd),
                MZStatus::NeedDict => mem::compress_failed(ErrorMessage),
            },
            Err(status) => match status {
                MZError::Buf => Ok(Status::BufError),
                _ => mem::compress_failed(ErrorMessage),
            },
        }
    }

    fn reset(&mut self) {
        self.total_in = 0;
        self.total_out = 0;
        self.inner.reset();
    }
}

impl Backend for Deflate {
    #[inline]
    fn total_in(&self) -> u64 {
        self.total_in
    }

    #[inline]
    fn total_out(&self) -> u64 {
        self.total_out
    }
}
