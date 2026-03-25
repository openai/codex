use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;

pub const AGENTS_MD_START_MARKER: &str = "# AGENTS.md instructions for ";
pub const AGENTS_MD_END_MARKER: &str = "</INSTRUCTIONS>";
pub const SKILL_OPEN_TAG: &str = "<skill>";
pub const SKILL_CLOSE_TAG: &str = "</skill>";

#[derive(Clone, Copy)]
pub struct ContextualUserFragmentDefinition {
    start_marker: &'static str,
    end_marker: &'static str,
}

impl ContextualUserFragmentDefinition {
    pub const fn new(start_marker: &'static str, end_marker: &'static str) -> Self {
        Self {
            start_marker,
            end_marker,
        }
    }

    pub fn matches_text(&self, text: &str) -> bool {
        let trimmed = text.trim_start();
        let starts_with_marker = trimmed
            .get(..self.start_marker.len())
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(self.start_marker));
        let trimmed = trimmed.trim_end();
        let ends_with_marker = trimmed
            .get(trimmed.len().saturating_sub(self.end_marker.len())..)
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(self.end_marker));
        starts_with_marker && ends_with_marker
    }

    pub const fn start_marker(&self) -> &'static str {
        self.start_marker
    }

    pub const fn end_marker(&self) -> &'static str {
        self.end_marker
    }

    pub fn wrap(&self, body: String) -> String {
        format!("{}\n{}\n{}", self.start_marker, body, self.end_marker)
    }

    pub fn into_message(self, text: String) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText { text }],
            end_turn: None,
            phase: None,
        }
    }
}

pub const AGENTS_MD_FRAGMENT: ContextualUserFragmentDefinition =
    ContextualUserFragmentDefinition::new(AGENTS_MD_START_MARKER, AGENTS_MD_END_MARKER);
pub const SKILL_FRAGMENT: ContextualUserFragmentDefinition =
    ContextualUserFragmentDefinition::new(SKILL_OPEN_TAG, SKILL_CLOSE_TAG);
