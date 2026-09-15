//! Human-readable plugin and skill labels.

pub(super) fn tool_name(label: &str) -> String {
    let label = match label.split_once(':') {
        Some((namespace, name)) if namespace.trim().eq_ignore_ascii_case(name.trim()) => {
            name.trim()
        }
        _ => label,
    };
    label
        .split_whitespace()
        .map(|word| match word.to_ascii_lowercase().as_str() {
            "pr" => "PR",
            "cli" => "CLI",
            "api" => "API",
            "ide" => "IDE",
            "mcp" => "MCP",
            "sdk" => "SDK",
            "ui" => "UI",
            "tui" => "TUI",
            "openai" => "OpenAI",
            _ => word,
        })
        .collect::<Vec<_>>()
        .join(" ")
}
