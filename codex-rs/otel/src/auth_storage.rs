//! Credential-storage metrics contain only bounded classifications, never credentials or paths.
//! Shared metrics infrastructure supplies OS/version metadata and startup buffering.

mod originator;

pub use originator::AuthStorageOriginator;
use std::error::Error;
use std::time::Duration;
use std::time::Instant;
use strum_macros::AsRefStr;

#[derive(Clone, Copy, Debug, AsRefStr)]
#[strum(serialize_all = "snake_case")]
pub enum CredentialKind {
    Codex,
    Mcp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, AsRefStr)]
#[strum(serialize_all = "snake_case")]
pub enum StoreMode {
    File,
    Auto,
    Keyring,
    Ephemeral,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, AsRefStr)]
#[strum(serialize_all = "snake_case")]
pub enum Store {
    File,
    DirectKeyring,
    Secrets,
    Ephemeral,
    /// A deletion spanning the selected backend, plaintext fallback, and legacy entries.
    Multiple,
}

#[derive(Clone, Copy, Debug, AsRefStr)]
#[strum(serialize_all = "snake_case")]
pub enum Operation {
    Load,
    Save,
    Delete,
    Cleanup,
    RefreshPersist,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, AsRefStr)]
#[strum(serialize_all = "snake_case")]
enum Outcome {
    NotAttempted,
    Success,
    NotFound,
    Error,
}

#[derive(Clone, Copy, Debug)]
struct Observation {
    credential_kind: CredentialKind,
    mode: StoreMode,
    selected_store: Store,
    actual_store: Store,
    operation: Operation,
    secure_outcome: Outcome,
    outcome: Outcome,
    duration: Duration,
    phase: StoragePhase,
    secure_error: &'static str,
    originator: AuthStorageOriginator,
}

impl Observation {
    fn record(self) {
        let fallback = match (
            self.mode,
            self.phase,
            self.actual_store,
            self.secure_outcome,
        ) {
            (StoreMode::Auto, StoragePhase::Policy, Store::File, Outcome::Error) => "secure_error",
            (StoreMode::Auto, StoragePhase::Policy, Store::File, Outcome::NotFound) => {
                "secure_entry_missing"
            }
            _ => "none",
        };
        let tags = &[
            ("credential_kind", self.credential_kind.as_ref()),
            ("store_mode", self.mode.as_ref()),
            ("selected_store", self.selected_store.as_ref()),
            ("actual_store", self.actual_store.as_ref()),
            ("operation", self.operation.as_ref()),
            ("secure_outcome", self.secure_outcome.as_ref()),
            ("outcome", self.outcome.as_ref()),
            ("fallback_reason", fallback),
            ("secure_error", self.secure_error),
            ("storage_phase", self.phase.as_ref()),
            (crate::ORIGINATOR_TAG, self.originator.as_str()),
        ];
        if fallback != "none" {
            tracing::warn!(
                credential_kind = self.credential_kind.as_ref(),
                store_mode = self.mode.as_ref(),
                selected_store = self.selected_store.as_ref(),
                actual_store = self.actual_store.as_ref(),
                operation = self.operation.as_ref(),
                secure_outcome = self.secure_outcome.as_ref(),
                outcome = self.outcome.as_ref(),
                fallback_reason = fallback,
                secure_error = self.secure_error,
                storage_phase = self.phase.as_ref(),
                originator = self.originator.as_str(),
                "credential storage file fallback completed"
            );
        }
        let (count, duration) = match self.operation {
            Operation::RefreshPersist => (
                "codex.auth_storage.refresh_persist",
                "codex.auth_storage.refresh_persist.duration",
            ),
            Operation::Load | Operation::Save | Operation::Delete | Operation::Cleanup => (
                "codex.auth_storage.operation",
                "codex.auth_storage.duration",
            ),
        };
        let _ = crate::record_global_operation(count, duration, self.duration, tags);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, AsRefStr)]
#[strum(serialize_all = "snake_case")]
pub enum StoragePhase {
    Policy,
    Pinned,
}

/// Records one logical storage operation, including the secure attempt and any file fallback.
/// Callers execute each attempt and pass its result by reference; error messages and payloads
/// are not read. The combined outcome and elapsed time are recorded when this guard is dropped.
/// Client attribution is supplied at construction and retained until emission.
#[derive(Debug)]
pub struct StorageTelemetry {
    observation: Observation,
    started: Instant,
}

impl StorageTelemetry {
    pub fn new(
        credential_kind: CredentialKind,
        mode: StoreMode,
        selected_store: Store,
        operation: Operation,
        originator: AuthStorageOriginator,
    ) -> Self {
        Self {
            observation: Observation {
                credential_kind,
                mode,
                selected_store,
                actual_store: selected_store,
                operation,
                secure_outcome: Outcome::NotAttempted,
                outcome: Outcome::NotAttempted,
                duration: Duration::ZERO,
                phase: StoragePhase::Policy,
                secure_error: "none",
                originator,
            },
            started: Instant::now(),
        }
    }

    /// Pinned operations retain their configured mode but never re-evaluate fallback policy.
    pub fn with_phase(mut self, phase: StoragePhase) -> Self {
        self.observation.phase = phase;
        self
    }

    /// Classifies typed causes only; never formats errors or parses their messages.
    /// May be called before or after recording the corresponding failed secure attempt.
    pub fn record_secure_error(&mut self, error: &(dyn Error + 'static)) {
        let mut cause = Some(error);
        while let Some(error) = cause {
            if let Some(error) = error.downcast_ref::<std::io::Error>() {
                let category = match error.kind() {
                    std::io::ErrorKind::PermissionDenied => "access_denied",
                    std::io::ErrorKind::NotConnected => "unavailable",
                    std::io::ErrorKind::WouldBlock => "locked",
                    std::io::ErrorKind::Unsupported => "unsupported",
                    std::io::ErrorKind::TimedOut => "timeout",
                    _ => "other",
                };
                if category != "other" {
                    self.observation.secure_error = category;
                    return;
                }
                cause = error.get_ref().map(|error| error as &dyn Error);
            } else {
                cause = error.source();
            }
        }
        self.observation.secure_error = "other";
    }

    pub fn record_load_attempt<T, E>(&mut self, store: Store, result: &Result<Option<T>, E>) {
        let outcome = match result {
            Ok(Some(_)) => Outcome::Success,
            Ok(None) => Outcome::NotFound,
            Err(_) => Outcome::Error,
        };
        self.record_attempt(store, outcome);
    }

    pub fn record_save_attempt<E>(&mut self, store: Store, result: &Result<(), E>) {
        let outcome = match result {
            Ok(()) => Outcome::Success,
            Err(_) => Outcome::Error,
        };
        self.record_attempt(store, outcome);
    }

    pub fn record_delete_attempt<E>(&mut self, store: Store, result: &Result<bool, E>) {
        let outcome = match result {
            Ok(true) => Outcome::Success,
            Ok(false) => Outcome::NotFound,
            Err(_) => Outcome::Error,
        };
        self.record_attempt(store, outcome);
    }

    fn record_attempt(&mut self, store: Store, outcome: Outcome) {
        self.observation.actual_store = store;
        self.observation.outcome = outcome;
        if matches!(store, Store::DirectKeyring | Store::Secrets) {
            self.observation.secure_outcome = outcome;
            if outcome != Outcome::Error {
                self.observation.secure_error = "none";
            } else if self.observation.secure_error == "none" {
                self.observation.secure_error = "other";
            }
        }
    }
}

impl Drop for StorageTelemetry {
    fn drop(&mut self) {
        if self.observation.outcome == Outcome::NotAttempted {
            return;
        }
        self.observation.duration = self.started.elapsed();
        self.observation.record();
    }
}

#[cfg(test)]
#[path = "auth_storage_tests.rs"]
mod tests;
