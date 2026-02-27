use codex_protocol::openai_models::ReasoningEffort;

pub(crate) fn model_with_reasoning_label(
    model: &str,
    reasoning_effort: Option<ReasoningEffort>,
) -> String {
    if let Some(reasoning_label) = reasoning_label_for(model, reasoning_effort) {
        format!("{} {reasoning_label}", model)
    } else {
        model.to_string()
    }
}

pub(crate) fn reasoning_label(reasoning_effort: Option<ReasoningEffort>) -> &'static str {
    match reasoning_effort {
        Some(ReasoningEffort::Minimal) => "minimal",
        Some(ReasoningEffort::Low) => "low",
        Some(ReasoningEffort::Medium) => "medium",
        Some(ReasoningEffort::High) => "high",
        Some(ReasoningEffort::XHigh) => "xhigh",
        None | Some(ReasoningEffort::None) => "default",
    }
}

pub(crate) fn reasoning_label_for(
    model: &str,
    reasoning_effort: Option<ReasoningEffort>,
) -> Option<&'static str> {
    (!model.starts_with("codex-auto-")).then(|| reasoning_label(reasoning_effort))
}
