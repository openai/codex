use anyhow::Context;

use crate::codex::TurnContext;
use crate::config::Config;

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(crate) enum SubAgentPromptInheritance {
    InheritParent,
    None,
    Select(SubAgentPromptSelection),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SubAgentPromptSelection {
    pub(crate) base_instructions: bool,
    pub(crate) user_instructions: bool,
    pub(crate) project_docs: bool,
    pub(crate) developer_instructions: bool,
    pub(crate) compact_prompt: bool,
    pub(crate) permissions: bool,
    pub(crate) apps: bool,
    pub(crate) environment_context: bool,
}

pub(crate) struct SubAgentConfigBuilder {
    config: Config,
}

impl SubAgentConfigBuilder {
    pub(crate) fn from_parent_config(parent_config: &Config) -> Self {
        Self {
            config: parent_config.clone(),
        }
    }

    pub(crate) fn from_parent_turn(turn: &TurnContext) -> anyhow::Result<Self> {
        let base_config = turn.config.clone();
        let mut builder = Self::from_parent_config(base_config.as_ref());
        builder.config.model = Some(turn.model_info.slug.clone());
        builder.config.model_provider = turn.provider.clone();
        builder.config.model_reasoning_effort = turn.reasoning_effort;
        builder.config.model_reasoning_summary = Some(turn.reasoning_summary);
        builder.config.developer_instructions = turn.developer_instructions.clone();
        builder.config.compact_prompt = turn.compact_prompt.clone();
        Self::apply_turn_runtime_to_config(&mut builder.config, turn)?;
        Ok(builder)
    }

    pub(crate) fn apply_turn_runtime_to_config(
        config: &mut Config,
        turn: &TurnContext,
    ) -> anyhow::Result<()> {
        config
            .permissions
            .approval_policy
            .set(turn.approval_policy.value())
            .context("approval_policy is invalid")?;
        config.permissions.shell_environment_policy = turn.shell_environment_policy.clone();
        config.codex_linux_sandbox_exe = turn.codex_linux_sandbox_exe.clone();
        config.cwd = turn.cwd.clone();
        config
            .permissions
            .sandbox_policy
            .set(turn.sandbox_policy.get().clone())
            .context("sandbox_policy is invalid")?;
        config.permissions.file_system_sandbox_policy = turn.file_system_sandbox_policy.clone();
        config.permissions.network_sandbox_policy = turn.network_sandbox_policy;
        Ok(())
    }

    pub(crate) fn prompt_inheritance(mut self, inheritance: SubAgentPromptInheritance) -> Self {
        match inheritance {
            SubAgentPromptInheritance::InheritParent => {}
            SubAgentPromptInheritance::None => {
                self.config.base_instructions = None;
                self.config.user_instructions = None;
                self.config.developer_instructions = None;
                self.config.compact_prompt = None;
                self.config.project_doc_max_bytes = 0;
                self.config.project_doc_fallback_filenames.clear();
                self.config.include_permissions_instructions = false;
                self.config.include_apps_instructions = false;
                self.config.include_environment_context = false;
            }
            SubAgentPromptInheritance::Select(selection) => {
                self.apply_prompt_selection(selection);
            }
        }
        self
    }

    pub(crate) fn build(self) -> Config {
        self.config
    }

    fn apply_prompt_selection(&mut self, selection: SubAgentPromptSelection) {
        if !selection.base_instructions {
            self.config.base_instructions = None;
        }
        if !selection.user_instructions {
            self.config.user_instructions = None;
        }
        if !selection.developer_instructions {
            self.config.developer_instructions = None;
        }
        if !selection.compact_prompt {
            self.config.compact_prompt = None;
        }
        if !selection.project_docs {
            self.config.project_doc_max_bytes = 0;
            self.config.project_doc_fallback_filenames.clear();
        }

        self.config.include_permissions_instructions = selection.permissions;
        self.config.include_apps_instructions = selection.apps;
        self.config.include_environment_context = selection.environment_context;
    }
}
