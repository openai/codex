use crate::legacy_core::config::Config;
use codex_features::Feature;

pub(crate) const TERMINAL_VISUALIZATION_INSTRUCTIONS: &str = "\
- This surface is a terminal. When the formatting rules require a visual, include one in the final answer using compact ASCII diagrams, trees, timelines, or tables.
- Use tables for exact mappings or comparisons rather than collapsing known mappings into prose.
- Use trees for hierarchy or one-to-many relationships, and diagrams or timelines for sequence, change, or state transferred between records across event order.
- Use only ASCII characters in visuals.";

pub(crate) fn with_terminal_visualization_instructions(
    config: &Config,
    control_instructions: Option<String>,
) -> Option<String> {
    if !config
        .features
        .enabled(Feature::TerminalVisualizationInstructions)
    {
        return control_instructions;
    }

    let existing_instructions =
        control_instructions.or_else(|| config.developer_instructions.clone());
    Some(match existing_instructions.as_deref() {
        Some(existing) if !existing.trim().is_empty() => {
            format!("{existing}\n\n{TERMINAL_VISUALIZATION_INSTRUCTIONS}")
        }
        _ => TERMINAL_VISUALIZATION_INSTRUCTIONS.to_string(),
    })
}
