use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use codex_core::config::Config;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolContributor;
use codex_features::Feature;
use codex_protocol::ThreadId;
use codex_state::AutomationDispatchSettings;

use crate::tool::AutomationUpdateTool;

#[derive(Clone, Debug)]
pub(crate) struct AutomationThreadContext {
    pub(crate) thread_id: ThreadId,
    pub(crate) cwd: PathBuf,
    pub(crate) workspace_roots: Vec<PathBuf>,
    pub(crate) dispatch_settings: AutomationDispatchSettings,
    pub(crate) enabled: bool,
    pub(crate) tools_available_for_thread: bool,
}

impl AutomationThreadContext {
    fn from_thread_start(input: &ThreadStartInput<'_, Config>) -> Option<Self> {
        let thread_id = ThreadId::from_string(input.thread_store.level_id()).ok()?;
        let enabled = input.config.features.enabled(Feature::Automations);
        let tools_available_for_thread = input.persistent_thread_state_available
            && !input.session_source.is_automation()
            && !input.session_source.is_internal();
        Some(Self::from_config(
            thread_id,
            input.config,
            enabled,
            tools_available_for_thread,
        ))
    }

    fn from_config(
        thread_id: ThreadId,
        config: &Config,
        enabled: bool,
        tools_available_for_thread: bool,
    ) -> Self {
        let cwd = config.cwd.clone().into_path_buf();
        let workspace_roots = config
            .effective_workspace_roots()
            .into_iter()
            .map(codex_utils_absolute_path::AbsolutePathBuf::into_path_buf)
            .collect::<Vec<_>>();
        let workspace_roots = if workspace_roots.is_empty() {
            vec![cwd.clone()]
        } else {
            workspace_roots
        };
        Self {
            thread_id,
            cwd,
            workspace_roots: workspace_roots.clone(),
            dispatch_settings: AutomationDispatchSettings {
                workspace_roots,
                approval_policy: *config.permissions.approval_policy.get(),
                approvals_reviewer: config.approvals_reviewer,
                permission_profile: config.permissions.effective_permission_profile(),
            },
            enabled,
            tools_available_for_thread,
        }
    }

    pub(crate) fn tools_visible(&self) -> bool {
        self.enabled && self.tools_available_for_thread
    }
}

#[derive(Clone)]
struct AutomationExtension {
    state_db: Arc<codex_state::StateRuntime>,
}

impl AutomationExtension {
    fn new(state_db: Arc<codex_state::StateRuntime>) -> Self {
        Self { state_db }
    }
}

impl std::fmt::Debug for AutomationExtension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutomationExtension")
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl ThreadLifecycleContributor<Config> for AutomationExtension {
    async fn on_thread_start(&self, input: ThreadStartInput<'_, Config>) {
        let Some(context) = AutomationThreadContext::from_thread_start(&input) else {
            return;
        };
        input.thread_store.insert(context);
    }
}

impl ConfigContributor<Config> for AutomationExtension {
    fn on_config_changed(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
        _previous_config: &Config,
        new_config: &Config,
    ) {
        let Some(existing) = thread_store.get::<AutomationThreadContext>() else {
            return;
        };
        thread_store.insert(AutomationThreadContext::from_config(
            existing.thread_id,
            new_config,
            new_config.features.enabled(Feature::Automations),
            existing.tools_available_for_thread,
        ));
    }
}

impl ToolContributor for AutomationExtension {
    fn tools(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn codex_extension_api::ToolExecutor<codex_extension_api::ToolCall>>> {
        let Some(context) = thread_store.get::<AutomationThreadContext>() else {
            return Vec::new();
        };
        if !context.tools_visible() {
            return Vec::new();
        }
        vec![Arc::new(AutomationUpdateTool::new(
            Arc::clone(&self.state_db),
            context.as_ref().clone(),
        ))]
    }
}

pub fn install(
    registry: &mut ExtensionRegistryBuilder<Config>,
    state_db: Arc<codex_state::StateRuntime>,
) {
    let extension = Arc::new(AutomationExtension::new(state_db));
    registry.thread_lifecycle_contributor(extension.clone());
    registry.config_contributor(extension.clone());
    registry.tool_contributor(extension);
}
