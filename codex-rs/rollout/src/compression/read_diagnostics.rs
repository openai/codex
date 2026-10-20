//! Captures byte counts and file changes on the first compressed-stream failure.
//! Healthy reads only retain the first frame header and count decoded bytes. Logs contain
//! no paths or contents and are limited to one failure per process per minute.

use std::fs::File;
use std::fs::Metadata;
use std::io;
use std::io::BufReader;
use std::io::Read;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::time::Duration;
use std::time::Instant;

use super::error_metrics::error_kind;

pub(super) struct DiagnosticDecoder {
    decoder: zstd::stream::read::Decoder<'static, BufReader<Source>>,
    decoded_bytes: u64,
    failure: Option<FailureDetails>,
}

#[derive(Debug, PartialEq, Eq)]
struct FailureDetails {
    compressed_bytes: Option<u64>,
    first_frame_content_bytes: Option<u64>,
    decoded_bytes: u64,
    file_changed: Option<bool>,
}

struct Source {
    file: File,
    opened_metadata: Option<Metadata>,
    header: [u8; 18],
    header_len: usize,
}

impl DiagnosticDecoder {
    pub(super) fn new(file: File) -> io::Result<Self> {
        let source = Source {
            opened_metadata: file.metadata().ok(),
            file,
            header: [0; 18],
            header_len: 0,
        };
        Ok(Self {
            decoder: zstd::stream::read::Decoder::new(source)?,
            decoded_bytes: 0,
            failure: None,
        })
    }
}

impl Read for DiagnosticDecoder {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let result = self.decoder.read(buffer);
        match &result {
            Ok(bytes) => self.decoded_bytes = self.decoded_bytes.saturating_add(*bytes as u64),
            Err(error) if self.failure.is_none() => {
                let source = self.decoder.get_ref().get_ref();
                let metadata = source.file.metadata().ok();
                let details = FailureDetails {
                    compressed_bytes: metadata.as_ref().map(Metadata::len),
                    // This is the first frame's size, not the total for concatenated frames.
                    first_frame_content_bytes: zstd::zstd_safe::get_frame_content_size(
                        &source.header[..source.header_len],
                    )
                    .ok()
                    .flatten(),
                    decoded_bytes: self.decoded_bytes,
                    file_changed: source
                        .opened_metadata
                        .as_ref()
                        .zip(metadata.as_ref())
                        .and_then(|(before, after)| {
                            Some(
                                before.len() != after.len()
                                    || before.modified().ok()? != after.modified().ok()?,
                            )
                        }),
                };
                static LAST_REPORT: Mutex<Option<Instant>> = Mutex::new(None);
                let report = {
                    let mut last = LAST_REPORT.lock().unwrap_or_else(PoisonError::into_inner);
                    if last
                        .is_none_or(|time| time.elapsed() >= Duration::from_secs(/*secs*/ 60))
                    {
                        *last = Some(Instant::now());
                        true
                    } else {
                        false
                    }
                };
                if report {
                    tracing::warn!(
                        error_kind = error_kind(error),
                        compressed_bytes = ?details.compressed_bytes,
                        first_frame_content_bytes = ?details.first_frame_content_bytes,
                        decoded_bytes = details.decoded_bytes,
                        file_changed = ?details.file_changed,
                        "compressed rollout read failed"
                    );
                }
                self.failure = Some(details);
            }
            Err(_) => {}
        }
        result
    }
}

impl Read for Source {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let bytes = self.file.read(buffer)?;
        let header_bytes = bytes.min(self.header.len() - self.header_len);
        self.header[self.header_len..self.header_len + header_bytes]
            .copy_from_slice(&buffer[..header_bytes]);
        self.header_len += header_bytes;
        Ok(bytes)
    }
}

#[cfg(test)]
#[path = "read_diagnostics_tests.rs"]
mod tests;
