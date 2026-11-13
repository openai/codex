//! Whole JSON bodies in ordered Zstandard shards. Each record is a little-endian
//! u16 ID length and u64 body length, followed by the UTF-8 ID and the body bytes.
//! A flush publishes one record without resetting the compression window.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::ops::Range;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use serde::de::IgnoredAny;
use zstd::stream::write::Encoder;

use crate::bundle::PAYLOADS_DIR_NAME;
use crate::payload::PayloadCapture;
use crate::payload::RawPayloadId;
use crate::payload::RawPayloadRef;

const TARGET_BYTES: usize = 32 * 1024 * 1024;
const WINDOW_LOG: u32 = 23; // 8 MiB of match history across payloads.
const HEADER_BYTES: usize = 10;

fn shard_path(directory: &Path, number: u64) -> PathBuf {
    directory.join(format!("{number:06}.zst"))
}

pub(crate) struct ShardWriter {
    directory: PathBuf,
    active: Option<Encoder<'static, BufWriter<File>>>,
    number: u64,
    bytes: usize,
}

impl ShardWriter {
    pub(crate) fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            active: None,
            number: 0,
            bytes: 0,
        }
    }

    /// Returns the shard containing this whole record once it is readable.
    pub(crate) fn append(&mut self, id: &str, body: &[u8]) -> Result<u64> {
        let id_len = u16::try_from(id.len())?;
        let size = HEADER_BYTES + id.len() + body.len();
        if self.bytes.saturating_add(size) > TARGET_BYTES {
            self.finish()?;
        }
        let encoder = match self.active.as_mut() {
            Some(active) => active,
            None => {
                self.number += 1;
                self.bytes = 0;
                let path = shard_path(&self.directory, self.number);
                let file = File::create_new(&path)
                    .with_context(|| format!("create shard {}", path.display()))?;
                let mut encoder = Encoder::new(BufWriter::new(file), 3)?;
                encoder.window_log(WINDOW_LOG)?;
                encoder.include_checksum(true)?;
                self.active.insert(encoder)
            }
        };
        encoder.write_all(&id_len.to_le_bytes())?;
        encoder.write_all(&(body.len() as u64).to_le_bytes())?;
        encoder.write_all(id.as_bytes())?;
        encoder.write_all(body)?;
        encoder.flush()?;
        self.bytes += size;
        Ok(self.number)
    }

    /// Flushes the frame footer before the event log can publish RecordingCompleted.
    pub(crate) fn finish(&mut self) -> Result<()> {
        if let Some(active) = self.active.take() {
            active.finish()?.flush()?;
        }
        Ok(())
    }
}

struct DecodedShard {
    number: u64,
    sealed: bool,
    bytes: Box<[u8]>,
    records: HashMap<String, Range<usize>>,
}

impl DecodedShard {
    fn open(directory: &Path, number: u64) -> Result<Self> {
        let path = shard_path(directory, number);
        let file = File::open(&path).with_context(|| format!("open shard {}", path.display()))?;
        let mut decoder = zstd::stream::read::Decoder::new(file)?;
        decoder.window_log_max(WINDOW_LOG)?;
        let mut bytes = Vec::new();
        let sealed = match decoder.read_to_end(&mut bytes) {
            Ok(_) => true,
            // A live writer may have flushed complete records without sealing the frame.
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => false,
            Err(error) => {
                return Err(error).with_context(|| format!("decompress shard {}", path.display()));
            }
        };

        let mut records = HashMap::new();
        let mut offset = 0;
        while bytes.len() - offset >= HEADER_BYTES {
            let header = &bytes[offset..offset + HEADER_BYTES];
            let id_len = usize::from(u16::from_le_bytes(header[..2].try_into()?));
            let body_len = usize::try_from(u64::from_le_bytes(header[2..].try_into()?))?;
            let id_start = offset + HEADER_BYTES;
            let Some(body_start) = id_start.checked_add(id_len) else {
                break;
            };
            let Some(end) = body_start.checked_add(body_len) else {
                break;
            };
            if end > bytes.len() {
                break;
            }
            let id = std::str::from_utf8(&bytes[id_start..body_start])?;
            ensure!(
                records.insert(id.to_owned(), body_start..end).is_none(),
                "duplicate payload {id} in shard {number}"
            );
            offset = end;
        }
        ensure!(
            !sealed || offset == bytes.len(),
            "partial record in sealed shard {number}"
        );
        Ok(Self {
            number,
            sealed,
            bytes: bytes.into_boxed_slice(),
            records,
        })
    }
}

/// Keeps the two most recently read shards so replay can cross a shard boundary
/// in either direction. A shard larger than the target occupies the cache alone.
pub struct PayloadReader {
    directory: PathBuf,
    shards: Vec<DecodedShard>,
    assembled: Vec<u8>,
}

impl PayloadReader {
    /// Opens payloads on demand relative to this bundle root.
    pub fn new(bundle: impl AsRef<Path>) -> Self {
        Self {
            directory: bundle.as_ref().join(PAYLOADS_DIR_NAME),
            shards: Vec::new(),
            assembled: Vec::new(),
        }
    }

    /// Reads the captured body verbatim, including from a flushed, unfinished shard.
    pub fn read(&mut self, payload: &RawPayloadRef) -> Result<&[u8]> {
        self.assembled = Vec::new();
        ensure!(
            payload.is_complete(),
            "partial payload {}",
            payload.raw_payload_id
        );
        if payload.parts.is_empty() {
            return self.read_inner(payload, /*require_sealed*/ false);
        }
        let mut bytes = Vec::new();
        self.assemble(payload, &mut bytes)?;
        self.assembled = bytes;
        Ok(&self.assembled)
    }

    /// Read the surviving structure for reduction. Known gaps occupy their original slots.
    /// Exact replay uses `read`, which rejects a body with any missing descendant.
    pub(crate) fn observed_json(&mut self, payload: &RawPayloadRef) -> Result<serde_json::Value> {
        if payload.is_complete() {
            return self.read_json(payload);
        }
        let mut bytes = Vec::new();
        self.assemble(payload, &mut bytes)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn assemble(&mut self, payload: &RawPayloadRef, bytes: &mut Vec<u8>) -> Result<()> {
        if !payload.capture.is_complete() {
            serde_json::to_writer(
                bytes,
                &serde_json::json!({"$trace_unavailable": payload.raw_payload_id}),
            )?;
        } else if payload.parts.is_empty() {
            bytes.extend_from_slice(self.read_inner(payload, /*require_sealed*/ false)?);
        } else {
            let fragments = decode_fragments(
                self.read_inner(payload, /*require_sealed*/ false)?,
                payload.parts.len(),
            )?;
            bytes.extend_from_slice(fragments[0].as_bytes());
            for (part, suffix) in payload.parts.iter().zip(&fragments[1..]) {
                self.assemble(part, bytes)?;
                bytes.extend_from_slice(suffix.as_bytes());
            }
        }
        Ok(())
    }

    /// Returns the captured prefix of a truncated body, which can end inside a UTF-8 character.
    pub fn read_prefix(&mut self, payload: &RawPayloadRef) -> Result<&[u8]> {
        ensure!(
            matches!(payload.capture, PayloadCapture::Truncated { .. }),
            "expected a truncated payload"
        );
        self.read_inner(payload, /*require_sealed*/ false)
    }

    /// Decodes the complete JSON at this reference.
    pub fn read_json(&mut self, payload: &RawPayloadRef) -> Result<serde_json::Value> {
        serde_json::from_slice(self.read(payload)?)
            .with_context(|| format!("parse payload {}", payload.raw_payload_id))
    }

    /// Checks every published body, including payloads the semantic reducer does not inspect.
    pub(crate) fn validate(
        &mut self,
        index: &BTreeMap<RawPayloadId, RawPayloadRef>,
        recording_closed: bool,
    ) -> Result<()> {
        // Group reads to decompress each shard just once during validation.
        // Only the last shard can legitimately lack its footer in a live recording.
        let mut refs: Vec<_> = index.values().collect();
        refs.sort_unstable_by_key(|p| p.shard);
        let last = refs.last().and_then(|p| p.shard);
        for payload in refs {
            if matches!(payload.capture, PayloadCapture::Omitted { .. }) {
                ensure!(payload.shard.is_none(), "omitted payload has a shard");
                continue;
            }
            let sealed = recording_closed || payload.shard != last;
            let bytes = self.read_inner(payload, sealed)?;
            if let PayloadCapture::Truncated {
                original_bytes,
                retained_bytes,
            } = payload.capture
            {
                ensure!(
                    bytes.len() == retained_bytes && retained_bytes < original_bytes,
                    "invalid retained length"
                );
                continue;
            }
            if payload.parts.is_empty() {
                // IgnoredAny skips UTF-8 checks inside strings; validate the encoding separately.
                serde_json::from_str::<IgnoredAny>(std::str::from_utf8(bytes)?)
                    .with_context(|| format!("invalid payload {}", payload.raw_payload_id))?;
            } else {
                // Filling the slots with null verifies that this layout is a JSON document.
                let fragments = decode_fragments(bytes, payload.parts.len())?;
                serde_json::from_str::<IgnoredAny>(&fragments.join("null"))?;
            }
        }
        Ok(())
    }

    fn read_inner(&mut self, payload: &RawPayloadRef, require_sealed: bool) -> Result<&[u8]> {
        let shard = payload.shard.context("payload omitted by size limit")?;
        let hit = self.shards.iter().position(|cached| {
            cached.number == shard
                && (cached.sealed
                    || (!require_sealed && cached.records.contains_key(&payload.raw_payload_id)))
        });
        if let Some(index) = hit {
            self.shards.swap(0, index);
        } else {
            // An open shard can have gained a record or footer since the previous read.
            self.shards
                .retain(|cached| cached.number != shard && cached.bytes.len() <= TARGET_BYTES);
            self.shards.truncate(1);
            let decoded = DecodedShard::open(&self.directory, shard)?;
            if decoded.bytes.len() > TARGET_BYTES {
                self.shards.clear();
            }
            self.shards.insert(0, decoded);
        }
        let cached = &self.shards[0];
        ensure!(
            !require_sealed || cached.sealed,
            "missing footer in payload shard {shard}"
        );
        let range = cached
            .records
            .get(&payload.raw_payload_id)
            .with_context(|| {
                format!(
                    "missing payload {} in shard {shard}",
                    payload.raw_payload_id
                )
            })?;
        Ok(&cached.bytes[range.clone()])
    }
}

fn decode_fragments(bytes: &[u8], parts: usize) -> Result<Vec<String>> {
    let fragments: Vec<String> = serde_json::from_slice(bytes)?;
    ensure!(
        fragments.len() == parts + 1,
        "invalid payload fragment count"
    );
    Ok(fragments)
}

#[cfg(test)]
#[path = "shard_tests.rs"]
mod tests;
