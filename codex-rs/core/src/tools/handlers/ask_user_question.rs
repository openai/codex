use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::ask_user_question::AskUserQuestion;

pub struct AskUserQuestionHandler;

#[derive(Deserialize)]
struct AskUserQuestionArgs {
    questions: Vec<AskUserQuestion>,
}

#[async_trait]
impl ToolHandler for AskUserQuestionHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            payload,
            call_id,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "ask_user_question handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: AskUserQuestionArgs = parse_arguments(&arguments)?;
        if args.questions.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "ask_user_question requires at least one question".to_string(),
            ));
        }

        let mut answers: Vec<Vec<String>> = Vec::with_capacity(args.questions.len());
        let mut question_ids: Vec<String> = Vec::with_capacity(args.questions.len());
        for (idx, question) in args.questions.iter().cloned().enumerate() {
            let id = format!("{call_id}:{idx}");
            let selected = session
                .request_user_question(turn.as_ref(), id.clone(), question.clone())
                .await;
            question_ids.push(id);
            answers.push(selected);
        }

        let payload = json!({
            "questions": args.questions,
            "question_ids": question_ids,
            "answers": answers,
        });

        Ok(ToolOutput::Function {
            content: payload.to_string(),
            content_items: None,
            success: Some(true),
        })
    }
}
