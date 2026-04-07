use codex_instructions::AGENTS_MD_FRAGMENT;
use codex_instructions::SKILL_FRAGMENT;
use codex_protocol::items::parse_hook_prompt_fragment;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::LocalShellAction;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::APPS_INSTRUCTIONS_OPEN_TAG;
use codex_protocol::protocol::COLLABORATION_MODE_OPEN_TAG;
use codex_protocol::protocol::PLUGINS_INSTRUCTIONS_OPEN_TAG;
use codex_protocol::protocol::REALTIME_CONVERSATION_OPEN_TAG;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;

use crate::context_manager::history::estimate_item_token_count;
use crate::contextual_user_message::ENVIRONMENT_CONTEXT_FRAGMENT;
use crate::contextual_user_message::SUBAGENT_NOTIFICATION_FRAGMENT;
use crate::contextual_user_message::TURN_ABORTED_FRAGMENT;
use crate::contextual_user_message::USER_SHELL_COMMAND_FRAGMENT;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextWindowBreakdown {
    pub model_context_window: Option<i64>,
    pub total_tokens: i64,
    pub sections: Vec<ContextWindowSection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextWindowSection {
    pub label: String,
    pub tokens: i64,
    pub details: Vec<ContextWindowDetail>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextWindowDetail {
    pub label: String,
    pub tokens: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ContextSectionKind {
    BuiltIn,
    User,
    Conversation,
}

#[derive(Debug, Clone)]
struct DetailAllocation {
    section: ContextSectionKind,
    label: String,
    estimated_tokens: i64,
}

#[derive(Debug, Default)]
struct SectionAccumulator {
    section: Option<ContextWindowSection>,
}

#[derive(Debug, Default)]
struct BreakdownAccumulator {
    built_in: SectionAccumulator,
    user: SectionAccumulator,
    conversation: SectionAccumulator,
    verbose: bool,
}

pub(super) fn build_context_window_breakdown(
    items: &[ResponseItem],
    base_instructions: &BaseInstructions,
    model_context_window: Option<i64>,
    verbose: bool,
) -> ContextWindowBreakdown {
    let mut accumulator = BreakdownAccumulator {
        verbose,
        ..Default::default()
    };
    let base_instruction_tokens = estimate_text_tokens(&base_instructions.text);
    accumulator.add_detail(
        ContextSectionKind::BuiltIn,
        "Base instructions".to_string(),
        base_instruction_tokens,
    );

    for item in items {
        accumulator.add_item(item);
    }

    ContextWindowBreakdown {
        model_context_window,
        total_tokens: base_instruction_tokens.saturating_add(
            items
                .iter()
                .map(estimate_item_token_count)
                .fold(0i64, i64::saturating_add),
        ),
        sections: accumulator.into_sections(),
    }
}

impl BreakdownAccumulator {
    fn add_item(&mut self, item: &ResponseItem) {
        let item_tokens = estimate_item_token_count(item);
        match item {
            ResponseItem::Message {
                id,
                role,
                content,
                end_turn,
                phase,
            } => {
                let mut details =
                    classify_message_content(id.as_ref(), role, content, *end_turn, phase.as_ref());
                scale_detail_tokens(&mut details, item_tokens);
                for detail in details {
                    self.add_detail(detail.section, detail.label, detail.estimated_tokens);
                }
            }
            ResponseItem::Reasoning { .. } => {
                self.add_detail(
                    ContextSectionKind::Conversation,
                    "Reasoning".to_string(),
                    item_tokens,
                );
            }
            ResponseItem::LocalShellCall { action, .. } => {
                let command = match action {
                    LocalShellAction::Exec(exec) => exec.command.join(" "),
                };
                self.add_detail(
                    ContextSectionKind::Conversation,
                    format!("Shell call: {command}"),
                    item_tokens,
                );
            }
            ResponseItem::FunctionCall { name, .. } => {
                self.add_detail(
                    ContextSectionKind::Conversation,
                    format!("Tool call: {name}"),
                    item_tokens,
                );
            }
            ResponseItem::FunctionCallOutput { .. } => {
                self.add_detail(
                    ContextSectionKind::Conversation,
                    "Tool output".to_string(),
                    item_tokens,
                );
            }
            ResponseItem::ToolSearchCall { execution, .. } => {
                self.add_detail(
                    ContextSectionKind::Conversation,
                    format!("Tool search: {execution}"),
                    item_tokens,
                );
            }
            ResponseItem::CustomToolCall { name, .. } => {
                self.add_detail(
                    ContextSectionKind::Conversation,
                    format!("Custom tool call: {name}"),
                    item_tokens,
                );
            }
            ResponseItem::CustomToolCallOutput { name, .. } => {
                self.add_detail(
                    ContextSectionKind::Conversation,
                    format!(
                        "Custom tool output{}",
                        name.as_ref()
                            .map(|value| format!(": {value}"))
                            .unwrap_or_default()
                    ),
                    item_tokens,
                );
            }
            ResponseItem::ToolSearchOutput { execution, .. } => {
                self.add_detail(
                    ContextSectionKind::Conversation,
                    format!("Tool search output: {execution}"),
                    item_tokens,
                );
            }
            ResponseItem::WebSearchCall { .. } => {
                self.add_detail(
                    ContextSectionKind::Conversation,
                    "Web search call".to_string(),
                    item_tokens,
                );
            }
            ResponseItem::ImageGenerationCall { .. } => {
                self.add_detail(
                    ContextSectionKind::Conversation,
                    "Image generation call".to_string(),
                    item_tokens,
                );
            }
            ResponseItem::Compaction { .. } => {
                self.add_detail(
                    ContextSectionKind::Conversation,
                    "Compaction summary".to_string(),
                    item_tokens,
                );
            }
            ResponseItem::GhostSnapshot { .. } | ResponseItem::Other => {}
        }
    }

    fn add_detail(&mut self, section: ContextSectionKind, label: String, tokens: i64) {
        if tokens <= 0 {
            return;
        }
        let verbose = self.verbose;
        let section = self
            .section_accumulator(section)
            .section
            .get_or_insert_with(|| ContextWindowSection {
                label: section.label().to_string(),
                tokens: 0,
                details: Vec::new(),
            });
        section.tokens = section.tokens.saturating_add(tokens);
        if verbose {
            section.details.push(ContextWindowDetail { label, tokens });
            return;
        }
        if let Some(existing) = section
            .details
            .iter_mut()
            .find(|detail| detail.label == label)
        {
            existing.tokens = existing.tokens.saturating_add(tokens);
        } else {
            section.details.push(ContextWindowDetail { label, tokens });
        }
    }

    fn section_accumulator(&mut self, section: ContextSectionKind) -> &mut SectionAccumulator {
        match section {
            ContextSectionKind::BuiltIn => &mut self.built_in,
            ContextSectionKind::User => &mut self.user,
            ContextSectionKind::Conversation => &mut self.conversation,
        }
    }

    fn into_sections(self) -> Vec<ContextWindowSection> {
        let mut sections: Vec<ContextWindowSection> = [
            self.built_in.section,
            self.user.section,
            self.conversation.section,
        ]
        .into_iter()
        .flatten()
        .collect();
        for section in &mut sections {
            section.details.sort_by(|left, right| {
                right
                    .tokens
                    .cmp(&left.tokens)
                    .then(left.label.cmp(&right.label))
            });
        }
        sections.sort_by(|left, right| {
            right
                .tokens
                .cmp(&left.tokens)
                .then(section_order(&left.label).cmp(&section_order(&right.label)))
                .then(left.label.cmp(&right.label))
        });
        sections
    }
}

fn classify_message_content(
    id: Option<&String>,
    role: &str,
    content: &[ContentItem],
    end_turn: Option<bool>,
    phase: Option<&MessagePhase>,
) -> Vec<DetailAllocation> {
    if content.is_empty() {
        return vec![DetailAllocation {
            section: section_for_message_role(role),
            label: format_message_label(role, phase),
            estimated_tokens: estimate_message_tokens(id, role, content, end_turn, phase),
        }];
    }

    content
        .iter()
        .map(|content_item| {
            let (section, label) = classify_content_item(role, content_item, phase);
            DetailAllocation {
                section,
                label,
                estimated_tokens: estimate_message_tokens(
                    id,
                    role,
                    std::slice::from_ref(content_item),
                    end_turn,
                    phase,
                ),
            }
        })
        .collect()
}

fn classify_content_item(
    role: &str,
    content_item: &ContentItem,
    phase: Option<&MessagePhase>,
) -> (ContextSectionKind, String) {
    let (ContentItem::InputText { text } | ContentItem::OutputText { text }) = content_item else {
        return (
            ContextSectionKind::Conversation,
            format_message_label(role, phase),
        );
    };

    if role == "developer" {
        return classify_developer_text(text);
    }
    if role == "user" {
        return classify_user_text(text, phase);
    }
    (
        section_for_message_role(role),
        format_message_label(role, phase),
    )
}

fn classify_developer_text(text: &str) -> (ContextSectionKind, String) {
    let trimmed = text.trim_start();
    if starts_with_tag(trimmed, SKILLS_INSTRUCTIONS_OPEN_TAG) {
        return (
            ContextSectionKind::User,
            format!(
                "Implicit skills catalog ({} skills)",
                count_catalog_entries(trimmed)
            ),
        );
    }
    if starts_with_tag(trimmed, APPS_INSTRUCTIONS_OPEN_TAG) {
        return (
            ContextSectionKind::User,
            "Apps connector instructions".to_string(),
        );
    }
    if starts_with_tag(trimmed, PLUGINS_INSTRUCTIONS_OPEN_TAG) {
        return (
            ContextSectionKind::User,
            format!(
                "Plugin instructions ({} plugins)",
                count_catalog_entries(trimmed)
            ),
        );
    }
    if starts_with_tag(trimmed, "<permissions instructions>") {
        return (
            ContextSectionKind::BuiltIn,
            "Permission instructions".to_string(),
        );
    }
    if starts_with_tag(trimmed, "<model_switch>") {
        return (
            ContextSectionKind::BuiltIn,
            "Model switch instructions".to_string(),
        );
    }
    if starts_with_tag(trimmed, COLLABORATION_MODE_OPEN_TAG) {
        return (
            ContextSectionKind::BuiltIn,
            "Collaboration mode instructions".to_string(),
        );
    }
    if starts_with_tag(trimmed, "<personality_spec>") {
        return (
            ContextSectionKind::BuiltIn,
            "Personality instructions".to_string(),
        );
    }
    if starts_with_tag(trimmed, REALTIME_CONVERSATION_OPEN_TAG) {
        return (ContextSectionKind::BuiltIn, "Realtime context".to_string());
    }
    (
        ContextSectionKind::BuiltIn,
        "Developer instructions".to_string(),
    )
}

fn classify_user_text(text: &str, phase: Option<&MessagePhase>) -> (ContextSectionKind, String) {
    if AGENTS_MD_FRAGMENT.matches_text(text) {
        return (ContextSectionKind::User, format_agents_label(text));
    }
    if SKILL_FRAGMENT.matches_text(text) {
        return (ContextSectionKind::User, format_skill_label(text));
    }
    if ENVIRONMENT_CONTEXT_FRAGMENT.matches_text(text) {
        return (
            ContextSectionKind::BuiltIn,
            "Environment context".to_string(),
        );
    }
    if USER_SHELL_COMMAND_FRAGMENT.matches_text(text) {
        return (
            ContextSectionKind::BuiltIn,
            "User shell command".to_string(),
        );
    }
    if TURN_ABORTED_FRAGMENT.matches_text(text) {
        return (
            ContextSectionKind::BuiltIn,
            "Turn aborted marker".to_string(),
        );
    }
    if SUBAGENT_NOTIFICATION_FRAGMENT.matches_text(text) {
        return (
            ContextSectionKind::BuiltIn,
            "Subagent notification".to_string(),
        );
    }
    if parse_hook_prompt_fragment(text).is_some() {
        return (
            ContextSectionKind::BuiltIn,
            "Hook prompt context".to_string(),
        );
    }
    (
        ContextSectionKind::Conversation,
        format_message_label("user", phase),
    )
}

fn estimate_message_tokens(
    id: Option<&String>,
    role: &str,
    content: &[ContentItem],
    end_turn: Option<bool>,
    phase: Option<&MessagePhase>,
) -> i64 {
    estimate_item_token_count(&ResponseItem::Message {
        id: id.cloned(),
        role: role.to_string(),
        content: content.to_vec(),
        end_turn,
        phase: phase.cloned(),
    })
}

fn estimate_text_tokens(text: &str) -> i64 {
    codex_utils_output_truncation::approx_token_count(text)
        .try_into()
        .unwrap_or(i64::MAX)
}

fn format_agents_label(text: &str) -> String {
    let directory = text
        .trim_start()
        .strip_prefix(AGENTS_MD_FRAGMENT.start_marker())
        .and_then(|rest| rest.lines().next())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .unwrap_or("current workspace");
    format!("AGENTS.md instructions for {directory}")
}

fn format_skill_label(text: &str) -> String {
    let name = extract_tag_text(text, "name").unwrap_or("unknown skill");
    let path = extract_tag_text(text, "path");
    match path {
        Some(path) => format!("Skill: {name} ({path})"),
        None => format!("Skill: {name}"),
    }
}

fn extract_tag_text<'a>(text: &'a str, tag_name: &str) -> Option<&'a str> {
    let open_tag = format!("<{tag_name}>");
    let close_tag = format!("</{tag_name}>");
    let start = text.find(&open_tag)?.saturating_add(open_tag.len());
    let value = text.get(start..)?;
    let end = value.find(&close_tag)?;
    Some(value[..end].trim())
}

fn count_catalog_entries(text: &str) -> usize {
    let mut in_available_section = false;
    let mut count = 0;
    for line in text.lines() {
        match line.trim() {
            "### Available skills" | "### Available plugins" => {
                in_available_section = true;
            }
            "### How to use skills" | "### How to use plugins" => {
                in_available_section = false;
            }
            line if in_available_section && line.starts_with("- ") => {
                count += 1;
            }
            _ => {}
        }
    }
    count
}

fn starts_with_tag(text: &str, tag: &str) -> bool {
    text.get(..tag.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(tag))
}

fn scale_detail_tokens(details: &mut [DetailAllocation], target_tokens: i64) {
    if details.is_empty() {
        return;
    }
    let total_estimated = details
        .iter()
        .map(|detail| detail.estimated_tokens.max(0))
        .fold(0i64, i64::saturating_add);
    if total_estimated == 0 {
        let last = details.len() - 1;
        for detail in &mut details[..last] {
            detail.estimated_tokens = 0;
        }
        details[last].estimated_tokens = target_tokens.max(0);
        return;
    }

    let mut assigned = 0i64;
    let last = details.len() - 1;
    for detail in &mut details[..last] {
        detail.estimated_tokens = detail
            .estimated_tokens
            .max(0)
            .saturating_mul(target_tokens.max(0))
            / total_estimated;
        assigned = assigned.saturating_add(detail.estimated_tokens);
    }
    details[last].estimated_tokens = target_tokens.max(0).saturating_sub(assigned);
}

fn format_message_label(role: &str, phase: Option<&MessagePhase>) -> String {
    match (role, phase) {
        ("assistant", Some(MessagePhase::Commentary)) => {
            "Assistant message (commentary)".to_string()
        }
        ("assistant", Some(MessagePhase::FinalAnswer)) => "Assistant message (final)".to_string(),
        ("assistant", None) => "Assistant message".to_string(),
        ("user", _) => "User message".to_string(),
        ("system", _) => "System message".to_string(),
        (role, _) => format!("{role} message"),
    }
}

fn section_for_message_role(role: &str) -> ContextSectionKind {
    match role {
        "developer" | "system" => ContextSectionKind::BuiltIn,
        "user" | "assistant" => ContextSectionKind::Conversation,
        _ => ContextSectionKind::Conversation,
    }
}

fn section_order(label: &str) -> usize {
    match label {
        "Built-in" => 0,
        "User" => 1,
        "Conversation" => 2,
        _ => usize::MAX,
    }
}

impl ContextSectionKind {
    fn label(self) -> &'static str {
        match self {
            ContextSectionKind::BuiltIn => "Built-in",
            ContextSectionKind::User => "User",
            ContextSectionKind::Conversation => "Conversation",
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn user_text(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: text.to_string(),
            }],
            end_turn: None,
            phase: None,
        }
    }

    fn developer_text(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "developer".to_string(),
            content: vec![ContentItem::InputText {
                text: text.to_string(),
            }],
            end_turn: None,
            phase: None,
        }
    }

    #[test]
    fn groups_built_in_user_and_conversation_sections() {
        let base_instructions = BaseInstructions {
            text: "Base instructions".to_string(),
        };
        let skill = SKILL_FRAGMENT.into_message(SKILL_FRAGMENT.wrap(
            "<name>notify</name>\n<path>/tmp/skills/notify/SKILL.md</path>\nBody".to_string(),
        ));
        let breakdown = build_context_window_breakdown(
            &[
                developer_text(
                    "<skills_instructions>\n### Available skills\n- notify: send a notification\n### How to use skills\n</skills_instructions>",
                ),
                user_text(
                    "# AGENTS.md instructions for /repo\n\n<INSTRUCTIONS>\nUse focused tests.\n</INSTRUCTIONS>",
                ),
                skill,
                user_text("<environment_context>\n  <cwd>/repo</cwd>\n</environment_context>"),
                user_text("hello"),
                ResponseItem::Reasoning {
                    id: "reasoning-1".to_string(),
                    summary: Vec::new(),
                    content: None,
                    encrypted_content: Some("a".repeat(1000)),
                },
            ],
            &base_instructions,
            Some(1000),
            /*verbose*/ false,
        );

        let mut section_labels: Vec<String> = breakdown
            .sections
            .iter()
            .map(|section| section.label.clone())
            .collect();
        section_labels.sort();
        assert_eq!(
            section_labels,
            vec![
                "Built-in".to_string(),
                "Conversation".to_string(),
                "User".to_string(),
            ]
        );
        assert_eq!(
            breakdown.total_tokens,
            breakdown
                .sections
                .iter()
                .map(|section| section.tokens)
                .sum::<i64>()
        );
        let mut detail_labels = breakdown
            .sections
            .iter()
            .flat_map(|section| section.details.iter())
            .map(|detail| detail.label.clone())
            .collect::<Vec<_>>();
        detail_labels.sort();
        assert_eq!(
            detail_labels,
            vec![
                "AGENTS.md instructions for /repo".to_string(),
                "Base instructions".to_string(),
                "Environment context".to_string(),
                "Implicit skills catalog (1 skills)".to_string(),
                "Reasoning".to_string(),
                "Skill: notify (/tmp/skills/notify/SKILL.md)".to_string(),
                "User message".to_string(),
            ]
        );
    }

    #[test]
    fn verbose_breakdown_keeps_repeated_rows_instead_of_merging() {
        let breakdown = build_context_window_breakdown(
            &[user_text("first"), user_text("second")],
            &BaseInstructions {
                text: String::new(),
            },
            /*model_context_window*/ None,
            /*verbose*/ true,
        );

        let conversation = breakdown
            .sections
            .iter()
            .find(|section| section.label == "Conversation")
            .expect("conversation section");

        assert_eq!(
            conversation
                .details
                .iter()
                .map(|detail| detail.label.clone())
                .collect::<Vec<_>>(),
            vec!["User message".to_string(), "User message".to_string()]
        );
        assert_eq!(
            conversation
                .details
                .iter()
                .map(|detail| detail.tokens)
                .sum::<i64>(),
            conversation.tokens
        );
    }

    #[test]
    fn developer_message_content_items_are_classified_individually() {
        let breakdown = build_context_window_breakdown(
            &[ResponseItem::Message {
                id: None,
                role: "developer".to_string(),
                content: vec![
                    ContentItem::InputText {
                        text: "<permissions instructions>\npolicy".to_string(),
                    },
                    ContentItem::InputText {
                        text: "<skills_instructions>\n### Available skills\n- notify: send a notification\n### How to use skills\n</skills_instructions>"
                            .to_string(),
                    },
                ],
                end_turn: None,
                phase: None,
            }],
            &BaseInstructions {
                text: String::new(),
            },
            /*model_context_window*/ None,
            /*verbose*/ false,
        );

        let mut sections = breakdown
            .sections
            .iter()
            .map(|section| (section.label.clone(), section.tokens))
            .collect::<Vec<_>>();
        sections.sort_by(|left, right| left.0.cmp(&right.0));

        assert_eq!(
            sections
                .iter()
                .map(|(label, _)| label.clone())
                .collect::<Vec<_>>(),
            vec!["Built-in".to_string(), "User".to_string()]
        );
        assert_eq!(
            breakdown.total_tokens,
            breakdown
                .sections
                .iter()
                .map(|section| section.tokens)
                .sum::<i64>()
        );
    }
}
