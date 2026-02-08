//! Prompt section assembly.
//!
//! Defines the ordered sections of a system prompt and provides
//! template rendering and assembly functions.

use cocode_context::ConversationContext;
use cocode_context::InjectionPosition;
use cocode_protocol::PermissionMode;

use crate::templates;

/// Logical sections of the system prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromptSection {
    /// Agent identity and capabilities.
    Identity,
    /// Tool usage rules.
    ToolPolicy,
    /// Security guidelines.
    Security,
    /// Git workflow rules.
    GitWorkflow,
    /// Task management approach.
    TaskManagement,
    /// MCP server instructions.
    McpInstructions,
    /// Runtime environment info.
    Environment,
    /// Permission mode rules.
    Permission,
    /// Memory file contents.
    MemoryFiles,
    /// Injected content.
    Injections,
}

/// Assemble ordered sections into a single prompt string.
///
/// Sections are joined with double newlines. Empty sections are skipped.
pub fn assemble_sections(sections: &[(PromptSection, String)]) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for (_, content) in sections {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed);
        }
    }
    parts.join("\n\n")
}

/// Render the environment template with values from the conversation context.
pub fn render_environment(ctx: &ConversationContext) -> String {
    let git_branch = ctx.environment.git_branch.as_deref().unwrap_or("(none)");

    let mut env = templates::ENVIRONMENT_TEMPLATE
        .replace("{{platform}}", &ctx.environment.platform)
        .replace("{{os_version}}", &ctx.environment.os_version)
        .replace("{{cwd}}", &ctx.environment.cwd.display().to_string())
        .replace("{{is_git_repo}}", &ctx.environment.is_git_repo.to_string())
        .replace("{{git_branch}}", git_branch)
        .replace("{{date}}", &ctx.environment.date)
        .replace("{{model}}", &ctx.environment.model);

    // Append language preference if set
    if let Some(ref lang) = ctx.environment.language_preference {
        env.push_str(&format!("\n# Language Preference\n\nYou MUST respond in {}. All your responses, explanations, and communications should be in this language unless the user explicitly requests otherwise.\n", lang));
    }

    env
}

/// Get the permission section text for the given mode.
pub fn permission_section(mode: &PermissionMode) -> &'static str {
    match mode {
        PermissionMode::Default => templates::PERMISSION_DEFAULT,
        PermissionMode::Plan => templates::PERMISSION_PLAN,
        PermissionMode::AcceptEdits => templates::PERMISSION_ACCEPT_EDITS,
        PermissionMode::Bypass => templates::PERMISSION_BYPASS,
        PermissionMode::DontAsk => templates::PERMISSION_DEFAULT,
    }
}

/// Render memory files as a prompt section.
pub fn render_memory_files(ctx: &ConversationContext) -> String {
    if ctx.memory_files.is_empty() {
        return String::new();
    }

    let mut parts = vec!["# Memory Files".to_string()];
    let mut sorted_files = ctx.memory_files.clone();
    sorted_files.sort_by_key(|f| f.priority);

    for file in &sorted_files {
        parts.push(format!("## {}\n\n{}", file.path, file.content.trim()));
    }

    parts.join("\n\n")
}

/// Generate tool-specific policy lines based on available tool names.
///
/// Only includes policy lines for tools that are actually registered,
/// avoiding wasted tokens for disabled tools.
pub fn generate_tool_policy_lines(tool_names: &[String]) -> String {
    let tools: std::collections::HashSet<&str> = tool_names.iter().map(|s| s.as_str()).collect();
    let mut lines = Vec::new();
    if tools.contains("Read") {
        lines.push("- Use Read for reading files (not cat/head/tail)");
    }
    if tools.contains("Edit") {
        lines.push("- Use Edit for modifying files (not sed/awk)");
    }
    if tools.contains("Write") {
        lines.push("- Use Write for creating files (not echo/heredoc)");
    }
    if tools.contains("Grep") {
        lines.push("- Use Grep for searching file contents (not grep/rg)");
    }
    if tools.contains("Glob") {
        lines.push("- Use Glob for finding files by pattern (not find/ls)");
    }
    if tools.contains("LS") {
        lines.push("- Use LS for directory listing (not Bash ls)");
    }
    if lines.is_empty() {
        String::new()
    } else {
        lines.join("\n")
    }
}

/// Render injections for a specific position.
pub fn render_injections(ctx: &ConversationContext, position: InjectionPosition) -> String {
    let matching: Vec<&_> = ctx
        .injections
        .iter()
        .filter(|i| i.position == position)
        .collect();

    if matching.is_empty() {
        return String::new();
    }

    matching
        .iter()
        .map(|i| format!("<!-- {} -->\n{}", i.label, i.content.trim()))
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use cocode_context::ContextInjection;
    use cocode_context::EnvironmentInfo;
    use cocode_context::MemoryFile;

    use super::*;

    fn test_ctx() -> ConversationContext {
        let env = EnvironmentInfo::builder()
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
            .unwrap();

        ConversationContext::builder()
            .environment(env)
            .build()
            .unwrap()
    }

    #[test]
    fn test_assemble_sections_order() {
        let sections = vec![
            (PromptSection::Identity, "First section".to_string()),
            (PromptSection::Security, "Second section".to_string()),
            (PromptSection::Environment, "Third section".to_string()),
        ];

        let result = assemble_sections(&sections);
        assert!(result.starts_with("First section"));
        assert!(result.contains("Second section"));
        assert!(result.ends_with("Third section"));
    }

    #[test]
    fn test_assemble_sections_skips_empty() {
        let sections = vec![
            (PromptSection::Identity, "Content".to_string()),
            (PromptSection::Security, "".to_string()),
            (PromptSection::Environment, "   ".to_string()),
            (PromptSection::Permission, "More content".to_string()),
        ];

        let result = assemble_sections(&sections);
        assert_eq!(result, "Content\n\nMore content");
    }

    #[test]
    fn test_render_environment() {
        let ctx = test_ctx();
        let rendered = render_environment(&ctx);

        assert!(rendered.contains("darwin"));
        assert!(rendered.contains("/home/user/project"));
        assert!(rendered.contains("2025-01-29"));
        assert!(rendered.contains("claude-3-opus"));
        assert!(rendered.contains("main"));
        assert!(rendered.contains("true"));
        assert!(!rendered.contains("{{"));
    }

    #[test]
    fn test_permission_section() {
        assert!(permission_section(&PermissionMode::Default).contains("Default"));
        assert!(permission_section(&PermissionMode::Plan).contains("Plan"));
        assert!(permission_section(&PermissionMode::AcceptEdits).contains("Accept Edits"));
        assert!(permission_section(&PermissionMode::Bypass).contains("Bypass"));
    }

    #[test]
    fn test_render_memory_files() {
        let env = EnvironmentInfo::builder()
            .cwd("/tmp")
            .model("test")
            .build()
            .unwrap();

        let ctx = ConversationContext::builder()
            .environment(env)
            .memory_files(vec![
                MemoryFile {
                    path: "CLAUDE.md".to_string(),
                    content: "Project rules here".to_string(),
                    priority: 0,
                },
                MemoryFile {
                    path: "README.md".to_string(),
                    content: "Readme content".to_string(),
                    priority: 1,
                },
            ])
            .build()
            .unwrap();

        let rendered = render_memory_files(&ctx);
        assert!(rendered.contains("CLAUDE.md"));
        assert!(rendered.contains("Project rules here"));
        assert!(rendered.contains("README.md"));
        // CLAUDE.md should come first (lower priority value)
        let claude_pos = rendered.find("CLAUDE.md").unwrap();
        let readme_pos = rendered.find("README.md").unwrap();
        assert!(claude_pos < readme_pos);
    }

    #[test]
    fn test_render_memory_files_empty() {
        let ctx = test_ctx();
        let rendered = render_memory_files(&ctx);
        assert!(rendered.is_empty());
    }

    #[test]
    fn test_render_injections() {
        let env = EnvironmentInfo::builder()
            .cwd("/tmp")
            .model("test")
            .build()
            .unwrap();

        let ctx = ConversationContext::builder()
            .environment(env)
            .injections(vec![
                ContextInjection {
                    label: "hook-output".to_string(),
                    content: "Hook says hello".to_string(),
                    position: InjectionPosition::EndOfPrompt,
                },
                ContextInjection {
                    label: "pre-tool".to_string(),
                    content: "Before tools".to_string(),
                    position: InjectionPosition::BeforeTools,
                },
            ])
            .build()
            .unwrap();

        let end_injections = render_injections(&ctx, InjectionPosition::EndOfPrompt);
        assert!(end_injections.contains("Hook says hello"));
        assert!(!end_injections.contains("Before tools"));

        let before_injections = render_injections(&ctx, InjectionPosition::BeforeTools);
        assert!(before_injections.contains("Before tools"));
    }

    #[test]
    fn test_render_environment_without_language_preference() {
        let ctx = test_ctx();
        let rendered = render_environment(&ctx);

        // Should not contain language preference section
        assert!(!rendered.contains("# Language Preference"));
    }

    #[test]
    fn test_render_environment_with_language_preference() {
        let env = EnvironmentInfo::builder()
            .platform("darwin")
            .os_version("Darwin 24.0.0")
            .cwd("/home/user/project")
            .is_git_repo(true)
            .git_branch("main")
            .date("2025-01-29")
            .model("claude-3-opus")
            .language_preference("中文")
            .build()
            .unwrap();

        let ctx = ConversationContext::builder()
            .environment(env)
            .build()
            .unwrap();

        let rendered = render_environment(&ctx);

        // Should contain language preference section
        assert!(rendered.contains("# Language Preference"));
        assert!(rendered.contains("中文"));
        assert!(rendered.contains("MUST respond in"));
    }

    #[test]
    fn test_generate_tool_policy_lines_with_ls() {
        let tool_names = vec!["Read".to_string(), "Edit".to_string(), "LS".to_string()];
        let result = generate_tool_policy_lines(&tool_names);
        assert!(result.contains("Use Read for reading files"));
        assert!(result.contains("Use Edit for modifying files"));
        assert!(result.contains("Use LS for directory listing"));
        assert!(!result.contains("Use Grep"));
    }

    #[test]
    fn test_generate_tool_policy_lines_without_ls() {
        let tool_names = vec!["Read".to_string(), "Edit".to_string(), "Grep".to_string()];
        let result = generate_tool_policy_lines(&tool_names);
        assert!(result.contains("Use Read for reading files"));
        assert!(result.contains("Use Edit for modifying files"));
        assert!(result.contains("Use Grep for searching file contents"));
        assert!(!result.contains("Use LS"));
    }

    #[test]
    fn test_generate_tool_policy_lines_empty() {
        let tool_names: Vec<String> = vec![];
        let result = generate_tool_policy_lines(&tool_names);
        assert!(result.is_empty());
    }

    #[test]
    fn test_generate_tool_policy_lines_all_tools() {
        let tool_names = vec![
            "Read".to_string(),
            "Edit".to_string(),
            "Write".to_string(),
            "Grep".to_string(),
            "Glob".to_string(),
            "LS".to_string(),
        ];
        let result = generate_tool_policy_lines(&tool_names);
        assert!(result.contains("Use Read"));
        assert!(result.contains("Use Edit"));
        assert!(result.contains("Use Write"));
        assert!(result.contains("Use Grep"));
        assert!(result.contains("Use Glob"));
        assert!(result.contains("Use LS"));
    }
}
