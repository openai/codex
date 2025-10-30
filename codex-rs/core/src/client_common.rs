use std::borrow::Cow;
use std::ops::Deref;

use futures::Stream;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::sync::mpsc;

use crate::error::Result;
pub use codex_api_client::Prompt;
pub use codex_api_client::Reasoning;
pub use codex_api_client::TextControls;
pub use codex_api_client::TextFormat;
pub use codex_api_client::TextFormatType;
use codex_apply_patch::APPLY_PATCH_TOOL_INSTRUCTIONS;
use codex_protocol::config_types::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::config_types::Verbosity as VerbosityConfig;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RateLimitSnapshot;
use codex_protocol::protocol::TokenUsage;
use serde_json::Value;

use crate::model_family::ModelFamily;

/// Review thread system prompt. Edit `core/src/review_prompt.md` to customize.
pub const REVIEW_PROMPT: &str = include_str!("../review_prompt.md");

pub const REVIEW_EXIT_SUCCESS_TMPL: &str = include_str!("../templates/review/exit_success.xml");
pub const REVIEW_EXIT_INTERRUPTED_TMPL: &str =
    include_str!("../templates/review/exit_interrupted.xml");

pub fn compute_full_instructions<'a>(
    base_override: Option<&'a str>,
    model: &'a ModelFamily,
    is_apply_patch_present: bool,
) -> Cow<'a, str> {
    let base = base_override.unwrap_or(model.base_instructions.deref());
    if base_override.is_none()
        && model.needs_special_apply_patch_instructions
        && !is_apply_patch_present
    {
        Cow::Owned(format!("{base}\n{APPLY_PATCH_TOOL_INSTRUCTIONS}"))
    } else {
        Cow::Borrowed(base)
    }
}

pub fn create_reasoning_param_for_request(
    model_family: &ModelFamily,
    effort: Option<ReasoningEffortConfig>,
    summary: ReasoningSummaryConfig,
) -> Option<Reasoning> {
    if !model_family.supports_reasoning_summaries {
        return None;
    }

    Some(Reasoning {
        effort,
        summary: Some(summary),
    })
}

pub fn create_text_param_for_request(
    verbosity: Option<VerbosityConfig>,
    output_schema: &Option<Value>,
) -> Option<TextControls> {
    if verbosity.is_none() && output_schema.is_none() {
        return None;
    }

    Some(TextControls {
        verbosity: verbosity.map(|v| match v {
            VerbosityConfig::Low => "low".to_string(),
            VerbosityConfig::Medium => "medium".to_string(),
            VerbosityConfig::High => "high".to_string(),
        }),
        format: output_schema.as_ref().map(|schema| TextFormat {
            r#type: TextFormatType::JsonSchema,
            strict: true,
            schema: schema.clone(),
            name: "codex_output_schema".to_string(),
        }),
    })
}

#[derive(Debug)]
pub enum ResponseEvent {
    Created,
    OutputItemDone(ResponseItem),
    OutputItemAdded(ResponseItem),
    Completed {
        response_id: String,
        token_usage: Option<TokenUsage>,
    },
    OutputTextDelta(String),
    ReasoningSummaryDelta(String),
    ReasoningContentDelta(String),
    ReasoningSummaryPartAdded,
    RateLimits(RateLimitSnapshot),
}

pub struct ResponseStream {
    pub(crate) rx_event: mpsc::Receiver<Result<ResponseEvent>>,
}

impl Stream for ResponseStream {
    type Item = Result<ResponseEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx_event.poll_recv(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_family::find_family_for_model;

    #[test]
    fn compute_full_instructions_respects_apply_patch_flag() {
        let model = find_family_for_model("gpt-4.1").expect("model");
        let with_tool = compute_full_instructions(None, &model, true);
        assert_eq!(with_tool.as_ref(), model.base_instructions.deref());

        let without_tool = compute_full_instructions(None, &model, false);
        assert!(
            without_tool
                .as_ref()
                .ends_with(APPLY_PATCH_TOOL_INSTRUCTIONS)
        );
    }

    #[test]
    fn create_text_controls_includes_verbosity() {
        let controls = create_text_param_for_request(Some(VerbosityConfig::Low), &None)
            .expect("text controls");
        assert_eq!(controls.verbosity.as_deref(), Some("low"));
        assert!(controls.format.is_none());
    }

    #[test]
    fn create_text_controls_includes_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {"answer": {"type": "string"}},
            "required": ["answer"],
        });
        let controls =
            create_text_param_for_request(None, &Some(schema.clone())).expect("text controls");
        let format = controls.format.expect("format");
        assert_eq!(format.name, "codex_output_schema");
        assert!(format.strict);
        assert_eq!(format.schema, schema);
    }

    #[test]
    fn create_text_controls_none_when_no_options() {
        assert!(create_text_param_for_request(None, &None).is_none());
    }
}
