use async_trait::async_trait;

use crate::features::Feature;
use crate::function_tool::FunctionCallError;
use crate::tools::code_mode;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::models::FunctionCallOutputBody;
use serde::Deserialize;

pub struct CodeModeHandler;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CodeModeArgs {
    code: String,
    #[serde(default, rename = "timeout_ms", alias = "_timeout_ms")]
    timeout_ms: Option<u64>,
}

#[async_trait]
impl ToolHandler for CodeModeHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(
            payload,
            ToolPayload::Function { .. } | ToolPayload::Custom { .. }
        )
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            tracker,
            payload,
            ..
        } = invocation;

        if !session.features().enabled(Feature::CodeMode) {
            return Err(FunctionCallError::RespondToModel(
                "code_mode is disabled by feature flag".to_string(),
            ));
        }

        let (code, timeout_ms) = match payload {
            ToolPayload::Function { arguments } => {
                let args: CodeModeArgs = parse_arguments(&arguments)?;
                (args.code, args.timeout_ms)
            }
            ToolPayload::Custom { input } => (input, None),
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "code_mode expects function or custom payload".to_string(),
                ));
            }
        };

        let content_items = code_mode::execute(session, turn, tracker, code, timeout_ms).await?;
        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::ContentItems(content_items),
            success: Some(true),
        })
    }
}
