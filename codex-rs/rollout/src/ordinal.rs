use std::fs::File;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
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

pub(crate) async fn ordinal_state_for_rollout(path: &Path) -> io::Result<RolloutOrdinalState> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || ordinal_state_for_rollout_blocking(path.as_path()))
        .await
        .map_err(io::Error::other)?
}

fn ordinal_state_for_rollout_blocking(path: &Path) -> io::Result<RolloutOrdinalState> {
    let history_mode = read_history_mode(path)?;
    if matches!(history_mode, ThreadHistoryMode::Legacy) {
        return Ok(RolloutOrdinalState::Legacy);
    }

    let mut scanner = ReverseJsonlScanner::new(File::open(path)?)?;
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
    Ok(RolloutOrdinalState::Paginated {
        next: ordinal.checked_add(1),
    })
}

fn read_history_mode(path: &Path) -> io::Result<ThreadHistoryMode> {
    let reader = BufReader::new(File::open(path)?);
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
        return Ok(session_meta.meta.history_mode);
    }
    Err(io::Error::other(format!(
        "rollout at {} contains no records",
        path.display()
    )))
}
