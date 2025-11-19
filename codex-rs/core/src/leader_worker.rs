use std::collections::HashMap;
use std::collections::HashSet;

use codex_protocol::ConversationId;

use crate::config::LeaderWorkerSettings;
use crate::protocol::LeaderWorkerMode;
use crate::protocol::LeaderWorkerPendingSubtask;
use crate::protocol::LeaderWorkerStatusEvent;
use crate::protocol::LeaderWorkerWorkerState;
use crate::protocol::LeaderWorkerWorkerStatus;

#[derive(Debug, thiserror::Error)]
pub enum LeaderWorkerError {
    #[error("leader-worker workflow is disabled")]
    Disabled,
    #[error("session is not running in leader mode")]
    NotLeader,
    #[error("worker pool is at capacity ({0})")]
    TooManyWorkers(u8),
    #[error("worker already registered")]
    DuplicateWorker,
    #[error("worker not found")]
    WorkerNotFound,
}

#[derive(Debug, Clone)]
struct WorkerRecord {
    state: LeaderWorkerWorkerState,
    summary: Option<String>,
    active_paths: HashSet<String>,
}

impl WorkerRecord {
    fn new(state: LeaderWorkerWorkerState, summary: Option<String>) -> Self {
        Self {
            state,
            summary,
            active_paths: HashSet::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedSubtask {
    pub id: String,
    pub summary: String,
    pub target_paths: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LeaderWorkerManager {
    settings: LeaderWorkerSettings,
    mode: LeaderWorkerMode,
    conversation_id: ConversationId,
    leader_id: Option<ConversationId>,
    worker_id: Option<String>,
    workers: HashMap<String, WorkerRecord>,
    pending_subtasks: Vec<PlannedSubtask>,
}

impl LeaderWorkerManager {
    pub fn new(settings: LeaderWorkerSettings, conversation_id: ConversationId) -> Self {
        let mode = if settings.enabled {
            LeaderWorkerMode::Leader
        } else {
            LeaderWorkerMode::Standard
        };
        Self {
            settings,
            mode,
            conversation_id,
            leader_id: None,
            worker_id: None,
            workers: HashMap::new(),
            pending_subtasks: Vec::new(),
        }
    }

    pub fn mode(&self) -> LeaderWorkerMode {
        self.mode
    }

    pub fn is_enabled(&self) -> bool {
        self.settings.enabled
    }

    pub fn max_workers(&self) -> u8 {
        self.settings.max_workers
    }

    pub fn register_worker(
        &mut self,
        worker_id: impl Into<String>,
    ) -> Result<(), LeaderWorkerError> {
        if !self.settings.enabled {
            return Err(LeaderWorkerError::Disabled);
        }
        if self.mode != LeaderWorkerMode::Leader {
            return Err(LeaderWorkerError::NotLeader);
        }
        if self.workers.len() as u8 >= self.settings.max_workers {
            return Err(LeaderWorkerError::TooManyWorkers(self.settings.max_workers));
        }
        let worker_id = worker_id.into();
        if self.workers.contains_key(&worker_id) {
            return Err(LeaderWorkerError::DuplicateWorker);
        }
        self.workers.insert(
            worker_id,
            WorkerRecord::new(LeaderWorkerWorkerState::Starting, None),
        );
        Ok(())
    }

    pub fn update_worker_state(
        &mut self,
        worker_id: &str,
        state: LeaderWorkerWorkerState,
        summary: Option<String>,
    ) -> Result<(), LeaderWorkerError> {
        if !self.settings.enabled {
            return Err(LeaderWorkerError::Disabled);
        }
        let record = self
            .workers
            .get_mut(worker_id)
            .ok_or(LeaderWorkerError::WorkerNotFound)?;
        record.state = state;
        record.summary = summary;
        Ok(())
    }

    pub fn remove_worker(&mut self, worker_id: &str) -> Result<(), LeaderWorkerError> {
        if !self.settings.enabled {
            return Err(LeaderWorkerError::Disabled);
        }
        self.workers
            .remove(worker_id)
            .map(|_| ())
            .ok_or(LeaderWorkerError::WorkerNotFound)
    }

    pub fn status_snapshot(&self) -> LeaderWorkerStatusEvent {
        let workers = self
            .workers
            .iter()
            .map(|(worker_id, record)| LeaderWorkerWorkerStatus {
                worker_id: worker_id.clone(),
                state: record.state,
                summary: record.summary.clone(),
            })
            .collect();

        let pending_subtasks = if self.pending_subtasks.is_empty() {
            None
        } else {
            Some(
                self.pending_subtasks
                    .iter()
                    .map(|subtask| LeaderWorkerPendingSubtask {
                        id: subtask.id.clone(),
                        summary: subtask.summary.clone(),
                        target_paths: subtask.target_paths.clone(),
                    })
                    .collect(),
            )
        };

        LeaderWorkerStatusEvent {
            mode: self.mode,
            workers,
            pending_subtasks,
        }
    }

    pub fn descriptor(&self) -> crate::protocol::LeaderWorkerSessionDescriptor {
        crate::protocol::LeaderWorkerSessionDescriptor {
            mode: self.mode,
            configured_worker_count: Some(self.settings.default_worker_count),
            max_workers: Some(self.settings.max_workers),
            leader_id: self.leader_id,
            worker_id: self.worker_id.clone(),
        }
    }

    pub fn set_worker_identity(&mut self, leader_id: ConversationId, worker_id: String) {
        self.mode = LeaderWorkerMode::Worker;
        self.leader_id = Some(leader_id);
        self.worker_id = Some(worker_id);
    }

    pub fn plan_subtasks_from_text(&self, request: &str) -> Vec<PlannedSubtask> {
        let mut sentences = request
            .split(|ch| matches!(ch, '.' | '\n' | ';'))
            .map(str::trim)
            .filter(|chunk| !chunk.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();

        if sentences.is_empty() {
            return Vec::new();
        }

        let limit = self.settings.assisted_worker_count(None) as usize;
        sentences.truncate(limit.max(1));

        let mut reserved_paths: HashSet<String> = HashSet::new();
        sentences
            .into_iter()
            .enumerate()
            .map(|(idx, summary)| {
                let extracted = extract_path_hints(&summary)
                    .into_iter()
                    .filter(|path| reserved_paths.insert(path.clone()))
                    .collect::<Vec<_>>();
                PlannedSubtask {
                    id: format!("plan-{}", idx + 1),
                    summary,
                    target_paths: extracted,
                }
            })
            .filter(|subtask| !subtask.summary.trim().is_empty())
            .collect()
    }

    pub fn set_pending_subtasks(&mut self, subtasks: Vec<PlannedSubtask>) {
        self.pending_subtasks = subtasks;
    }

    pub fn pending_subtasks(&self) -> &[PlannedSubtask] {
        &self.pending_subtasks
    }
}

impl LeaderWorkerSettings {
    pub fn assisted_worker_count(&self, requested: Option<u8>) -> u8 {
        requested
            .map(|value| LeaderWorkerSettings::sanitize_worker_count(value, self.max_workers))
            .unwrap_or(self.default_worker_count)
    }
}

fn extract_path_hints(text: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for token in text.split_whitespace() {
        if paths.len() >= 4 {
            break;
        }
        if let Some(sanitized) = sanitize_path_token(token) {
            if sanitized.contains('/') || sanitized.contains('.') {
                paths.push(sanitized);
            }
        }
    }
    paths
}

fn sanitize_path_token(token: &str) -> Option<String> {
    let mut cleaned = String::with_capacity(token.len());
    for ch in token.chars() {
        if ch.is_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-') {
            cleaned.push(ch);
        }
    }
    let cleaned = cleaned
        .trim_matches(|c: char| c == '.' || c == ',')
        .trim()
        .to_string();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}
