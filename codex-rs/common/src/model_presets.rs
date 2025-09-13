use codex_core::protocol_config_types::ReasoningEffort;

/// A simple preset pairing a model slug with a reasoning effort.
#[derive(Debug, Clone, Copy)]
pub struct ModelPreset {
    /// Stable identifier for the preset.
    pub id: &'static str,
    /// Display label shown in UIs.
    pub label: &'static str,
    /// Short human description shown next to the label in UIs.
    pub description: &'static str,
    /// Model slug (e.g., "gpt-5").
    pub model: &'static str,
    /// Reasoning effort to apply for this preset.
    pub effort: Option<ReasoningEffort>,
}

/// Built-in list of model presets that pair a model with a reasoning effort.
///
/// Keep this UI-agnostic so it can be reused by both TUI and MCP server.
pub fn builtin_model_presets() -> &'static [ModelPreset] {
    // Order reflects effort from minimal to high.
    const PRESETS: &[ModelPreset] = &[
        ModelPreset {
            id: "gpt-5-minimal",
            label: "gpt-5 minimal",
            description: "— fastest responses with limited reasoning; ideal for coding, instructions, or lightweight tasks",
            model: "gpt-5",
            effort: Some(ReasoningEffort::Minimal),
        },
        ModelPreset {
            id: "gpt-5-low",
            label: "gpt-5 low",
            description: "— balances speed with some reasoning; useful for straightforward queries and short explanations",
            model: "gpt-5",
            effort: Some(ReasoningEffort::Low),
        },
        ModelPreset {
            id: "gpt-5-medium",
            label: "gpt-5 medium",
            description: "— default setting; provides a solid balance of reasoning depth and latency for general-purpose tasks",
            model: "gpt-5",
            effort: Some(ReasoningEffort::Medium),
        },
        ModelPreset {
            id: "gpt-5-high",
            label: "gpt-5 high",
            description: "— maximizes reasoning depth for complex or ambiguous problems",
            model: "gpt-5",
            effort: Some(ReasoningEffort::High),
        },
        ModelPreset {
            id: "swiftfox-low",
            label: "swiftfox low",
            description: "— balances speed with some reasoning; useful for straightforward queries and short explanations",
            model: "swiftfox-low",
            effort: None,
        },
        ModelPreset {
            id: "swiftfox-medium",
            label: "swiftfox medium",
            description: "— default setting; provides a solid balance of reasoning depth and latency for general-purpose tasks",
            model: "swiftfox-medium",
            effort: None,
        },
        ModelPreset {
            id: "swiftfox-high",
            label: "swiftfox high",
            description: "",
            model: "swiftfox-high",
            effort: None,
        },
    ];
    PRESETS
}
