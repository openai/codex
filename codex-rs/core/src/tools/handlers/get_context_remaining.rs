use crate::context::ContextualUserFragment;
use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::get_context_remaining_spec::GET_CONTEXT_REMAINING_TOOL_NAME;
use crate::tools::handlers::get_context_remaining_spec::create_get_context_remaining_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_protocol::config_types::AutoCompactTokenLimitScope;
use codex_protocol::models::ResponseInputItem;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use serde_json::Value as JsonValue;
use serde_json::json;

#[derive(Debug, Clone)]
struct GetContextRemainingOutput {
    tokens_left: Option<i64>,
}

impl GetContextRemainingOutput {
    fn new(tokens_left: Option<i64>) -> Self {
        Self { tokens_left }
    }

    fn fragment(&self) -> String {
        match self.tokens_left {
            Some(tokens_left) => {
                crate::context::TokenBudgetRemainingContext::new(tokens_left).render()
            }
            None => crate::context::TokenBudgetRemainingContext::unknown().render(),
        }
    }
}

impl ToolOutput for GetContextRemainingOutput {
    fn log_preview(&self) -> String {
        self.fragment()
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        FunctionToolOutput::from_text(self.fragment(), Some(true))
            .to_response_item(call_id, payload)
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
        json!({
            "tokens_left": self.tokens_left,
        })
    }
}

pub struct GetContextRemainingHandler;

impl ToolExecutor<ToolInvocation> for GetContextRemainingHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(GET_CONTEXT_REMAINING_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        create_get_context_remaining_tool()
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(async move {
            if !matches!(invocation.payload, ToolPayload::Function { .. }) {
                return Err(FunctionCallError::RespondToModel(
                    "get_context_remaining handler received unsupported payload".to_string(),
                ));
            }

            let token_status = crate::session::context_window::context_window_token_status(
                invocation.session.as_ref(),
                invocation.turn.as_ref(),
            )
            .await;

            let tokens_left = match invocation.turn.config.model_auto_compact_token_limit_scope {
                AutoCompactTokenLimitScope::Total => {
                    invocation.turn.model_context_window().map(|limit| {
                        limit
                            .saturating_sub(token_status.active_context_tokens.max(0))
                            .max(0)
                    })
                }
                AutoCompactTokenLimitScope::BodyAfterPrefix => {
                    let scope_limit = invocation
                        .turn
                        .config
                        .model_auto_compact_token_limit
                        .or_else(|| invocation.turn.model_info.auto_compact_token_limit());
                    (scope_limit.is_some() || token_status.full_context_window_limit.is_some())
                        .then_some(token_status.tokens_until_compaction)
                }
            };

            Ok(boxed_tool_output(GetContextRemainingOutput::new(
                tokens_left,
            )))
        })
    }
}

impl CoreToolRuntime for GetContextRemainingHandler {}
