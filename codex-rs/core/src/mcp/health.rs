use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    Unknown,
    Passing,
    Failing,
}

#[derive(Debug, Clone)]
pub struct HealthReport {
    pub status: HealthStatus,
    pub last_checked_at: Option<SystemTime>,
    pub notes: Option<String>,
}

impl HealthReport {
    pub fn new(status: HealthStatus) -> Self {
        Self {
            status,
            last_checked_at: None,
            notes: None,
        }
    }

    pub fn with_notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }

    pub fn unknown() -> Self {
        Self::new(HealthStatus::Unknown).with_notes("health checks not implemented yet")
    }
}
