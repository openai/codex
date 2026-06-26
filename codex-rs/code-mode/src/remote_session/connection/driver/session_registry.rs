use std::collections::HashMap;
use std::sync::Arc;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::host::SessionId;
use codex_code_mode_protocol::host::WireCellId;

use super::cell_ids::public_cell_id;
use super::cleanup::SessionCleanup;
use super::types::RemoteSession;

pub(super) struct CellOwner {
    pub(super) session_id: SessionId,
    pub(super) cell_id: CellId,
    pub(super) delegate: Arc<dyn CodeModeSessionDelegate>,
}

pub(super) struct DelegateTarget {
    pub(super) session_id: SessionId,
    pub(super) cell_id: CellId,
    pub(super) delegate: Arc<dyn CodeModeSessionDelegate>,
}

pub(super) struct FailedSession {
    pub(super) cleanup: SessionCleanup,
    pub(super) cells: Vec<CellOwner>,
}

pub(super) enum CellAdmissionError {
    MissingSession,
    DuplicateCell,
}

struct SessionRecord {
    remote: RemoteSession,
    delegate: Arc<dyn CodeModeSessionDelegate>,
    cleanup: SessionCleanup,
    phase: SessionPhase,
    cells: HashMap<WireCellId, CellId>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SessionPhase {
    Ready,
    Closing,
}

pub(super) struct SessionRegistry {
    records: HashMap<SessionId, SessionRecord>,
}

impl SessionRegistry {
    pub(super) fn new() -> Self {
        Self {
            records: HashMap::new(),
        }
    }

    pub(super) fn contains(&self, session_id: &SessionId) -> bool {
        self.records.contains_key(session_id)
    }

    pub(super) fn insert_ready(
        &mut self,
        session: RemoteSession,
        delegate: Arc<dyn CodeModeSessionDelegate>,
        cleanup: SessionCleanup,
    ) {
        self.records.insert(
            session.id.clone(),
            SessionRecord {
                remote: session,
                delegate,
                cleanup,
                phase: SessionPhase::Ready,
                cells: HashMap::new(),
            },
        );
    }

    pub(super) fn require_ready(&self, session: &RemoteSession) -> Result<(), String> {
        let record = self
            .records
            .get(&session.id)
            .ok_or_else(|| format!("unknown code-mode session {}", session.id))?;
        if record.remote != *session {
            return Err("stale code-mode session generation".to_string());
        }
        if record.phase != SessionPhase::Ready {
            return Err("code-mode session is shutting down".to_string());
        }
        Ok(())
    }

    pub(super) fn begin_shutdown(&mut self, session: &RemoteSession) -> Result<(), String> {
        let record = self
            .records
            .get_mut(&session.id)
            .ok_or_else(|| format!("unknown code-mode session {}", session.id))?;
        if record.remote != *session {
            return Err("stale code-mode session generation".to_string());
        }
        if record.phase == SessionPhase::Closing {
            return Err("code-mode session is already closing".to_string());
        }
        record.phase = SessionPhase::Closing;
        Ok(())
    }

    pub(super) fn begin_abandoned_shutdown(&mut self, session_id: &SessionId) -> Option<bool> {
        let record = self.records.get_mut(session_id)?;
        if record.phase == SessionPhase::Closing {
            return Some(false);
        }
        record.phase = SessionPhase::Closing;
        Some(true)
    }

    pub(super) fn is_closing(&self, session_id: &SessionId) -> Option<bool> {
        self.records
            .get(session_id)
            .map(|record| record.phase == SessionPhase::Closing)
    }

    pub(super) fn admit_cell(
        &mut self,
        session: &RemoteSession,
        cell_id: WireCellId,
    ) -> Result<CellId, CellAdmissionError> {
        let Some(record) = self.records.get_mut(&session.id) else {
            return Err(CellAdmissionError::MissingSession);
        };
        if record.cells.contains_key(&cell_id) {
            return Err(CellAdmissionError::DuplicateCell);
        }
        let public_id = public_cell_id(session.generation, &cell_id);
        record.cells.insert(cell_id, public_id.clone());
        Ok(public_id)
    }

    pub(super) fn delegate_target(
        &self,
        session_id: &SessionId,
        cell_id: &WireCellId,
    ) -> Result<DelegateTarget, String> {
        let session = self
            .records
            .get(session_id)
            .ok_or_else(|| format!("code-mode host delegated for unknown session {session_id}"))?;
        let public_id = session.cells.get(cell_id).cloned().ok_or_else(|| {
            format!(
                "code-mode host delegated for unknown cell {} in session {session_id}",
                cell_id.as_str()
            )
        })?;
        Ok(DelegateTarget {
            session_id: session_id.clone(),
            cell_id: public_id,
            delegate: Arc::clone(&session.delegate),
        })
    }

    pub(super) fn remove_cell(
        &mut self,
        session_id: &SessionId,
        cell_id: &WireCellId,
    ) -> Result<CellOwner, String> {
        let session = self.records.get_mut(session_id).ok_or_else(|| {
            format!(
                "code-mode host closed cell {} in unknown session {session_id}",
                cell_id.as_str()
            )
        })?;
        let public_id = session
            .cells
            .remove(cell_id)
            .ok_or_else(|| format!("code-mode host closed unknown cell in session {session_id}"))?;
        Ok(CellOwner {
            session_id: session_id.clone(),
            cell_id: public_id,
            delegate: Arc::clone(&session.delegate),
        })
    }

    pub(super) fn remove_session(&mut self, session_id: &SessionId) -> Vec<CellOwner> {
        let Some(session) = self.records.remove(session_id) else {
            return Vec::new();
        };
        session
            .cells
            .into_values()
            .map(|cell_id| CellOwner {
                session_id: session_id.clone(),
                cell_id,
                delegate: Arc::clone(&session.delegate),
            })
            .collect()
    }

    pub(super) fn drain(&mut self) -> Vec<FailedSession> {
        let sessions = std::mem::take(&mut self.records);
        sessions
            .into_iter()
            .map(|(session_id, session)| {
                let cells = session
                    .cells
                    .into_values()
                    .map(|cell_id| CellOwner {
                        session_id: session_id.clone(),
                        cell_id,
                        delegate: Arc::clone(&session.delegate),
                    })
                    .collect();
                FailedSession {
                    cleanup: session.cleanup,
                    cells,
                }
            })
            .collect()
    }
}
