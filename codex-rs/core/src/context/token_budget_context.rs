use super::ContextualUserFragment;
use super::world_state::PreviousSectionState;
use super::world_state::WorldStateSection;
use crate::config::TokenBudgetConfig;
use crate::context::world_state::Placement;
use crate::context::world_state::SectionTransition;
use crate::context::world_state::WorldStateUpdate;
use crate::context::world_state::WorldStateUpdateContent;
use crate::state::AutoCompactWindowIds;
use codex_context_fragments::AnnotatedContent;
use codex_context_fragments::message_from_parts;
use codex_protocol::AgentPath;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::input_text;
use codex_protocol::protocol::CONTEXT_WINDOW_CLOSE_TAG;
use codex_protocol::protocol::CONTEXT_WINDOW_GUIDANCE_CLOSE_TAG;
use codex_protocol::protocol::CONTEXT_WINDOW_GUIDANCE_OPEN_TAG;
use codex_protocol::protocol::CONTEXT_WINDOW_OPEN_TAG;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TokenBudgetContext {
    agent_path: AgentPath,
    ids: AutoCompactWindowIds,
    as_tool_output: bool,
}

impl TokenBudgetContext {
    pub(crate) fn new(
        agent_path: AgentPath,
        ids: AutoCompactWindowIds,
        token_budget: Option<&TokenBudgetConfig>,
    ) -> Self {
        Self {
            agent_path,
            ids,
            as_tool_output: token_budget.is_some_and(|budget| budget.context_window_as_tool_output),
        }
    }

    fn description(&self) -> String {
        let first_window_id = &self.ids.first_window_id;
        let window_id = &self.ids.window_id;
        let mut lines = vec![
            format!("Agent name: {}", self.agent_path),
            format!("First context window id: {first_window_id}"),
            format!("Current context window id: {window_id}"),
        ];
        if let Some(previous_window_id) = &self.ids.previous_window_id {
            lines.push(format!("Previous context window id: {previous_window_id}"));
        }
        lines.join("\n")
    }

    /// Renders the window fields together with any hints supplied for this window.
    pub(crate) fn render_with_hints(&self, hints: Vec<AnnotatedContent>) -> ResponseItem {
        if self.as_tool_output {
            let mut text = self.description();
            for hint in hints {
                if let Some(hint) = hint.into_parts().0.into_text() {
                    text.push('\n');
                    text.push_str(&hint);
                }
            }
            return ResponseItem::FunctionCallOutput {
                id: None,
                call_id: None,
                namespace: Some("agent".into()),
                name: Some("thread_hint".into()),
                output: FunctionCallOutputPayload::from_text(text),
                internal_chat_message_metadata_passthrough: None,
            };
        }
        let (open, close) = self.markers();
        let annotate = |content| {
            AnnotatedContent::from_parts(content, self.content_kind(), self.content_metadata())
        };
        let mut content = vec![annotate(input_text(open)), annotate(self.body())];
        for hint in hints {
            content.push(hint);
            content.push(annotate(input_text("\n")));
        }
        content.push(annotate(input_text(close)));
        message_from_parts(self.role(), content)
    }
}

impl ContextualUserFragment for TokenBudgetContext {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("token_budget.context_window".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (CONTEXT_WINDOW_OPEN_TAG, CONTEXT_WINDOW_CLOSE_TAG)
    }

    fn body(&self) -> ContentItem {
        input_text(format!("\n{}\n", self.description()))
    }
}

impl WorldStateSection for TokenBudgetContext {
    const ID: &'static str = "context_window";
    type Snapshot = AgentPath;

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> SectionTransition<Self::Snapshot> {
        let mut updates = Vec::new();
        if matches!(previous, PreviousSectionState::Known(path) if path != &self.agent_path) {
            updates.push(if self.as_tool_output {
                WorldStateUpdate {
                    placement: Placement::Standalone,
                    content: WorldStateUpdateContent::Item(Box::new(
                        self.render_with_hints(Vec::new()),
                    )),
                }
            } else {
                WorldStateUpdate::fragment(self.clone()).standalone()
            });
        }
        (Some(self.agent_path.clone()), updates)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContextWindowGuidance {
    message: String,
}

impl ContextWindowGuidance {
    pub(crate) fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

impl ContextualUserFragment for ContextWindowGuidance {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("token_budget.context_window_guidance".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (
            CONTEXT_WINDOW_GUIDANCE_OPEN_TAG,
            CONTEXT_WINDOW_GUIDANCE_CLOSE_TAG,
        )
    }

    fn body(&self) -> ContentItem {
        input_text(format!("\n{}\n", self.message))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TokenBudgetRemainingContext {
    tokens_left: Option<i64>,
}

impl TokenBudgetRemainingContext {
    pub(crate) fn new(tokens_left: i64) -> Self {
        Self {
            tokens_left: Some(tokens_left),
        }
    }

    pub(crate) fn unknown() -> Self {
        Self { tokens_left: None }
    }
}

impl ContextualUserFragment for TokenBudgetRemainingContext {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("token_budget.remaining_tokens".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> ContentItem {
        input_text(match self.tokens_left {
            Some(tokens_left) => {
                format!("You have {tokens_left} tokens left in this context window.")
            }
            None => "You have unknown tokens left in this context window.".to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TokenBudgetReminder {
    message: String,
}

impl TokenBudgetReminder {
    pub(crate) fn new(message_template: &str, n_remaining: i64) -> Self {
        Self {
            message: message_template.replace("{n_remaining}", &n_remaining.to_string()),
        }
    }
}

impl ContextualUserFragment for TokenBudgetReminder {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("token_budget.reminder".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> ContentItem {
        input_text(self.message.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AutoCompactFallbackPrompt {
    message: String,
}

impl AutoCompactFallbackPrompt {
    pub(crate) fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

impl ContextualUserFragment for AutoCompactFallbackPrompt {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("compaction.auto_fallback_prompt".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> ContentItem {
        input_text(self.message.clone())
    }
}
