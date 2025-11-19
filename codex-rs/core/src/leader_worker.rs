use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;

use codex_protocol::ConversationId;

use crate::config::LeaderWorkerSettings;
use crate::protocol::LeaderWorkerAggregationSummaryEvent;
use crate::protocol::LeaderWorkerAssignmentResultEvent;
use crate::protocol::LeaderWorkerAssignmentStatus;
use crate::protocol::LeaderWorkerInFlightAssignment;
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
    #[error("subtask not found")]
    SubtaskNotFound,
    #[error("assignment not found")]
    AssignmentNotFound,
    #[error("worker is busy with an assignment")]
    WorkerBusy,
    #[error("worker is not paused")]
    NotPaused,
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
    in_flight: Vec<LeaderWorkerInFlightAssignment>,
    completed: Vec<CompletedAssignment>,
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
            in_flight: Vec::new(),
            completed: Vec::new(),
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

    fn worker_in_flight(&self, worker_id: &str) -> bool {
        self.in_flight
            .iter()
            .any(|assignment| assignment.worker_id == worker_id)
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
        if self.worker_in_flight(worker_id) {
            return Err(LeaderWorkerError::WorkerBusy);
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
            in_flight_assignments: if self.in_flight.is_empty() {
                None
            } else {
                Some(self.in_flight.clone())
            },
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

    pub fn is_plan_complete(&self) -> bool {
        self.pending_subtasks.is_empty() && self.in_flight.is_empty()
    }

    pub fn clear_assignments(&mut self) {
        self.in_flight.clear();
        for record in self.workers.values_mut() {
            record.active_paths.clear();
            if record.state == LeaderWorkerWorkerState::Running
                || record.state == LeaderWorkerWorkerState::Starting
            {
                record.state = LeaderWorkerWorkerState::Idle;
                record.summary = None;
            }
        }
    }

    pub fn begin_assignment(
        &mut self,
        worker_id: &str,
        subtask_id: &str,
    ) -> Result<(), LeaderWorkerError> {
        if !self.settings.enabled {
            return Err(LeaderWorkerError::Disabled);
        }
        let record = self
            .workers
            .get(worker_id)
            .ok_or(LeaderWorkerError::WorkerNotFound)?;
        if record.state == LeaderWorkerWorkerState::Paused {
            return Err(LeaderWorkerError::WorkerBusy);
        }
        let idx = self
            .pending_subtasks
            .iter()
            .position(|subtask| subtask.id == subtask_id)
            .ok_or(LeaderWorkerError::SubtaskNotFound)?;
        let subtask = self.pending_subtasks.remove(idx);
        self.in_flight.push(LeaderWorkerInFlightAssignment {
            worker_id: worker_id.to_string(),
            subtask_id: subtask.id.clone(),
            description: subtask.summary.clone(),
            target_paths: subtask.target_paths.clone(),
        });
        if let Some(record) = self.workers.get_mut(worker_id) {
            record.state = LeaderWorkerWorkerState::Running;
            record.summary = Some(subtask.summary.clone());
            record
                .active_paths
                .extend(subtask.target_paths.iter().cloned());
        }
        Ok(())
    }

    pub fn finish_assignment(
        &mut self,
        worker_id: &str,
        subtask_id: &str,
    ) -> Result<(), LeaderWorkerError> {
        if !self.settings.enabled {
            return Err(LeaderWorkerError::Disabled);
        }
        if !self.workers.contains_key(worker_id) {
            return Err(LeaderWorkerError::WorkerNotFound);
        }
        let before = self.in_flight.len();
        self.in_flight.retain(|assignment| {
            !(assignment.worker_id == worker_id && assignment.subtask_id == subtask_id)
        });
        if self.in_flight.len() == before {
            return Err(LeaderWorkerError::AssignmentNotFound);
        }
        if let Some(record) = self.workers.get_mut(worker_id) {
            record.active_paths.clear();
            record.state = LeaderWorkerWorkerState::Idle;
            record.summary = None;
        }
        Ok(())
    }

    pub fn pause_worker(&mut self, worker_id: &str) -> Result<(), LeaderWorkerError> {
        if !self.settings.enabled {
            return Err(LeaderWorkerError::Disabled);
        }
        if self.worker_in_flight(worker_id) {
            return Err(LeaderWorkerError::WorkerBusy);
        }
        let record = self
            .workers
            .get_mut(worker_id)
            .ok_or(LeaderWorkerError::WorkerNotFound)?;
        record.state = LeaderWorkerWorkerState::Paused;
        record.summary = Some("Paused by user".to_string());
        Ok(())
    }

    pub fn resume_worker(&mut self, worker_id: &str) -> Result<(), LeaderWorkerError> {
        if !self.settings.enabled {
            return Err(LeaderWorkerError::Disabled);
        }
        let record = self
            .workers
            .get_mut(worker_id)
            .ok_or(LeaderWorkerError::WorkerNotFound)?;
        if record.state != LeaderWorkerWorkerState::Paused {
            return Err(LeaderWorkerError::NotPaused);
        }
        record.state = LeaderWorkerWorkerState::Idle;
        record.summary = Some("Awaiting assignment".to_string());
        Ok(())
    }

    pub fn add_worker(&mut self, worker_id: impl Into<String>) -> Result<(), LeaderWorkerError> {
        let worker_id = worker_id.into();
        self.register_worker(worker_id.clone())?;
        self.update_worker_state(
            &worker_id,
            LeaderWorkerWorkerState::Idle,
            Some("Awaiting assignment".to_string()),
        )?;
        Ok(())
    }

    pub fn complete_assignment(
        &mut self,
        worker_id: &str,
        subtask_id: &str,
        status: LeaderWorkerAssignmentStatus,
        summary: Option<String>,
        files_changed: Vec<String>,
    ) -> Result<CompletedAssignment, LeaderWorkerError> {
        self.finish_assignment(worker_id, subtask_id)?;
        let completed = CompletedAssignment {
            worker_id: worker_id.to_string(),
            subtask_id: subtask_id.to_string(),
            status,
            summary,
            files_changed,
        };
        self.completed.push(completed.clone());
        Ok(completed)
    }

    pub fn latest_completed(&self) -> Option<&CompletedAssignment> {
        self.completed.last()
    }

    pub fn aggregate_summary(&self) -> LeaderWorkerAggregationSummaryEvent {
        let mut files = BTreeSet::new();
        let mut success = 0u32;
        let mut failure = 0u32;
        for completed in &self.completed {
            match completed.status {
                LeaderWorkerAssignmentStatus::Success => success += 1,
                LeaderWorkerAssignmentStatus::Failure => failure += 1,
                LeaderWorkerAssignmentStatus::Cancelled => {}
            }
            for path in &completed.files_changed {
                files.insert(path.clone());
            }
        }
        LeaderWorkerAggregationSummaryEvent {
            success_count: success,
            failure_count: failure,
            files_changed: files.into_iter().collect(),
        }
    }
}

impl LeaderWorkerSettings {
    pub fn assisted_worker_count(&self, requested: Option<u8>) -> u8 {
        requested
            .map(|value| LeaderWorkerSettings::sanitize_worker_count(value, self.max_workers))
            .unwrap_or(self.default_worker_count)
    }
}

#[derive(Debug, Clone)]
pub struct CompletedAssignment {
    pub worker_id: String,
    pub subtask_id: String,
    pub status: LeaderWorkerAssignmentStatus,
    pub summary: Option<String>,
    pub files_changed: Vec<String>,
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
