use std::sync::Arc;

use crate::environment_selection::TurnEnvironmentSnapshot;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use codex_tools::ToolEnvironmentMode;
use codex_utils_absolute_path::AbsolutePathBuf;

/// Immutable environment state shared by model context, tools, and tool calls.
/// Persisting and diffing starting/attached state remain deferred to a dedicated step baseline.
#[derive(Debug)]
pub(crate) struct StepContext {
    pub(crate) environments: TurnEnvironmentSnapshot,
}

impl StepContext {
    pub(crate) fn new(environments: TurnEnvironmentSnapshot) -> Self {
        Self { environments }
    }

    pub(crate) fn tool_environment_mode(&self) -> ToolEnvironmentMode {
        ToolEnvironmentMode::from_count(self.environments.turn_environments.len())
    }

    pub(crate) fn effective_cwd(&self, turn: &TurnContext) -> AbsolutePathBuf {
        self.environments
            .primary()
            .map(super::turn_context::TurnEnvironment::cwd)
            .or_else(|| {
                self.environments
                    .starting
                    .first()
                    .map(|environment| &environment.selection.cwd)
            })
            .and_then(|cwd| cwd.to_abs_path().ok())
            .unwrap_or_else(|| {
                #[allow(deprecated)]
                turn.cwd.clone()
            })
    }

    #[cfg(test)]
    pub(crate) fn local_for_test(turn: &TurnContext) -> Self {
        #[allow(deprecated)]
        let cwd = codex_utils_path_uri::PathUri::from_abs_path(&turn.cwd);
        let environment = Arc::new(
            codex_exec_server::Environment::create_for_tests(/*exec_server_url*/ None)
                .expect("create local test environment"),
        );
        Self::new(TurnEnvironmentSnapshot::from_turn_environments(vec![
            crate::session::turn_context::TurnEnvironment::new(
                codex_exec_server::LOCAL_ENVIRONMENT_ID.to_string(),
                environment,
                cwd,
                /*shell*/ None,
            ),
        ]))
    }
}

impl Session {
    pub(crate) async fn prepare_step_for_request(&self) -> Arc<StepContext> {
        Arc::new(StepContext::new(
            self.services.turn_environments.snapshot().await,
        ))
    }
}
