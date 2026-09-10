//! Measures and checks complete synchronous requests after wire prefix assembly.
//! Includes reused history, tool definitions, output format and continuations.

use codex_api::ResponsesApiRequest;
use codex_guardian_context::REQUEST_TOKENS_BOUNDARIES;
use codex_guardian_context::REQUEST_TOKENS_METRIC;
use codex_guardian_context::effective_input_token_limit;
use codex_otel::SessionTelemetry;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::TruncationPolicy;

use crate::config::Config;
use crate::context_manager::estimate_item_token_count;
use crate::session::session::Session;

pub(super) const INPUT_TOKEN_MARGIN: usize = 256;

// Keep a failed reviewer out of reuse unless compaction and a fresh budget
// check succeed. Also distinguishes local budget failures from backend errors.
pub(crate) enum ExhaustedReviewBudget {
    Detected,
    // A compaction service failure must not be mistaken for local exhaustion.
    Compacting,
}

pub(crate) fn observe(telemetry: &SessionTelemetry, request: &ResponsesApiRequest) -> usize {
    let total = estimate_request_tokens(request);
    // The assembled input already includes inherited history and the current
    // review. Do not report a guessed old/new split after context injection.
    telemetry.histogram_with_boundaries(
        REQUEST_TOKENS_METRIC,
        i64::try_from(total).unwrap_or(i64::MAX),
        REQUEST_TOKENS_BOUNDARIES,
        &[("target", "sync"), ("component", "total")],
    );
    total
}

pub(super) fn estimate_request_tokens(request: &ResponsesApiRequest) -> usize {
    let input = request
        .input
        .iter()
        .map(estimate_item_token_count)
        .fold(0i64, i64::saturating_add);
    let instructions = TruncationPolicy::Bytes(request.instructions.len()).token_budget();
    let metadata = serde_json::to_vec(&(&request.tools, &request.text))
        .map(|bytes| TruncationPolicy::Bytes(bytes.len()).token_budget())
        .unwrap_or(usize::MAX);
    usize::try_from(input)
        .unwrap_or(usize::MAX)
        .saturating_add(instructions)
        .saturating_add(metadata)
}

/// Checks the fully assembled prompt after turn context has been injected. This
/// final guard also covers retries and reviewer tool continuations.
pub(crate) fn check_prompt(
    session: &Session,
    prompt: &crate::client_common::Prompt,
    config: &Config,
    model: &ModelInfo,
    metadata: &crate::responses_metadata::CodexResponsesMetadata,
) -> CodexResult<()> {
    let request = session.services.model_client.build_responses_request(
        prompt,
        model,
        /*effort*/ None,
        codex_protocol::config_types::ReasoningSummary::None,
        /*service_tier*/ None,
        metadata,
    )?;
    if estimate_request_tokens(&request)
        > effective_input_token_limit(model, config.model_context_window)
            .saturating_sub(INPUT_TOKEN_MARGIN)
    {
        session
            .services
            .thread_extension_data
            .insert(ExhaustedReviewBudget::Detected);
        return Err(CodexErr::ContextWindowExceeded);
    }
    session
        .services
        .thread_extension_data
        .remove::<ExhaustedReviewBudget>();
    Ok(())
}
