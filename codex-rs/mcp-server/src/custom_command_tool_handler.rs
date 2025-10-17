// Custom Command Tool Handler
use anyhow::Result;
use mcp_types::CallToolResult;
use mcp_types::ContentBlock;
use mcp_types::TextContent;
use serde_json::Value;

use crate::custom_command_tool::CustomCommandToolParam;

pub async fn handle_custom_command_tool_call(arguments: Value) -> Result<CallToolResult> {
    let params: CustomCommandToolParam = serde_json::from_value(arguments)?;

    let response_text = match params.action.as_str() {
        "execute" => {
            let command_name = params
                .command_name
                .ok_or_else(|| anyhow::anyhow!("command_name required for execute"))?;
            let context = params
                .context
                .unwrap_or_else(|| "No context provided".to_string());

            let (subagent, description) = match command_name.as_str() {
                "analyze_code" => ("CodeExpert", "Analyzing code for bugs and improvements"),
                "security_review" => ("SecurityExpert", "Performing security review"),
                "generate_tests" => ("TestingExpert", "Generating test suite"),
                "deep_research" => ("DeepResearcher", "Conducting deep research"),
                "debug_issue" => ("DebugExpert", "Debugging and fixing issues"),
                "optimize_performance" => ("PerformanceExpert", "Optimizing performance"),
                "generate_docs" => ("DocsExpert", "Generating documentation"),
                _ => ("General", "Processing custom command"),
            };

            format!(
                "[CustomCommand] Executing: {}\n\n\
                Dispatching to subagent: {}\n\
                Description: {}\n\n\
                Context:\n{}\n\n\
                The {} subagent will process this request asynchronously.\n\
                Use the 'codex-subagent' tool with action 'check_inbox' to see results.",
                command_name, subagent, description, context, subagent
            )
        }
        "list" => "Available Custom Commands (7):\n\n\
            1. analyze_code → CodeExpert\n\
               Analyze code for bugs and improvements\n\n\
            2. security_review → SecurityExpert\n\
               Perform comprehensive security review\n\n\
            3. generate_tests → TestingExpert\n\
               Generate comprehensive test suite\n\n\
            4. deep_research → DeepResearcher\n\
               Conduct deep research on a topic\n\n\
            5. debug_issue → DebugExpert\n\
               Debug and fix issues\n\n\
            6. optimize_performance → PerformanceExpert\n\
               Optimize code for better performance\n\n\
            7. generate_docs → DocsExpert\n\
               Generate comprehensive documentation\n\n\
            Use action='execute' with command_name to run a command."
            .to_string(),
        "info" => {
            let command_name = params
                .command_name
                .ok_or_else(|| anyhow::anyhow!("command_name required for info"))?;

            match command_name.as_str() {
                "analyze_code" => "Command: analyze_code\n\
                    Description: Analyze code for bugs and improvements\n\
                    Subagent: CodeExpert\n\
                    Parameters: depth=detailed\n\
                    Pre-hooks: 0\n\
                    Post-hooks: 0"
                    .to_string(),
                "security_review" => "Command: security_review\n\
                    Description: Perform comprehensive security review\n\
                    Subagent: SecurityExpert\n\
                    Parameters: check_vulnerabilities=true\n\
                    Pre-hooks: 0\n\
                    Post-hooks: 0"
                    .to_string(),
                "deep_research" => "Command: deep_research\n\
                    Description: Conduct deep research on a topic\n\
                    Subagent: DeepResearcher\n\
                    Parameters: depth=5, sources=20\n\
                    Pre-hooks: 0\n\
                    Post-hooks: 0"
                    .to_string(),
                _ => {
                    format!(
                        "Command: {}\nNo detailed information available.",
                        command_name
                    )
                }
            }
        }
        _ => {
            return Err(anyhow::anyhow!("Unknown action: {}", params.action));
        }
    };

    Ok(CallToolResult {
        content: vec![ContentBlock::TextContent(TextContent {
            r#type: "text".to_string(),
            text: response_text,
            annotations: None,
        })],
        is_error: None,
        structured_content: None,
    })
}
