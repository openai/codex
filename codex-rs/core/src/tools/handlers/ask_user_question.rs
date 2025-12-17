use async_trait::async_trait;
use codex_protocol::ask_user_question::AskUserQuestion;
use codex_protocol::ask_user_question::AskUserQuestionArgs;
use codex_protocol::ask_user_question::AskUserQuestionResponse;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use serde_json::json;

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub(crate) const ASK_USER_QUESTION_TOOL_NAME: &str = "ask_user_question";

pub struct AskUserQuestionHandler;

fn normalize_choice_label(label: &str) -> String {
    let trimmed = label.trim_start();

    let mut chars = trimmed.char_indices().peekable();
    let mut after_digits = 0usize;
    let mut saw_digit = false;
    while let Some((idx, ch)) = chars.peek().copied()
        && ch.is_ascii_digit()
    {
        saw_digit = true;
        chars.next();
        after_digits = idx + ch.len_utf8();
    }

    if !saw_digit {
        return trimmed.to_string();
    }

    // Only strip numeric prefixes when they look like enumeration: "1) Foo", "2. Bar", "3: Baz".
    let Some((idx, ch)) = chars.peek().copied() else {
        return trimmed.to_string();
    };
    if !matches!(ch, ')' | '.' | ':') {
        return trimmed.to_string();
    }

    chars.next();
    let mut end = idx + ch.len_utf8();
    while let Some((idx, ch)) = chars.peek().copied()
        && ch.is_whitespace()
    {
        chars.next();
        end = idx + ch.len_utf8();
    }

    if end <= after_digits {
        return trimmed.to_string();
    }

    let rest = trimmed[end..].trim_start();
    if rest.is_empty() {
        trimmed.to_string()
    } else {
        rest.to_string()
    }
}

fn normalize_questions(mut questions: Vec<AskUserQuestion>) -> Vec<AskUserQuestion> {
    for q in &mut questions {
        for opt in &mut q.options {
            opt.label = normalize_choice_label(opt.label.as_str());
        }
    }
    questions
}

#[async_trait]
impl ToolHandler for AskUserQuestionHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        true
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            tool_name,
            payload,
            ..
        } = invocation;

        let ToolPayload::Function { arguments } = payload else {
            return Err(FunctionCallError::RespondToModel(format!(
                "unsupported payload for {tool_name}"
            )));
        };

        let source = turn.client.get_session_source();
        if let SessionSource::SubAgent(SubAgentSource::Other(label)) = &source
            && label.starts_with("plan_variant")
        {
            return Err(FunctionCallError::RespondToModel(
                "AskUserQuestion is not supported in non-interactive planning subagents"
                    .to_string(),
            ));
        }

        let args: AskUserQuestionArgs = serde_json::from_str(&arguments).map_err(|e| {
            FunctionCallError::RespondToModel(format!("failed to parse function arguments: {e:?}"))
        })?;

        validate_questions(&args.questions).map_err(FunctionCallError::RespondToModel)?;

        let questions = normalize_questions(args.questions);
        validate_questions(&questions).map_err(FunctionCallError::RespondToModel)?;

        let response = session
            .request_ask_user_question(turn.as_ref(), call_id, questions)
            .await;

        match response {
            AskUserQuestionResponse::Answered { answers } => {
                let output = json!({ "answers": answers }).to_string();
                Ok(ToolOutput::Function {
                    content: output,
                    content_items: None,
                    success: Some(true),
                })
            }
            AskUserQuestionResponse::Cancelled => Err(FunctionCallError::RespondToModel(
                "AskUserQuestion was cancelled by the user".to_string(),
            )),
        }
    }
}

fn validate_questions(questions: &[AskUserQuestion]) -> Result<(), String> {
    if questions.is_empty() {
        return Err("AskUserQuestion requires at least 1 question".to_string());
    }
    if questions.len() > 4 {
        return Err("AskUserQuestion supports at most 4 questions".to_string());
    }

    for (idx, q) in questions.iter().enumerate() {
        if q.header.is_empty() {
            return Err(format!("question {idx} header must be non-empty"));
        }
        if q.header.chars().count() > 12 {
            return Err(format!(
                "question {idx} header must be at most 12 characters"
            ));
        }

        if q.question.is_empty() {
            return Err(format!("question {idx} must be non-empty"));
        }

        if q.options.len() < 2 || q.options.len() > 4 {
            return Err(format!(
                "question {idx} options must have 2-4 items (Other is provided automatically)"
            ));
        }
        for opt in &q.options {
            if opt.label.eq_ignore_ascii_case("other") {
                return Err(format!(
                    "question {idx} must not include an 'Other' option (it is provided automatically)"
                ));
            }
            if opt.label.is_empty() {
                return Err(format!("question {idx} option label must be non-empty"));
            }
        }
    }

    Ok(())
}
