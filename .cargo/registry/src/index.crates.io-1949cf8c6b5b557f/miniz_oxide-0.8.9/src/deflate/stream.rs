//! Extra streaming compression functionality.
//!
//! As of now this is mainly intended for use to build a higher-level wrapper.
//!
//! There is no DeflateState as the needed state is contained in the compressor struct itself.

use crate::deflate::core::{compress, CompressorOxide, TDEFLFlush, TDEFLStatus};
use crate::{MZError, MZFlush, MZStatus, StreamResult};

/// Try to compress from input to output with the given [`CompressorOxide`].
///
/// # Errors
///
/// Returns [`MZError::Buf`] If the size of the `output` slice is empty or no progress was made due
/// to lack of expected input data, or if called without [`MZFlush::Finish`] after the compression
/// was already finished.
///
/// Returns [`MZError::Param`] if the compressor parameters are set wrong.
///
/// Returns [`MZError::Stream`] when lower-level decompressor returns a
/// [`TDEFLStatus::PutBufFailed`]; may not actually be possible.
pub fn deflate(
    compressor: &mut CompressorOxide,
    input: &[u8],
    output: &mut [u8],
    flush: MZFlush,
) -> StreamResult {
    if output.is_empty() {
        return StreamResult::error(MZError::Buf);
    }

    if compressor.prev_return_status() == TDEFLStatus::Done {
        return if flush == MZFlush::Finish {
            StreamResult {
                bytes_written: 0,
                bytes_consumed: 0,
                status: Ok(MZStatus::StreamEnd),
            }
        } else {
            StreamResult::error(MZError::Buf)
        };
    }

    let mut bytes_written = 0;
    let mut bytes_consumed = 0;

    let mut next_in = input;
    let mut next_out = output;

    let status = loop {
        let in_bytes;
        let out_bytes;
        let defl_status = {
            let res = compress(compressor, next_in, next_out, TDEFLFlush::from(flush));
            in_bytes = res.1;
            out_bytes = res.2;
            res.0
        };

        next_in = &next_in[in_bytes..];
        next_out = &mut next_out[out_bytes..];
        bytes_consumed += in_bytes;
        bytes_written += out_bytes;

        // Check if we are done, or compression failed.
        match defl_status {
            TDEFLStatus::BadParam => break Err(MZError::Param),
            // Don't think this can happen as we're not using a custom callback.
            TDEFLStatus::PutBufFailed => break Err(MZError::Stream),
            TDEFLStatus::Done => break Ok(MZStatus::StreamEnd),
            _ => (),
        };

        // All the output space was used, so wait for more.
        if next_out.is_empty() {
            break Ok(MZStatus::Ok);
        }

        if next_in.is_empty() && (flush != MZFlush::Finish) {
            let total_changed = bytes_written > 0 || bytes_consumed > 0;

            break if (flush != MZFlush::None) || total_changed {
                // We wrote or consumed something, and/or did a flush (sync/partial etc.).
                Ok(MZStatus::Ok)
            } else {
                // No more input data, not flushing, and nothing was consumed or written,
                // so couldn't make any progress.
                Err(MZError::Buf)
            };
        }
    };
    StreamResult {
        bytes_consumed,
        bytes_written,
        status,
    }
}

#[cfg(test)]
mod test {
    use super::deflate;
    use crate::deflate::CompressorOxide;
    use crate::inflate::decompress_to_vec_zlib;
    use crate::{MZFlush, MZStatus};
    use alloc::boxed::Box;
    use alloc::vec;

    #[test]
    fn test_state() {
        let data = b"Hello zlib!";
        let mut compressed = vec![0; 50];
        let mut compressor = Box::<CompressorOxide>::default();
        let res = deflate(&mut compressor, data, &mut compressed, MZFlush::Finish);
        let status = res.status.expect("Failed to compress!");
        let decomp =
            decompress_to_vec_zlib(&compressed).expect("Failed to decompress compressed data");
        assert_eq!(status, MZStatus::StreamEnd);
        assert_eq!(decomp[..], data[..]);
        assert_eq!(res.bytes_consumed, data.len());
    }
}
