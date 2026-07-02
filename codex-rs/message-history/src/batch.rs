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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HistoryByteAnchor {
    position: u64,
    revision: HistoryFileRevision,
}

/// File metadata that must remain unchanged before a byte position can be reused.
///
/// A length alone cannot detect an in-place trim followed by an append, so cursors also retain the
/// modification time. Filesystems without a modification time always fall back to an offset scan.
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
/// `entries`. It carries a byte position because the byte cap can make a batch contain fewer than
/// 128 rows and because continuation lookups must not rescan already traversed prefixes.
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
    bytes: Vec<u8>,
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
            Ok(()) => return scan_batch(&mut file, cursor),
            Err(std::fs::TryLockError::WouldBlock) => std::thread::sleep(RETRY_SLEEP),
            Err(error) => return Err(error.into()),
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::WouldBlock,
        "could not acquire shared history lock after multiple attempts",
    ))
}

/// Selects the anchored backward scan only when the file revision is unchanged.
///
/// Falling back to the forward scan preserves absolute row semantics after concurrent appends,
/// trims, or rewrites, at the cost of rescanning that one request from the beginning.
fn scan_batch(file: &mut File, cursor: HistoryBatchCursor) -> std::io::Result<HistoryBatch> {
    let metadata = file.metadata()?;
    let revision = HistoryFileRevision {
        len: metadata.len(),
        modified: metadata.modified().ok(),
    };
    if let Some(anchor) = cursor.byte_anchor
        && anchor.revision == revision
    {
        return scan_batch_backward(file, cursor.end_offset, anchor.position, revision);
    }

    file.seek(SeekFrom::Start(0))?;
    scan_batch_forward(file, cursor.end_offset, revision)
}

/// Streams from byte zero through `end_offset`, retaining only the bounded newest suffix.
///
/// This path establishes byte positions for later continuation cursors and is also the safe
/// fallback when an existing cursor belongs to an older file revision.
fn scan_batch_forward(
    file: &mut File,
    end_offset: usize,
    revision: HistoryFileRevision,
) -> std::io::Result<HistoryBatch> {
    let mut suffix = VecDeque::new();
    let mut suffix_bytes = 0usize;
    let mut byte_position = 0u64;
    let mut reader = BufReader::with_capacity(HISTORY_READ_BUFFER_SIZE, file);

    for offset in 0..=end_offset {
        let mut bytes = Vec::new();
        let read = reader.read_until(b'\n', &mut bytes)?;
        if read == 0 {
            break;
        }
        retain_row(&mut suffix, &mut suffix_bytes, offset, byte_position, bytes);
        byte_position += read as u64;
    }
    Ok(finish_batch(suffix.into_iter().rev().collect(), revision))
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
    let mut reversed_row = Vec::new();
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
            if byte == b'\n' && !reversed_row.is_empty() {
                reversed_row.reverse();
                let raw = RawHistoryBatchEntry {
                    offset,
                    byte_position: read_start + index as u64 + 1,
                    bytes: std::mem::take(&mut reversed_row),
                };
                if !retain_newest_row(&mut entries, &mut entries_bytes, raw) {
                    return Ok(finish_batch(entries, revision));
                }
                let Some(next_offset) = offset.checked_sub(1) else {
                    return Ok(finish_batch(entries, revision));
                };
                offset = next_offset;
                reversed_row.push(b'\n');
            } else {
                reversed_row.push(byte);
            }
        }
        read_end = read_start;
    }

    if !reversed_row.is_empty() {
        reversed_row.reverse();
        retain_newest_row(
            &mut entries,
            &mut entries_bytes,
            RawHistoryBatchEntry {
                offset,
                byte_position: 0,
                bytes: reversed_row,
            },
        );
    }
    Ok(finish_batch(entries, revision))
}

/// Retains the newest suffix seen by a forward scan under both row and byte caps.
///
/// A single oversized row replaces the suffix so the newest requested record is always returned
/// and callers can continue to an older cursor.
fn retain_row(
    suffix: &mut VecDeque<RawHistoryBatchEntry>,
    suffix_bytes: &mut usize,
    offset: usize,
    byte_position: u64,
    bytes: Vec<u8>,
) {
    let row_bytes = bytes.len();
    if row_bytes > MAX_BATCH_BYTES {
        suffix.clear();
        *suffix_bytes = row_bytes;
        suffix.push_back(RawHistoryBatchEntry {
            offset,
            byte_position,
            bytes,
        });
        return;
    }

    *suffix_bytes += row_bytes;
    suffix.push_back(RawHistoryBatchEntry {
        offset,
        byte_position,
        bytes,
    });
    while suffix.len() > MAX_BATCH_ROWS || *suffix_bytes > MAX_BATCH_BYTES {
        if let Some(removed) = suffix.pop_front() {
            *suffix_bytes -= removed.bytes.len();
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
    let row_bytes = entry.bytes.len();
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
fn finish_batch(entries: Vec<RawHistoryBatchEntry>, revision: HistoryFileRevision) -> HistoryBatch {
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
        .map(|raw| HistoryBatchEntry {
            offset: raw.offset,
            entry: parse_entry(&raw.bytes),
        })
        .collect();
    HistoryBatch {
        entries,
        next_older_cursor,
    }
}

fn parse_entry(raw: &[u8]) -> Option<HistoryEntry> {
    let raw = raw.strip_suffix(b"\n").unwrap_or(raw);
    let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
    serde_json::from_slice(raw).ok()
}
