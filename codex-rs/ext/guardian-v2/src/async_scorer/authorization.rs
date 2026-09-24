//! Binds cached classifier results to the user authorization and model policy they evaluated.

use codex_core::CodexThread;
use codex_core::GuardianAuthorizationVersion;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ScoreAuthorization {
    pub(super) permissions: codex_guardian_context::PermissionContext,
    pub(super) settings: codex_protocol::protocol::ThreadSettingsSnapshot,
    pub(super) environments: Vec<codex_protocol::protocol::TurnEnvironmentSelection>,
    pub(super) local: GuardianAuthorizationVersion,
    pub(super) root: Option<GuardianAuthorizationVersion>,
    pub(super) model: Option<std::sync::Arc<codex_protocol::openai_models::ModelInfo>>,
}

impl ScoreAuthorization {
    pub(super) async fn current(
        thread: &CodexThread,
        permissions: &codex_guardian_context::PermissionContext,
    ) -> Self {
        let root = thread
            .guardian_root_snapshot()
            .await
            .map(|snapshot| snapshot.authorization_version);
        Self {
            permissions: permissions.clone(),
            settings: thread.thread_settings_snapshot().await,
            environments: thread
                .config_snapshot()
                .await
                .environment_selections()
                .to_vec(),
            local: thread.guardian_authorization_version().await,
            root,
            model: thread
                .thread_extension_data()
                .get::<codex_protocol::openai_models::ModelInfo>(),
        }
    }
}
