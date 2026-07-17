use std::collections::VecDeque;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::time::SystemTime;

use super::HISTORY_READ_BUFFER_SIZE;
use super::HistoryConfig;
use super::HistoryEntry;
use super::MAX_RETRIES;
use super::RETRY_SLEEP;
use super::history_filepath;
use super::log_identity;

const MAX_BATCH_ROWS: usize = 128;
const MAX_BATCH_BYTES: usize = 64 * 1024;

/// Position of the newest record to include in a bounded history lookup.
///
/// The initial cursor identifies only an absolute row offset. Continuation cursors also retain a
/// byte position so older batches can scan backward from the previous batch instead of rescanning
/// the history prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryBatchCursor {
    end_offset: usize,
    byte_anchor: Option<HistoryByteAnchor>,
}

impl HistoryBatchCursor {
    /// Creates an initial cursor ending at the given absolute history offset.
    pub fn new(end_offset: usize) -> Self {
        Self {
            end_offset,
            byte_anchor: None,
        }
    }

    /// Returns the absolute history offset covered first by this cursor.
    pub fn end_offset(self) -> usize {
        self.end_offset
    }
}

/// Validated row boundary used to continue scanning one unchanged file revision.
///
/// Byte positions and file lengths use `u64` to match filesystem and seek APIs, while history row
/// offsets use `usize` to match collection indices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HistoryByteAnchor {
    position: u64,
    revision: HistoryFileRevision,
}

/// File metadata that must remain unchanged before a byte position can be reused.
///
/// Byte positions are only reused for uncapped histories, which Codex writes append-only. Capped
/// histories can be rewritten in place when they are trimmed, so their cursors always fall back to
/// an offset scan. Filesystems without a modification time also fall back to an offset scan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HistoryFileRevision {
    len: u64,
    modified: Option<SystemTime>,
}

/// One absolute history offset covered by a bounded lookup.
///
/// Malformed records retain their offset with `entry` set to `None`, allowing callers to continue
/// searching older valid records without changing offset semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryBatchEntry {
    /// Zero-based position in the history file, counted from the oldest record.
    pub offset: usize,
    /// Parsed record, or `None` when the row at `offset` is malformed.
    pub entry: Option<HistoryEntry>,
}

/// A bounded newest-first suffix ending at a requested absolute history offset.
///
/// `next_older_cursor` identifies the next position a caller should request after exhausting
/// `entries`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HistoryBatch {
    /// Covered records in newest-to-oldest order.
    pub entries: Vec<HistoryBatchEntry>,
    /// Next position to request after exhausting `entries`.
    pub next_older_cursor: Option<HistoryBatchCursor>,
}

struct RawHistoryBatchEntry {
    offset: usize,
    byte_position: u64,
    byte_len: usize,
    /// Oversized rows remain unbuffered until the scan determines they belong in the result.
    bytes: Option<Vec<u8>>,
}

/// Look up a bounded batch of history records ending at `cursor`.
///
/// The file is opened, identity-checked, and shared-locked once. Records are counted from the
/// oldest offset on the initial lookup. Continuation lookups scan backward from the byte position
/// returned with the previous batch. The result retains at most 128 rows and 64 KiB of raw JSONL,
/// except that one oversized newest row is returned alone so callers always make progress.
///
/// # Errors
///
/// Returns an I/O error when the history file cannot be opened, inspected, locked, or read.
pub fn lookup_batch(
    log_id: u64,
    cursor: HistoryBatchCursor,
    config: &HistoryConfig,
) -> std::io::Result<HistoryBatch> {
    let path = history_filepath(config);
    let mut file = OpenOptions::new().read(true).open(path)?;
    let current_log_id = log_identity(&file.metadata()?).unwrap_or(0);
    if log_id != 0 && current_log_id != log_id {
        return Ok(HistoryBatch::default());
    }

    for _ in 0..MAX_RETRIES {
        match file.try_lock_shared() {
            Ok(()) => return scan_batch(&mut file, cursor, config),
            Err(std::fs::TryLockError::WouldBlock) => std::thread::sleep(RETRY_SLEEP),
            Err(error) => return Err(error.into()),
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::WouldBlock,
        "could not acquire shared history lock after multiple attempts",
    ))
}

/// Selects the anchored backward scan only for an unchanged, uncapped history file.
///
/// Capped histories always use the forward scan because trimming rewrites them in place and a
/// same-size rewrite may not be distinguishable from metadata on filesystems with coarse
/// modification times. Falling back preserves absolute row semantics at the cost of rescanning
/// that request from the beginning.
fn scan_batch(
    file: &mut File,
    cursor: HistoryBatchCursor,
    config: &HistoryConfig,
) -> std::io::Result<HistoryBatch> {
    let metadata = file.metadata()?;
    let revision = HistoryFileRevision {
        len: metadata.len(),
        modified: metadata.modified().ok(),
    };
    if config.max_bytes.is_none()
        && let Some(anchor) = cursor.byte_anchor
        && anchor.revision == revision
    {
        return scan_batch_backward(file, cursor.end_offset, anchor.position, revision);
    }

    file.seek(SeekFrom::Start(0))?;
    let mut batch = scan_batch_forward(file, cursor.end_offset, revision)?;
    if config.max_bytes.is_some()
        && let Some(next_older_cursor) = &mut batch.next_older_cursor
    {
        next_older_cursor.byte_anchor = None;
    }
    Ok(batch)
}

/// Streams from byte zero through `end_offset`, retaining only the bounded newest suffix.
///
/// This path establishes byte positions for later continuation cursors and is also the safe
/// fallback when an existing cursor belongs to an older file revision. Oversized rows are tracked
/// by position and length, then materialized only if they remain in the returned suffix.
fn scan_batch_forward(
    file: &mut File,
    end_offset: usize,
    revision: HistoryFileRevision,
) -> std::io::Result<HistoryBatch> {
    let mut suffix = VecDeque::new();
    let mut suffix_bytes = 0usize;
    {
        let mut byte_position = 0u64;
        let mut reader = BufReader::with_capacity(HISTORY_READ_BUFFER_SIZE, &mut *file);

        'rows: for offset in 0..=end_offset {
            let mut byte_len = 0usize;
            let mut bytes = Some(Vec::new());
            loop {
                let buffer = reader.fill_buf()?;
                if buffer.is_empty() {
                    if byte_len == 0 {
                        break 'rows;
                    }
                    break;
                }

                let newline = buffer.iter().position(|byte| *byte == b'\n');
                let consumed = newline.map_or(buffer.len(), |index| index + 1);
                byte_len = byte_len.saturating_add(consumed);
                if let Some(buffered) = bytes.as_mut() {
                    if byte_len <= MAX_BATCH_BYTES {
                        buffered.extend_from_slice(&buffer[..consumed]);
                    } else {
                        bytes = None;
                    }
                }
                reader.consume(consumed);
                if newline.is_some() {
                    break;
                }
            }

            retain_row(
                &mut suffix,
                &mut suffix_bytes,
                RawHistoryBatchEntry {
                    offset,
                    byte_position,
                    byte_len,
                    bytes,
                },
            );
            byte_position += byte_len as u64;
        }
    }

    finish_materialized_batch(file, suffix.into_iter().rev().collect(), revision)
}

/// Reads complete rows backward from a validated exclusive byte boundary.
///
/// `end_byte_position` must be the start of the row immediately newer than `end_offset`. Scanning
/// in reverse lets each continuation touch only its own rows while preserving absolute offsets.
fn scan_batch_backward(
    file: &mut File,
    end_offset: usize,
    end_byte_position: u64,
    revision: HistoryFileRevision,
) -> std::io::Result<HistoryBatch> {
    let mut entries = Vec::new();
    let mut entries_bytes = 0usize;
    let mut reversed_row = Some(Vec::new());
    let mut row_byte_len = 0usize;
    let mut read_buffer = [0u8; HISTORY_READ_BUFFER_SIZE];
    let mut read_end = end_byte_position;
    let mut offset = end_offset;

    while read_end > 0 {
        let read_start = read_end.saturating_sub(HISTORY_READ_BUFFER_SIZE as u64);
        let read_len = usize::try_from(read_end - read_start).unwrap_or(HISTORY_READ_BUFFER_SIZE);
        file.seek(SeekFrom::Start(read_start))?;
        file.read_exact(&mut read_buffer[..read_len])?;

        for index in (0..read_len).rev() {
            let byte = read_buffer[index];
            if byte == b'\n' && row_byte_len > 0 {
                if let Some(bytes) = reversed_row.as_mut() {
                    bytes.reverse();
                }
                let raw = RawHistoryBatchEntry {
                    offset,
                    byte_position: read_start + index as u64 + 1,
                    byte_len: row_byte_len,
                    bytes: reversed_row.take(),
                };
                if !retain_newest_row(&mut entries, &mut entries_bytes, raw) {
                    return finish_materialized_batch(file, entries, revision);
                }
                let Some(next_offset) = offset.checked_sub(1) else {
                    return finish_materialized_batch(file, entries, revision);
                };
                offset = next_offset;
                reversed_row = Some(vec![b'\n']);
                row_byte_len = 1;
            } else {
                row_byte_len = row_byte_len.saturating_add(1);
                if let Some(bytes) = reversed_row.as_mut() {
                    if row_byte_len <= MAX_BATCH_BYTES {
                        bytes.push(byte);
                    } else {
                        reversed_row = None;
                    }
                }
            }
        }
        read_end = read_start;
    }

    if row_byte_len > 0 {
        if let Some(bytes) = reversed_row.as_mut() {
            bytes.reverse();
        }
        retain_newest_row(
            &mut entries,
            &mut entries_bytes,
            RawHistoryBatchEntry {
                offset,
                byte_position: 0,
                byte_len: row_byte_len,
                bytes: reversed_row,
            },
        );
    }
    finish_materialized_batch(file, entries, revision)
}

fn finish_materialized_batch(
    file: &mut File,
    mut entries: Vec<RawHistoryBatchEntry>,
    revision: HistoryFileRevision,
) -> std::io::Result<HistoryBatch> {
    for entry in &mut entries {
        if entry.bytes.is_none() {
            file.seek(SeekFrom::Start(entry.byte_position))?;
            let mut bytes = vec![0; entry.byte_len];
            file.read_exact(&mut bytes)?;
            entry.bytes = Some(bytes);
        }
    }
    finish_batch(entries, revision)
}

/// Retains the newest suffix seen by a forward scan under both row and byte caps.
///
/// A single oversized row replaces the suffix so the newest requested record is always returned
/// and callers can continue to an older cursor.
fn retain_row(
    suffix: &mut VecDeque<RawHistoryBatchEntry>,
    suffix_bytes: &mut usize,
    entry: RawHistoryBatchEntry,
) {
    let row_bytes = entry.byte_len;
    if row_bytes > MAX_BATCH_BYTES {
        suffix.clear();
        *suffix_bytes = row_bytes;
        suffix.push_back(entry);
        return;
    }

    *suffix_bytes += row_bytes;
    suffix.push_back(entry);
    while suffix.len() > MAX_BATCH_ROWS || *suffix_bytes > MAX_BATCH_BYTES {
        if let Some(removed) = suffix.pop_front() {
            *suffix_bytes -= removed.byte_len;
        }
    }
}

/// Appends one newest-to-oldest row and reports whether the backward scan should continue.
///
/// Returning `false` means the batch is complete. An oversized first row is retained alone;
/// otherwise the row that would exceed a cap is left for the next batch.
fn retain_newest_row(
    entries: &mut Vec<RawHistoryBatchEntry>,
    entries_bytes: &mut usize,
    entry: RawHistoryBatchEntry,
) -> bool {
    let row_bytes = entry.byte_len;
    if entries.is_empty() && row_bytes > MAX_BATCH_BYTES {
        entries.push(entry);
        return false;
    }
    if entries.len() == MAX_BATCH_ROWS || entries_bytes.saturating_add(row_bytes) > MAX_BATCH_BYTES
    {
        return false;
    }
    *entries_bytes += row_bytes;
    entries.push(entry);
    true
}

/// Parses newest-first rows and anchors the continuation at the oldest retained row's start.
fn finish_batch(
    entries: Vec<RawHistoryBatchEntry>,
    revision: HistoryFileRevision,
) -> std::io::Result<HistoryBatch> {
    let next_older_cursor = entries.last().and_then(|entry| {
        entry
            .offset
            .checked_sub(1)
            .map(|end_offset| HistoryBatchCursor {
                end_offset,
                byte_anchor: revision.modified.map(|_| HistoryByteAnchor {
                    position: entry.byte_position,
                    revision,
                }),
            })
    });
    let entries = entries
        .into_iter()
        .map(|raw| {
            let bytes = raw.bytes.ok_or_else(|| {
                std::io::Error::other("retained history row was not materialized")
            })?;
            Ok(HistoryBatchEntry {
                offset: raw.offset,
                entry: try_parse_entry(&bytes),
            })
        })
        .collect::<std::io::Result<Vec<_>>>()?;
    Ok(HistoryBatch {
        entries,
        next_older_cursor,
    })
}

fn try_parse_entry(raw: &[u8]) -> Option<HistoryEntry> {
    let raw = raw.strip_suffix(b"\n").unwrap_or(raw);
    let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
    serde_json::from_slice(raw).ok()
}
