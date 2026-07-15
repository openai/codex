use std::fs::File;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Seek;
use std::io::SeekFrom;
use std::path::Path;

use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::ThreadHistoryMode;

use crate::reverse_jsonl_scanner::ReverseJsonlScanner;
use crate::reverse_jsonl_scanner::ScanOutcome;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RolloutOrdinalState {
    Legacy,
    Paginated { next: Option<u64> },
}

impl RolloutOrdinalState {
    pub(crate) fn for_new_rollout(history_mode: ThreadHistoryMode) -> Self {
        match history_mode {
            ThreadHistoryMode::Legacy => Self::Legacy,
            ThreadHistoryMode::Paginated => Self::Paginated { next: Some(0) },
        }
    }

    pub(crate) fn current(&self) -> io::Result<Option<u64>> {
        match self {
            Self::Legacy => Ok(None),
            Self::Paginated { next } => {
                let ordinal = (*next)
                    .ok_or_else(|| io::Error::other("paginated rollout record ordinal overflow"))?;
                Ok(Some(ordinal))
            }
        }
    }

    pub(crate) fn advance(&mut self) {
        if let Self::Paginated { next } = self
            && let Some(ordinal) = *next
        {
            *next = ordinal.checked_add(1);
        }
    }
}

pub(crate) fn ordinal_state_for_rollout(
    file: &mut File,
    path: &Path,
) -> io::Result<RolloutOrdinalState> {
    let Some((history_mode, subagent_history_start_ordinal)) = read_history_metadata(file, path)?
    else {
        return Ok(RolloutOrdinalState::Legacy);
    };
    if matches!(history_mode, ThreadHistoryMode::Legacy) {
        return Ok(RolloutOrdinalState::Legacy);
    }

    let mut scanner = ReverseJsonlScanner::new(file)?;
    let record = loop {
        match scanner.scan_next::<RolloutLine>()? {
            Some(ScanOutcome::Parsed(record)) => break record,
            Some(ScanOutcome::Rejected(_)) => continue,
            None => {
                return Err(io::Error::other(format!(
                    "rollout at {} contains no valid records",
                    path.display()
                )));
            }
        }
    };
    let ordinal = record.ordinal.ok_or_else(|| {
        io::Error::other(format!(
            "final paginated rollout record at {} is missing an ordinal",
            path.display()
        ))
    })?;
    // The child metadata is written before its inherited prefix. Never resume a
    // partial initialization: later child records would otherwise stay below the boundary.
    if let Some(prefix_end) = subagent_history_start_ordinal.and_then(|start| start.checked_sub(1))
        && ordinal < prefix_end
    {
        return Err(io::Error::other(format!(
            "paginated subagent rollout at {} is incomplete: expected inherited prefix through ordinal {prefix_end}, found final durable ordinal {ordinal}",
            path.display()
        )));
    }
    Ok(RolloutOrdinalState::Paginated {
        next: ordinal.checked_add(1),
    })
}

fn read_history_metadata(
    file: &mut File,
    path: &Path,
) -> io::Result<Option<(ThreadHistoryMode, Option<u64>)>> {
    file.seek(SeekFrom::Start(0))?;
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let record: RolloutLine = serde_json::from_str(line.as_str()).map_err(|error| {
            io::Error::other(format!(
                "failed to parse first rollout record at {}: {error}",
                path.display()
            ))
        })?;
        let RolloutItem::SessionMeta(session_meta) = record.item else {
            return Err(io::Error::other(format!(
                "rollout at {} does not start with session metadata",
                path.display()
            )));
        };
        return Ok(Some((
            session_meta.meta.history_mode,
            session_meta.meta.subagent_history_start_ordinal,
        )));
    }
    Ok(None)
}
