//! System prompt builder.
//!
//! Assembles the complete system prompt from templates and conversation context.

use cocode_context::ConversationContext;
use cocode_context::InjectionPosition;
use cocode_context::SubagentType;

use crate::sections::PromptSection;
use crate::sections::assemble_sections;
use crate::sections::permission_section;
use crate::sections::render_environment;
use crate::sections::render_injections;
use crate::sections::render_memory_files;
use crate::sections::{self};
use crate::summarization;
use crate::templates;

/// System prompt builder.
///
/// All methods are sync — pure string assembly with no I/O.
pub struct SystemPromptBuilder;

impl SystemPromptBuilder {
    /// Build the complete system prompt for a main agent.
    pub fn build(ctx: &ConversationContext) -> String {
        let mut ordered_sections = Vec::new();

        // 1. Identity
        ordered_sections.push((
            PromptSection::Identity,
            templates::BASE_IDENTITY.to_string(),
        ));

        // 2. Tool policy (if tools present)
        if ctx.has_tools() {
            let mut policy = templates::TOOL_POLICY.to_string();
            let tool_lines = sections::generate_tool_policy_lines(&ctx.tool_names);
            if !tool_lines.is_empty() {
                policy.push('\n');
                policy.push_str(&tool_lines);
            }
            ordered_sections.push((PromptSection::ToolPolicy, policy));
        }

        // 3. Security
        ordered_sections.push((PromptSection::Security, templates::SECURITY.to_string()));

        // 4. Git workflow
        ordered_sections.push((
            PromptSection::GitWorkflow,
            templates::GIT_WORKFLOW.to_string(),
        ));

        // 5. Task management
        ordered_sections.push((
            PromptSection::TaskManagement,
            templates::TASK_MANAGEMENT.to_string(),
        ));

        // 6. MCP instructions (if MCP servers present)
        if ctx.has_mcp_servers() {
            ordered_sections.push((
                PromptSection::McpInstructions,
                templates::MCP_INSTRUCTIONS.to_string(),
            ));
        }

        // Before-tools injections
        let before_tools = render_injections(ctx, InjectionPosition::BeforeTools);
        if !before_tools.is_empty() {
            ordered_sections.push((PromptSection::Injections, before_tools));
        }

        // After-tools injections
        let after_tools = render_injections(ctx, InjectionPosition::AfterTools);
        if !after_tools.is_empty() {
            ordered_sections.push((PromptSection::Injections, after_tools));
        }

        // 7. Environment
        ordered_sections.push((PromptSection::Environment, render_environment(ctx)));

        // 8. Permission
        ordered_sections.push((
            PromptSection::Permission,
            permission_section(&ctx.permission_mode).to_string(),
        ));

        // 9. Memory files
        let memory = render_memory_files(ctx);
        if !memory.is_empty() {
            ordered_sections.push((PromptSection::MemoryFiles, memory));
        }

        // 10. End-of-prompt injections
        let end_injections = render_injections(ctx, InjectionPosition::EndOfPrompt);
        if !end_injections.is_empty() {
            ordered_sections.push((PromptSection::Injections, end_injections));
        }

        assemble_sections(&ordered_sections)
    }

    /// Build system prompt for a subagent (explore/plan).
    pub fn build_for_subagent(ctx: &ConversationContext, subagent_type: SubagentType) -> String {
        let subagent_template = match subagent_type {
            SubagentType::Explore => templates::EXPLORE_SUBAGENT,
            SubagentType::Plan => templates::PLAN_SUBAGENT,
        };

        let mut sections = vec![
            (PromptSection::Identity, subagent_template.to_string()),
            (PromptSection::Security, templates::SECURITY.to_string()),
            (
                PromptSection::Environment,
                sections::render_environment(ctx),
            ),
        ];

        // Include memory files for subagents too
        let memory = render_memory_files(ctx);
        if !memory.is_empty() {
            sections.push((PromptSection::MemoryFiles, memory));
        }

        assemble_sections(&sections)
    }

    /// Build summarization prompts for context compaction.
    ///
    /// Returns `(system_prompt, user_prompt)`.
    pub fn build_summarization(
        conversation_text: &str,
        custom_instructions: Option<&str>,
    ) -> (String, String) {
        summarization::build_summarization_prompt(conversation_text, custom_instructions)
    }

    /// Build brief summarization prompts for micro-compaction.
    ///
    /// Returns `(system_prompt, user_prompt)`.
    pub fn build_brief_summarization(conversation_text: &str) -> (String, String) {
        summarization::build_brief_summary_prompt(conversation_text)
    }
}

#[cfg(test)]
mod tests {
    use cocode_context::ContextInjection;
    use cocode_context::EnvironmentInfo;
    use cocode_context::InjectionPosition;
    use cocode_context::MemoryFile;
    use cocode_protocol::PermissionMode;

    use super::*;

    fn test_env() -> EnvironmentInfo {
        EnvironmentInfo::builder()
            .platform("darwin")
            .os_version("Darwin 24.0.0")
            .cwd("/home/user/project")
            .is_git_repo(true)
            .git_branch("main")
            .date("2025-01-29")
            .model("claude-3-opus")
            .context_window(200000)
            .output_token_limit(16384)
            .build()
            .unwrap()
    }

    #[test]
    fn test_build_minimal() {
        let ctx = ConversationContext::builder()
            .environment(test_env())
            .build()
            .unwrap();

        let prompt = SystemPromptBuilder::build(&ctx);

        // Should contain identity and environment
        assert!(prompt.contains("Identity"));
        assert!(prompt.contains("darwin"));
        assert!(prompt.contains("2025-01-29"));
        // Should NOT contain tool policy (no tools)
        assert!(!prompt.contains("Tool Usage Policy"));
        // Should NOT contain MCP instructions (no MCP servers)
        assert!(!prompt.contains("MCP Server Instructions"));
    }

    #[test]
    fn test_build_with_tools() {
        let ctx = ConversationContext::builder()
            .environment(test_env())
            .tool_names(vec!["Read".to_string(), "Write".to_string()])
            .build()
            .unwrap();

        let prompt = SystemPromptBuilder::build(&ctx);
        assert!(prompt.contains("Tool Usage Policy"));
    }

    #[test]
    fn test_build_with_mcp() {
        let ctx = ConversationContext::builder()
            .environment(test_env())
            .mcp_server_names(vec!["github".to_string()])
            .build()
            .unwrap();

        let prompt = SystemPromptBuilder::build(&ctx);
        assert!(prompt.contains("MCP Server Instructions"));
    }

    #[test]
    fn test_build_with_memory_files() {
        let ctx = ConversationContext::builder()
            .environment(test_env())
            .memory_files(vec![MemoryFile {
                path: "CLAUDE.md".to_string(),
                content: "Use Rust conventions".to_string(),
                priority: 0,
            }])
            .build()
            .unwrap();

        let prompt = SystemPromptBuilder::build(&ctx);
        assert!(prompt.contains("CLAUDE.md"));
        assert!(prompt.contains("Use Rust conventions"));
    }

    #[test]
    fn test_build_permission_modes() {
        for mode in &[
            PermissionMode::Default,
            PermissionMode::Plan,
            PermissionMode::AcceptEdits,
            PermissionMode::Bypass,
        ] {
            let ctx = ConversationContext::builder()
                .environment(test_env())
                .permission_mode(*mode)
                .build()
                .unwrap();

            let prompt = SystemPromptBuilder::build(&ctx);
            assert!(prompt.contains("Permission Mode"));
        }
    }

    #[test]
    fn test_build_with_injections() {
        let ctx = ConversationContext::builder()
            .environment(test_env())
            .injections(vec![ContextInjection {
                label: "custom-hook".to_string(),
                content: "Hook output here".to_string(),
                position: InjectionPosition::EndOfPrompt,
            }])
            .build()
            .unwrap();

        let prompt = SystemPromptBuilder::build(&ctx);
        assert!(prompt.contains("Hook output here"));
    }

    #[test]
    fn test_build_for_subagent_explore() {
        let ctx = ConversationContext::builder()
            .environment(test_env())
            .build()
            .unwrap();

        let prompt = SystemPromptBuilder::build_for_subagent(&ctx, SubagentType::Explore);
        assert!(prompt.contains("Explore Subagent"));
        assert!(prompt.contains("darwin"));
        assert!(!prompt.contains("Plan Subagent"));
    }

    #[test]
    fn test_build_for_subagent_plan() {
        let ctx = ConversationContext::builder()
            .environment(test_env())
            .build()
            .unwrap();

        let prompt = SystemPromptBuilder::build_for_subagent(&ctx, SubagentType::Plan);
        assert!(prompt.contains("Plan Subagent"));
        assert!(!prompt.contains("Explore Subagent"));
    }

    #[test]
    fn test_build_summarization() {
        let (system, user) = SystemPromptBuilder::build_summarization("conversation content", None);
        assert!(!system.is_empty());
        assert!(user.contains("conversation content"));
    }

    #[test]
    fn test_build_brief_summarization() {
        let (system, user) = SystemPromptBuilder::build_brief_summarization("brief content");
        assert!(!system.is_empty());
        assert!(user.contains("brief content"));
    }

    #[test]
    fn test_build_with_tools_includes_dynamic_policy() {
        let ctx = ConversationContext::builder()
            .environment(test_env())
            .tool_names(vec![
                "Read".to_string(),
                "Edit".to_string(),
                "LS".to_string(),
            ])
            .build()
            .unwrap();

        let prompt = SystemPromptBuilder::build(&ctx);
        assert!(prompt.contains("Use Read for reading files"));
        assert!(prompt.contains("Use Edit for modifying files"));
        assert!(prompt.contains("Use LS for directory listing"));
        assert!(!prompt.contains("Use Grep"));
    }

    #[test]
    fn test_build_with_tools_excludes_ls_when_not_registered() {
        let ctx = ConversationContext::builder()
            .environment(test_env())
            .tool_names(vec!["Read".to_string(), "Edit".to_string()])
            .build()
            .unwrap();

        let prompt = SystemPromptBuilder::build(&ctx);
        assert!(prompt.contains("Use Read for reading files"));
        assert!(prompt.contains("Use Edit for modifying files"));
        assert!(!prompt.contains("Use LS for directory listing"));
    }

    #[test]
    fn test_section_ordering() {
        let ctx = ConversationContext::builder()
            .environment(test_env())
            .tool_names(vec!["Read".to_string()])
            .mcp_server_names(vec!["github".to_string()])
            .memory_files(vec![MemoryFile {
                path: "CLAUDE.md".to_string(),
                content: "rules".to_string(),
                priority: 0,
            }])
            .build()
            .unwrap();

        let prompt = SystemPromptBuilder::build(&ctx);

        // Verify ordering: Identity before ToolPolicy before Security before Environment
        let identity_pos = prompt.find("# Identity").unwrap();
        let tool_pos = prompt.find("# Tool Usage Policy").unwrap();
        let security_pos = prompt.find("# Security Guidelines").unwrap();
        let env_pos = prompt.find("# Environment").unwrap();
        let permission_pos = prompt.find("# Permission Mode").unwrap();
        let memory_pos = prompt.find("# Memory Files").unwrap();

        assert!(identity_pos < tool_pos);
        assert!(tool_pos < security_pos);
        assert!(security_pos < env_pos);
        assert!(env_pos < permission_pos);
        assert!(permission_pos < memory_pos);
    }
}
