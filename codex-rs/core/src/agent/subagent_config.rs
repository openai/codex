use anyhow::Context;
use codex_features::Feature;

use crate::codex::TurnContext;
use crate::config::Config;
use crate::config::Constrained;
use crate::config::InitialContextInclusions;

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(crate) enum SubAgentPromptInheritance {
    InheritParent,
    None,
    Select(SubAgentPromptSelection),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SubAgentPromptSelection {
    pub(crate) model_update: bool,
    pub(crate) base_instructions: bool,
    pub(crate) user_instructions: bool,
    pub(crate) project_docs: bool,
    pub(crate) developer_instructions: bool,
    pub(crate) separate_developer_instructions: bool,
    pub(crate) compact_prompt: bool,
    pub(crate) permissions: bool,
    pub(crate) memory: bool,
    pub(crate) collaboration: bool,
    pub(crate) realtime: bool,
    pub(crate) personality: bool,
    pub(crate) apps: bool,
    pub(crate) skills: bool,
    pub(crate) plugins: bool,
    pub(crate) commit: bool,
    pub(crate) environment_context: bool,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(crate) enum SubAgentExtensionInheritance {
    InheritParent,
    None,
    Select(SubAgentExtensionSelection),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SubAgentExtensionSelection {
    pub(crate) mcp_servers: bool,
    pub(crate) apps: bool,
    pub(crate) plugins: bool,
    pub(crate) tool_search: bool,
    pub(crate) web_search: bool,
    pub(crate) js_repl: bool,
    pub(crate) code_mode: bool,
    pub(crate) memory: bool,
    pub(crate) apply_patch: bool,
    pub(crate) subagents: bool,
    pub(crate) image_generation: bool,
    pub(crate) request_permissions: bool,
    pub(crate) skill_dependencies: bool,
    pub(crate) shell_snapshot: bool,
    pub(crate) shell_zsh_fork: bool,
    pub(crate) unified_exec: bool,
    pub(crate) request_rule: bool,
    pub(crate) git_commit: bool,
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
                self.config.initial_context_inclusions = InitialContextInclusions::none();
            }
            SubAgentPromptInheritance::Select(selection) => {
                self.apply_prompt_selection(selection);
            }
        }
        self
    }

    pub(crate) fn extension_inheritance(
        mut self,
        inheritance: SubAgentExtensionInheritance,
    ) -> anyhow::Result<Self> {
        match inheritance {
            SubAgentExtensionInheritance::InheritParent => {}
            SubAgentExtensionInheritance::None => {
                self.apply_extension_selection(SubAgentExtensionSelection::none())?;
            }
            SubAgentExtensionInheritance::Select(selection) => {
                self.apply_extension_selection(selection)?;
            }
        }
        Ok(self)
    }

    pub(crate) fn initial_context_inclusions(
        mut self,
        inclusions: InitialContextInclusions,
    ) -> Self {
        self.config.initial_context_inclusions = inclusions;
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
        self.config.initial_context_inclusions = InitialContextInclusions {
            model_update: selection.model_update,
            permissions: selection.permissions,
            developer_instructions: selection.developer_instructions,
            separate_developer_instructions: selection.separate_developer_instructions,
            memory: selection.memory,
            collaboration: selection.collaboration,
            realtime: selection.realtime,
            personality: selection.personality,
            apps: selection.apps,
            skills: selection.skills,
            plugins: selection.plugins,
            commit: selection.commit,
            user_instructions: selection.user_instructions || selection.project_docs,
            environment_context: selection.environment_context,
        };
    }

    fn apply_extension_selection(
        &mut self,
        selection: SubAgentExtensionSelection,
    ) -> anyhow::Result<()> {
        if !selection.mcp_servers {
            self.config.mcp_servers = Constrained::allow_only(Default::default());
        }
        if !selection.apps {
            self.disable_feature(Feature::Apps)?;
        }
        if !selection.plugins {
            self.disable_feature(Feature::Plugins)?;
        }
        if !selection.tool_search {
            self.disable_feature(Feature::ToolSearch)?;
            self.disable_feature(Feature::ToolSuggest)?;
        }
        if !selection.web_search {
            self.disable_feature(Feature::WebSearchRequest)?;
            self.disable_feature(Feature::WebSearchCached)?;
        }
        if !selection.js_repl {
            self.config.js_repl_node_path = None;
            self.config.js_repl_node_module_dirs = Vec::new();
            self.disable_feature(Feature::JsReplToolsOnly)?;
            self.disable_feature(Feature::JsRepl)?;
        }
        if !selection.code_mode {
            self.disable_feature(Feature::CodeModeOnly)?;
            self.disable_feature(Feature::CodeMode)?;
        }
        if !selection.memory {
            self.disable_feature(Feature::MemoryTool)?;
        }
        if !selection.apply_patch {
            self.config.include_apply_patch_tool = false;
            self.disable_feature(Feature::ApplyPatchFreeform)?;
        }
        if !selection.subagents {
            self.disable_feature(Feature::SpawnCsv)?;
            self.disable_feature(Feature::MultiAgentV2)?;
            self.disable_feature(Feature::Collab)?;
            self.disable_feature(Feature::ChildAgentsMd)?;
        }
        if !selection.image_generation {
            self.disable_feature(Feature::ImageGeneration)?;
        }
        if !selection.request_permissions {
            self.disable_feature(Feature::RequestPermissionsTool)?;
            self.disable_feature(Feature::ExecPermissionApprovals)?;
        }
        if !selection.skill_dependencies {
            self.disable_feature(Feature::SkillMcpDependencyInstall)?;
            self.disable_feature(Feature::SkillEnvVarDependencyPrompt)?;
        }
        if !selection.shell_snapshot {
            self.disable_feature(Feature::ShellSnapshot)?;
        }
        if !selection.shell_zsh_fork {
            self.config.zsh_path = None;
            self.disable_feature(Feature::ShellZshFork)?;
        }
        if !selection.unified_exec {
            self.config.use_experimental_unified_exec_tool = false;
            self.config.main_execve_wrapper_exe = None;
            self.disable_feature(Feature::UnifiedExec)?;
        }
        if !selection.request_rule {
            self.disable_feature(Feature::RequestRule)?;
        }
        if !selection.git_commit {
            self.disable_feature(Feature::CodexGitCommit)?;
        }

        Ok(())
    }

    fn disable_feature(&mut self, feature: Feature) -> anyhow::Result<()> {
        self.config.features.disable(feature).map_err(|err| {
            anyhow::anyhow!(
                "subagent config could not disable `features.{}`: {err}",
                feature.key()
            )
        })?;
        if self.config.features.enabled(feature) {
            anyhow::bail!(
                "subagent config requires `features.{}` to be disabled",
                feature.key()
            );
        }
        Ok(())
    }
}

impl SubAgentExtensionSelection {
    const fn none() -> Self {
        Self {
            mcp_servers: false,
            apps: false,
            plugins: false,
            tool_search: false,
            web_search: false,
            js_repl: false,
            code_mode: false,
            memory: false,
            apply_patch: false,
            subagents: false,
            image_generation: false,
            request_permissions: false,
            skill_dependencies: false,
            shell_snapshot: false,
            shell_zsh_fork: false,
            unified_exec: false,
            request_rule: false,
            git_commit: false,
        }
    }
}
