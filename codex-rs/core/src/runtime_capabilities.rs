use std::sync::Arc;

use codex_exec_server::Environment;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecutorFileSystem;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;

/// Describes whether a runtime may use ambient worker-local capabilities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeMode {
    LocalCodex,
    Isolated,
}

/// Ambient worker-local capabilities available to a Codex runtime.
///
/// V1 intentionally models one worker-local `Environment` capability. Local
/// filesystem and local exec access are derived views of that one capability
/// rather than independently configurable powers.
#[derive(Clone, Debug)]
pub struct RuntimeCapabilities {
    mode: RuntimeMode,
    local_environment: Option<Arc<Environment>>,
}

impl RuntimeCapabilities {
    /// Builds capabilities for a local Codex runtime.
    pub fn local(environment_manager: &EnvironmentManager) -> Self {
        Self {
            mode: RuntimeMode::LocalCodex,
            local_environment: Some(environment_manager.require_local_environment()),
        }
    }

    /// Builds capabilities for a runtime isolated from worker-local powers.
    pub fn isolated() -> Self {
        Self {
            mode: RuntimeMode::Isolated,
            local_environment: None,
        }
    }

    /// Returns whether this runtime is local Codex or isolated.
    pub fn mode(&self) -> RuntimeMode {
        self.mode
    }

    /// Returns the ambient worker-local environment when available.
    pub fn local_environment(&self) -> Option<Arc<Environment>> {
        self.local_environment.as_ref().map(Arc::clone)
    }

    /// Returns the ambient worker-local environment or an unsupported error.
    pub fn require_local_environment(&self, operation: &str) -> CodexResult<Arc<Environment>> {
        self.local_environment()
            .ok_or_else(|| missing_local_capability(operation, "environment"))
    }

    /// Returns the ambient worker-local filesystem when available.
    pub fn local_filesystem(&self) -> Option<Arc<dyn ExecutorFileSystem>> {
        self.local_environment()
            .map(|environment| environment.get_filesystem())
    }

    /// Returns the ambient worker-local filesystem or an unsupported error.
    pub fn require_local_filesystem(
        &self,
        operation: &str,
    ) -> CodexResult<Arc<dyn ExecutorFileSystem>> {
        self.local_filesystem()
            .ok_or_else(|| missing_local_capability(operation, "filesystem"))
    }

    /// Returns whether ambient worker-local exec is available.
    pub fn local_exec_available(&self) -> bool {
        self.local_environment.is_some()
    }

    /// Returns the ambient worker-local exec environment when available.
    pub fn local_exec(&self) -> Option<Arc<Environment>> {
        self.local_environment()
    }

    /// Returns the ambient worker-local exec environment or an unsupported error.
    pub fn require_local_exec(&self, operation: &str) -> CodexResult<Arc<Environment>> {
        self.local_exec()
            .ok_or_else(|| missing_local_capability(operation, "exec"))
    }
}

fn missing_local_capability(operation: &str, capability: &str) -> CodexErr {
    CodexErr::UnsupportedOperation(format!(
        "{operation} requires ambient worker-local {capability}"
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use codex_exec_server::EnvironmentManager;
    use codex_protocol::error::CodexErr;
    use codex_protocol::error::Result as CodexResult;
    use pretty_assertions::assert_eq;

    use super::RuntimeCapabilities;
    use super::RuntimeMode;

    #[tokio::test]
    async fn local_capabilities_derive_worker_local_views_from_environment() {
        let environment_manager = EnvironmentManager::default_for_tests();
        let expected_environment = environment_manager.require_local_environment();
        let expected_filesystem = expected_environment.get_filesystem();
        let capabilities = RuntimeCapabilities::local(&environment_manager);

        assert_eq!(capabilities.mode(), RuntimeMode::LocalCodex);
        assert!(Arc::ptr_eq(
            &capabilities.local_environment().expect("local environment"),
            &expected_environment,
        ));
        assert!(Arc::ptr_eq(
            &capabilities
                .require_local_environment("test operation")
                .expect("required local environment"),
            &expected_environment,
        ));
        assert!(Arc::ptr_eq(
            &capabilities.local_filesystem().expect("local filesystem"),
            &expected_filesystem,
        ));
        assert!(Arc::ptr_eq(
            &capabilities
                .require_local_filesystem("test operation")
                .expect("required local filesystem"),
            &expected_filesystem,
        ));
        assert!(capabilities.local_exec_available());
        assert!(Arc::ptr_eq(
            &capabilities.local_exec().expect("local exec"),
            &expected_environment,
        ));
        assert!(Arc::ptr_eq(
            &capabilities
                .require_local_exec("test operation")
                .expect("local exec"),
            &expected_environment,
        ));
    }

    #[test]
    fn isolated_capabilities_do_not_expose_worker_local_views() {
        let capabilities = RuntimeCapabilities::isolated();

        assert_eq!(capabilities.mode(), RuntimeMode::Isolated);
        assert!(capabilities.local_environment().is_none());
        assert!(capabilities.local_filesystem().is_none());
        assert!(!capabilities.local_exec_available());
        assert!(capabilities.local_exec().is_none());
        assert_eq!(
            unsupported_message(capabilities.require_local_environment("test operation")),
            "test operation requires ambient worker-local environment",
        );
        assert_eq!(
            unsupported_message(capabilities.require_local_filesystem("test operation")),
            "test operation requires ambient worker-local filesystem",
        );
        assert_eq!(
            unsupported_message(capabilities.require_local_exec("test operation")),
            "test operation requires ambient worker-local exec",
        );
    }

    fn unsupported_message<T>(result: CodexResult<T>) -> String {
        match result {
            Ok(_) => panic!("expected unsupported operation"),
            Err(CodexErr::UnsupportedOperation(message)) => message,
            Err(err) => panic!("expected unsupported operation, got {err}"),
        }
    }
}
