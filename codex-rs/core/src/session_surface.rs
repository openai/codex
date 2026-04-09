#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SessionSurfacePolicy {
    pub(crate) prompt: PromptContextInclusions,
    pub(crate) capabilities: CapabilityInclusions,
}

impl SessionSurfacePolicy {
    pub(crate) const fn full() -> Self {
        Self {
            prompt: PromptContextInclusions::full(),
            capabilities: CapabilityInclusions::full(),
        }
    }

    pub(crate) const fn guardian_review() -> Self {
        Self {
            prompt: PromptContextInclusions {
                model_update: false,
                permissions: false,
                developer_instructions: true,
                separate_developer_instructions: true,
                collaboration: false,
                realtime: false,
                personality: false,
                commit: false,
                user_instructions: true,
                environment_context: true,
            },
            capabilities: CapabilityInclusions::minimal_local(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PromptContextInclusions {
    pub(crate) model_update: bool,
    pub(crate) permissions: bool,
    pub(crate) developer_instructions: bool,
    pub(crate) separate_developer_instructions: bool,
    pub(crate) collaboration: bool,
    pub(crate) realtime: bool,
    pub(crate) personality: bool,
    pub(crate) commit: bool,
    pub(crate) user_instructions: bool,
    pub(crate) environment_context: bool,
}

impl PromptContextInclusions {
    pub(crate) const fn full() -> Self {
        Self {
            model_update: true,
            permissions: true,
            developer_instructions: true,
            separate_developer_instructions: false,
            collaboration: true,
            realtime: true,
            personality: true,
            commit: true,
            user_instructions: true,
            environment_context: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CapabilityInclusions {
    pub(crate) mcp: bool,
    pub(crate) apps: bool,
    pub(crate) plugins: bool,
    pub(crate) skills: bool,
    pub(crate) memory: bool,
    pub(crate) dynamic_tools: bool,
    pub(crate) web_search: bool,
    pub(crate) js_repl: bool,
    pub(crate) code_mode: bool,
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

impl CapabilityInclusions {
    pub(crate) const fn full() -> Self {
        Self {
            mcp: true,
            apps: true,
            plugins: true,
            skills: true,
            memory: true,
            dynamic_tools: true,
            web_search: true,
            js_repl: true,
            code_mode: true,
            apply_patch: true,
            subagents: true,
            image_generation: true,
            request_permissions: true,
            skill_dependencies: true,
            shell_snapshot: true,
            shell_zsh_fork: true,
            unified_exec: true,
            request_rule: true,
            git_commit: true,
        }
    }

    pub(crate) const fn minimal_local() -> Self {
        Self {
            mcp: false,
            apps: false,
            plugins: false,
            skills: false,
            memory: false,
            dynamic_tools: false,
            web_search: false,
            js_repl: false,
            code_mode: false,
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
