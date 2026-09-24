//! Bounded attribution for auth-storage observations, scoped to one task.
//! Spawned tasks must explicitly carry this context across the task boundary.

use std::future::Future;

tokio::task_local! {
    static CURRENT: AuthStorageOriginator;
}

/// A bounded client identity used only by credential-storage telemetry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthStorageOriginator(&'static str);

impl AuthStorageOriginator {
    pub fn from_client_name(name: &str) -> Self {
        let name = match name {
            "Codex Desktop" => "codex_desktop",
            _ => name,
        };
        Self(crate::bounded_originator_tag_value(name))
    }

    /// Unattributed work must not borrow another connection's identity.
    pub fn current() -> Self {
        CURRENT
            .try_with(|originator| *originator)
            .unwrap_or(Self("none"))
    }

    pub fn as_str(self) -> &'static str {
        self.0
    }

    pub async fn scope<F: Future>(self, future: F) -> F::Output {
        CURRENT.scope(self, future).await
    }

    pub fn sync_scope<R>(self, operation: impl FnOnce() -> R) -> R {
        CURRENT.sync_scope(self, operation)
    }
}

#[cfg(test)]
#[path = "originator_tests.rs"]
mod tests;
