use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use codex_core::AuthManager;
use codex_core::ModelClient;
use codex_core::Prompt;
use codex_core::ResponseEvent;
use codex_core::config::Config;
use codex_core::default_client::originator;
use codex_core::models_manager::manager::ModelsManager;
use codex_otel::SessionTelemetry;
use codex_protocol::ThreadId;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionSource;
use ratatui::prelude::Stylize;
use ratatui::text::Line;
use tokio::time::timeout;
use tokio_stream::StreamExt;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::history_cell::HistoryCell;
use crate::markdown;
use crate::wrapping::RtOptions;
use crate::wrapping::adaptive_wrap_lines;

const ANSWER_INTERLEAVE_MODEL: &str = "gpt-5.1-codex-mini";
const ANSWER_INTERLEAVE_TIMEOUT: Duration = Duration::from_secs(8);
const ANSWER_INTERLEAVE_STATUS: &str = "restructuring answer...";
const ANSWER_INTERLEAVE_AGENT_LABEL: &str = "tui_answer_interleave";

#[derive(Debug, Clone)]
pub(crate) struct AnswerInterleaveRequest {
    pub(crate) request_id: u64,
    pub(crate) user_prompt: String,
    pub(crate) final_answer: String,
}

#[derive(Debug)]
struct LiveAnswerState {
    markdown: String,
    interleaving_pending: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct LiveAnswerHandle {
    state: Arc<Mutex<LiveAnswerState>>,
}

impl LiveAnswerHandle {
    pub(crate) fn new(cwd: PathBuf) -> (Self, LiveAnswerCell) {
        let state = Arc::new(Mutex::new(LiveAnswerState {
            markdown: String::new(),
            interleaving_pending: false,
        }));
        (
            Self {
                state: Arc::clone(&state),
            },
            LiveAnswerCell { state, cwd },
        )
    }

    pub(crate) fn push_delta(&self, delta: &str) {
        #[expect(clippy::unwrap_used)]
        let mut state = self.state.lock().unwrap();
        state.markdown.push_str(delta);
    }

    pub(crate) fn set_markdown(&self, markdown: String) {
        #[expect(clippy::unwrap_used)]
        let mut state = self.state.lock().unwrap();
        state.markdown = markdown;
    }

    pub(crate) fn set_interleaving_pending(&self, interleaving_pending: bool) {
        #[expect(clippy::unwrap_used)]
        let mut state = self.state.lock().unwrap();
        state.interleaving_pending = interleaving_pending;
    }

    pub(crate) fn markdown(&self) -> String {
        #[expect(clippy::unwrap_used)]
        let state = self.state.lock().unwrap();
        state.markdown.clone()
    }
}

#[derive(Debug)]
pub(crate) struct LiveAnswerCell {
    state: Arc<Mutex<LiveAnswerState>>,
    cwd: PathBuf,
}

impl HistoryCell for LiveAnswerCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.render(width)
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.render(width)
    }
}

impl LiveAnswerCell {
    fn render(&self, width: u16) -> Vec<Line<'static>> {
        #[expect(clippy::unwrap_used)]
        let state = self.state.lock().unwrap();
        let mut lines = Vec::new();
        markdown::append_markdown(
            &state.markdown,
            Some(width as usize),
            Some(self.cwd.as_path()),
            &mut lines,
        );
        if state.interleaving_pending {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(ANSWER_INTERLEAVE_STATUS.dim()));
        }
        adaptive_wrap_lines(
            &lines,
            RtOptions::new(width as usize)
                .initial_indent("• ".dim().into())
                .subsequent_indent("  ".into()),
        )
    }
}

pub(crate) struct AnswerInterleaveManager {
    auth_manager: Arc<AuthManager>,
    models_manager: Arc<ModelsManager>,
    app_tx: AppEventSender,
    config: Config,
}

impl AnswerInterleaveManager {
    pub(crate) fn new(
        auth_manager: Arc<AuthManager>,
        models_manager: Arc<ModelsManager>,
        app_tx: AppEventSender,
        config: Config,
    ) -> Self {
        Self {
            auth_manager,
            models_manager,
            app_tx,
            config,
        }
    }

    pub(crate) fn start_request(&self, request: AnswerInterleaveRequest) {
        let auth_manager = Arc::clone(&self.auth_manager);
        let models_manager = Arc::clone(&self.models_manager);
        let app_tx = self.app_tx.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let result = timeout(
                ANSWER_INTERLEAVE_TIMEOUT,
                interleave_answer(
                    auth_manager,
                    models_manager,
                    config,
                    &request.user_prompt,
                    &request.final_answer,
                ),
            )
            .await;

            let result = match result {
                Ok(result) => result,
                Err(_) => Err("answer interleaving timed out".to_string()),
            }
            .or_else(|err| {
                heuristic_interleave_answer(&request.user_prompt, &request.final_answer).ok_or(err)
            });

            app_tx.send(AppEvent::AnswerInterleaveResult {
                request_id: request.request_id,
                result,
            });
        });
    }
}

async fn interleave_answer(
    auth_manager: Arc<AuthManager>,
    models_manager: Arc<ModelsManager>,
    config: Config,
    user_prompt: &str,
    final_answer: &str,
) -> Result<String, String> {
    let model_info = models_manager
        .get_model_info(ANSWER_INTERLEAVE_MODEL, &config)
        .await;
    let auth = auth_manager.auth_cached();
    let auth_mode = auth
        .as_ref()
        .map(codex_core::CodexAuth::auth_mode)
        .map(codex_otel::TelemetryAuthMode::from);
    let account_id = auth
        .as_ref()
        .and_then(codex_core::CodexAuth::get_account_id);
    let account_email = auth
        .as_ref()
        .and_then(codex_core::CodexAuth::get_account_email);
    let telemetry = SessionTelemetry::new(
        ThreadId::new(),
        ANSWER_INTERLEAVE_MODEL,
        ANSWER_INTERLEAVE_MODEL,
        account_id,
        account_email,
        auth_mode,
        originator().value,
        config.otel.log_user_prompt,
        codex_core::terminal::user_agent(),
        SessionSource::SubAgent(codex_protocol::protocol::SubAgentSource::Other(
            ANSWER_INTERLEAVE_AGENT_LABEL.to_string(),
        )),
    );
    let model_client = ModelClient::new(
        Some(auth_manager),
        ThreadId::new(),
        config.model_provider.clone(),
        SessionSource::SubAgent(codex_protocol::protocol::SubAgentSource::Other(
            ANSWER_INTERLEAVE_AGENT_LABEL.to_string(),
        )),
        config.model_verbosity,
        false,
        false,
        false,
        None,
    );
    let mut session = model_client.new_session();
    let prompt = PromptBuilder::new(user_prompt, final_answer).build();
    let mut stream = session
        .stream(
            &prompt,
            &model_info,
            &telemetry,
            None,
            model_info.default_reasoning_summary,
            None,
            None,
        )
        .await
        .map_err(|err| err.to_string())?;

    let mut output = String::new();
    while let Some(event) = stream.next().await {
        match event.map_err(|err| err.to_string())? {
            ResponseEvent::OutputTextDelta(delta) => output.push_str(&delta),
            ResponseEvent::OutputItemDone(item) if output.is_empty() => {
                output.push_str(&response_item_text(&item));
            }
            ResponseEvent::OutputItemDone(_) => {}
            ResponseEvent::Completed { .. } => break,
            ResponseEvent::Created
            | ResponseEvent::OutputItemAdded(_)
            | ResponseEvent::ServerModel(_)
            | ResponseEvent::ServerReasoningIncluded(_)
            | ResponseEvent::ReasoningSummaryDelta { .. }
            | ResponseEvent::ReasoningContentDelta { .. }
            | ResponseEvent::ReasoningSummaryPartAdded { .. }
            | ResponseEvent::RateLimits(_)
            | ResponseEvent::ModelsEtag(_) => {}
        }
    }

    let output = output.trim().to_string();
    if output.is_empty() {
        Err("answer interleaving returned an empty response".to_string())
    } else {
        Ok(post_process_interleaved_answer(user_prompt, final_answer, &output).unwrap_or(output))
    }
}

fn response_item_text(item: &ResponseItem) -> String {
    match item {
        ResponseItem::Message { content, .. } => content
            .iter()
            .map(|content_item| match content_item {
                ContentItem::InputText { text } | ContentItem::OutputText { text } => text.as_str(),
                ContentItem::InputImage { .. } => "",
            })
            .collect(),
        _ => String::new(),
    }
}

struct PromptBuilder<'a> {
    user_prompt: &'a str,
    final_answer: &'a str,
}

impl<'a> PromptBuilder<'a> {
    fn new(user_prompt: &'a str, final_answer: &'a str) -> Self {
        Self {
            user_prompt,
            final_answer,
        }
    }

    fn build(self) -> Prompt {
        let request = format!(
            "You are formatting a Codex final answer for display in a CLI transcript.\n\
             Rewrite the assistant answer so it interleaves the user's questions with the answer.\n\
             Rules:\n\
             - Preserve the original meaning and facts.\n\
             - Do not invent information.\n\
             - If the user prompt contains multiple questions, numbered items, or bullet points, you must rewrite it into interleaved question and answer pairs.\n\
             - Keep the user's numbering or bullets when present.\n\
             - For each question or sub-question, show the prompt text, then a line starting with `A:`.\n\
             - Keep trailing sections like `Assumptions:` or warnings after the paired answers when they do not map cleanly to a single question.\n\
             - Output markdown only. Do not add a preamble or code fence.\n\
             - Only return the original answer unchanged when the prompt is clearly asking a single question.\n\n\
             <user_prompt>\n{user_prompt}\n</user_prompt>\n\n\
             <assistant_final_answer>\n{final_answer}\n</assistant_final_answer>\n",
            user_prompt = self.user_prompt,
            final_answer = self.final_answer,
        );
        let mut prompt = Prompt::default();
        prompt.input = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText { text: request }],
            end_turn: None,
            phase: None,
        }];
        prompt.base_instructions = BaseInstructions::default();
        prompt
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ListKind {
    Numbered,
    Bulleted,
}

#[derive(Debug, PartialEq, Eq)]
struct ListItem {
    marker: String,
    text: String,
}

#[derive(Debug, PartialEq, Eq)]
struct ParsedList {
    items: Vec<ListItem>,
    remainder: Option<String>,
}

fn post_process_interleaved_answer(
    user_prompt: &str,
    final_answer: &str,
    model_output: &str,
) -> Option<String> {
    if is_meaningfully_interleaved(user_prompt, model_output) {
        return None;
    }

    heuristic_interleave_answer(user_prompt, final_answer)
}

fn is_meaningfully_interleaved(user_prompt: &str, answer: &str) -> bool {
    if !has_multiple_prompt_items(user_prompt) {
        return true;
    }

    answer
        .lines()
        .any(|line| line.trim_start().starts_with("A:"))
}

fn has_multiple_prompt_items(user_prompt: &str) -> bool {
    parse_simple_list(user_prompt).is_some_and(|parsed| parsed.items.len() > 1)
}

fn heuristic_interleave_answer(user_prompt: &str, final_answer: &str) -> Option<String> {
    let questions = parse_simple_list(user_prompt)?;
    let answers = parse_simple_list(final_answer)?;
    if questions.items.len() < 2 || questions.items.len() != answers.items.len() {
        return None;
    }

    let mut lines = Vec::with_capacity(questions.items.len() * 2 + 2);
    for (question, answer) in questions.items.iter().zip(answers.items.iter()) {
        lines.push(format!("{} {}", question.marker, question.text));
        lines.push(format!("A: {}", answer.text));
    }
    if let Some(remainder) = answers.remainder
        && !remainder.trim().is_empty()
    {
        lines.push(String::new());
        lines.push(remainder);
    }

    Some(lines.join("\n"))
}

fn parse_simple_list(text: &str) -> Option<ParsedList> {
    let mut items = Vec::new();
    let mut kind = None;
    let mut started = false;
    let mut remainder_start = None;
    let lines: Vec<&str> = text.lines().collect();
    let mut idx = 0;
    while idx < lines.len() {
        let line = lines[idx];
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if started {
                remainder_start = lines[idx + 1..]
                    .iter()
                    .position(|candidate| !candidate.trim().is_empty())
                    .map(|offset| idx + 1 + offset);
                break;
            }
            idx += 1;
            continue;
        }

        if let Some((line_kind, marker, item_text)) = parse_list_item_line(trimmed) {
            match kind {
                Some(existing_kind) if existing_kind != line_kind => {
                    remainder_start = Some(idx);
                    break;
                }
                None => kind = Some(line_kind),
                Some(_) => {}
            }

            started = true;
            items.push(ListItem {
                marker,
                text: item_text.to_string(),
            });
            idx += 1;
            continue;
        }

        if started {
            if let Some(last_item) = items.last_mut() {
                last_item.text.push('\n');
                last_item.text.push_str(trimmed);
            }
            idx += 1;
            continue;
        }

        idx += 1;
    }

    if items.is_empty() {
        return None;
    }

    let remainder = remainder_start.map(|start| lines[start..].join("\n").trim().to_string());

    Some(ParsedList { items, remainder })
}

fn parse_list_item_line(line: &str) -> Option<(ListKind, String, &str)> {
    if let Some((marker, text)) = parse_numbered_item_line(line) {
        return Some((ListKind::Numbered, marker, text));
    }
    if let Some(text) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
        return Some((ListKind::Bulleted, line[..1].to_string(), text.trim()));
    }
    None
}

fn parse_numbered_item_line(line: &str) -> Option<(String, &str)> {
    let marker_end = line
        .char_indices()
        .take_while(|(_, ch)| ch.is_ascii_digit())
        .last()
        .map(|(idx, _)| idx + 1)?;
    let marker = line.get(..marker_end)?;
    let remainder = line.get(marker_end..)?;
    let (separator, text) = remainder.split_at_checked(1)?;
    if !matches!(separator, "." | ")") {
        return None;
    }
    let text = text.trim_start();
    if text.is_empty() {
        return None;
    }
    Some((format!("{marker}{separator}"), text))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::heuristic_interleave_answer;
    use super::parse_simple_list;
    use super::post_process_interleaved_answer;

    #[test]
    fn heuristic_interleave_pairs_numbered_questions_and_answers() {
        let result = heuristic_interleave_answer(
            "1. what's my name?\n2. what are we doing?",
            "1. Your name is Tuan-Lung.\n2. We are testing interleaving.\n\nAssumptions:\n- This is inferred.",
        );

        assert_eq!(
            result,
            Some(
                "1. what's my name?\n\
                 A: Your name is Tuan-Lung.\n\
                 2. what are we doing?\n\
                 A: We are testing interleaving.\n\n\
                 Assumptions:\n\
                 - This is inferred."
                    .to_string()
            )
        );
    }

    #[test]
    fn heuristic_interleave_returns_none_for_mismatched_shapes() {
        let result = heuristic_interleave_answer(
            "1. what's my name?\n2. what are we doing?",
            "Your name is Tuan-Lung.",
        );

        assert_eq!(result, None);
    }

    #[test]
    fn post_process_uses_heuristic_when_model_returns_raw_list_answer() {
        let result = post_process_interleaved_answer(
            "1. what's my name?\n2. what are we doing?",
            "1. Your name is Tuan-Lung.\n2. We are testing interleaving.",
            "1. Your name is Tuan-Lung.\n2. We are testing interleaving.",
        );

        assert_eq!(
            result,
            Some(
                "1. what's my name?\n\
                 A: Your name is Tuan-Lung.\n\
                 2. what are we doing?\n\
                 A: We are testing interleaving."
                    .to_string()
            )
        );
    }

    #[test]
    fn parse_simple_list_preserves_remainder() {
        let parsed = parse_simple_list("1. one\n2. two\n\nTrailing section");

        assert_eq!(
            parsed.map(|parsed| parsed.remainder),
            Some(Some("Trailing section".to_string()))
        );
    }

    #[test]
    fn heuristic_interleave_supports_prompt_preamble_before_numbered_list() {
        let result = heuristic_interleave_answer(
            "For testing purpose, let me ask you a few questions:\n1. how are you?\n2. what's your favorite model?",
            "1. I'm doing fine.\n2. I don't have a favorite.",
        );

        assert_eq!(
            result,
            Some(
                "1. how are you?\n\
                 A: I'm doing fine.\n\
                 2. what's your favorite model?\n\
                 A: I don't have a favorite."
                    .to_string()
            )
        );
    }

    #[test]
    fn heuristic_interleave_keeps_continuation_lines_with_numbered_answers() {
        let result = heuristic_interleave_answer(
            "1. how are you doing?\n2. what's the biggest city in the world?\n3. who's the ceo of openai?",
            "1. I'm doing fine.\n2. Jakarta is the largest city by population.\nSource: https://example.com/jakarta\n3. Sam Altman is the CEO of OpenAI.\nSource: https://example.com/openai",
        );

        assert_eq!(
            result,
            Some(
                "1. how are you doing?\n\
                 A: I'm doing fine.\n\
                 2. what's the biggest city in the world?\n\
                 A: Jakarta is the largest city by population.\n\
                 Source: https://example.com/jakarta\n\
                 3. who's the ceo of openai?\n\
                 A: Sam Altman is the CEO of OpenAI.\n\
                 Source: https://example.com/openai"
                    .to_string()
            )
        );
    }
}
