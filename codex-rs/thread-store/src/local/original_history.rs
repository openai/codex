//! Captures a fixed tail and reads its original-record suffix through one rollout reader.
//! Unfinished lines are excluded; suffix reads require contiguous, complete records.

use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HistoryPosition;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_rollout::ReverseJsonlScanner;
use codex_rollout::RolloutItem;
use codex_rollout::ScanOutcome;
use codex_rollout::open_rollout_seekable_reader;
use codex_rollout::parse_rollout_line_bytes;

use super::LocalThreadStore;
use super::thread_rollout_resolver;
use crate::OriginalHistoryItem;
use crate::OriginalHistoryRead;
use crate::OriginalHistoryRequest;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

#[cfg(test)]
#[path = "original_history_tests.rs"]
mod tests;

pub(super) async fn read(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    request: OriginalHistoryRequest,
) -> ThreadStoreResult<OriginalHistoryRead> {
    let source = thread_rollout_resolver::resolve_current(store, thread_id)
        .await?
        .ok_or(ThreadStoreError::ThreadNotFound { thread_id })?;
    if source.rollout_id != thread_id {
        return Err(ThreadStoreError::Unsupported {
            operation: "original_history_after_revert",
        });
    }
    let (stop, keep_running) = tokio::sync::oneshot::channel::<()>();
    let result = tokio::task::spawn_blocking(move || {
        let mut file = open_rollout_seekable_reader(&source.path, || stop.is_closed())?;
        let mut header = Vec::new();
        BufReader::new(&mut file).read_until(b'\n', &mut header)?;
        let line = parse_rollout_line_bytes(&header).map_err(io::Error::other)?;
        let RolloutItem::SessionMeta(meta) = line.item else {
            return Err(io::Error::other("original history has no session header"));
        };
        if meta.meta.id != thread_id
            || meta.meta.history_mode != ThreadHistoryMode::Paginated
            || meta.meta.history_base.is_some()
            || meta.meta.forked_from_id.is_some()
            || meta.meta.parent_thread_id.is_some()
            || meta.meta.subagent_history_start_ordinal.is_some()
        {
            return Err(io::Error::other(
                "original history requires a paginated root",
            ));
        }
        let end_byte_offset = complete_end(&mut file)?;
        let end_ordinal_exclusive = {
            let mut scanner = ReverseJsonlScanner::new_at(&mut file, end_byte_offset)?;
            let Some(ScanOutcome::Parsed(last)) = scanner.scan_next_rollout_line()? else {
                return Err(io::Error::other("original history has an invalid tail"));
            };
            last.ordinal
                .and_then(|ordinal| ordinal.checked_add(1))
                .ok_or_else(|| io::Error::other("original history tail has no next ordinal"))?
        };
        let end = HistoryPosition {
            thread_id,
            end_ordinal_exclusive,
            end_byte_offset,
        };
        match request {
            OriginalHistoryRequest::Tail => Ok(OriginalHistoryRead {
                items: Vec::new(),
                end,
            }),
            OriginalHistoryRequest::After(start) => {
                if start.thread_id != thread_id
                    || start.end_byte_offset > end.end_byte_offset
                    || start.end_ordinal_exclusive > end.end_ordinal_exclusive
                {
                    return Err(io::Error::other("original history range no longer applies"));
                }
                file.seek(SeekFrom::Start(start.end_byte_offset))?;
                let mut reader =
                    BufReader::new(file.take(end.end_byte_offset - start.end_byte_offset));
                let mut position = start;
                let mut bytes = Vec::new();
                let mut items = Vec::new();
                while reader.read_until(b'\n', &mut bytes)? != 0 {
                    if !bytes.ends_with(b"\n") {
                        return Err(io::Error::other("original history record is incomplete"));
                    }
                    let line = parse_rollout_line_bytes(&bytes).map_err(io::Error::other)?;
                    if line.ordinal != Some(position.end_ordinal_exclusive) {
                        return Err(io::Error::other("original history ordinal changed"));
                    }
                    position.end_byte_offset += bytes.len() as u64;
                    position.end_ordinal_exclusive = position
                        .end_ordinal_exclusive
                        .checked_add(1)
                        .ok_or_else(|| io::Error::other("original history ordinal overflow"))?;
                    items.push(OriginalHistoryItem {
                        item: line.item,
                        end: position,
                    });
                    bytes.clear();
                }
                if position != end {
                    return Err(io::Error::other("original history ended before its cutoff"));
                }
                Ok(OriginalHistoryRead { items, end })
            }
        }
    })
    .await;
    drop(keep_running);
    result
        .map_err(|error| ThreadStoreError::Internal {
            message: format!("original history reader failed: {error}"),
        })?
        .map_err(|error| ThreadStoreError::Internal {
            message: format!("cannot read original history: {error}"),
        })
}

/// Find the last complete record boundary without scanning the earlier history.
fn complete_end(file: &mut (impl Read + Seek)) -> io::Result<u64> {
    let mut end = file.seek(SeekFrom::End(0))?;
    let mut bytes = [0; 8192];
    while end > 0 {
        let count = end.min(bytes.len() as u64) as usize;
        let start = end - count as u64;
        file.seek(SeekFrom::Start(start))?;
        file.read_exact(&mut bytes[..count])?;
        if let Some(index) = bytes[..count].iter().rposition(|byte| *byte == b'\n') {
            return Ok(start + index as u64 + 1);
        }
        end = start;
    }
    Err(io::Error::other("original history has no complete record"))
}
