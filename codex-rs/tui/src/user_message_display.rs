//! Display handling for user-message payloads that carry hidden control data.
//!
//! Core can deliver user-message events whose model-visible text includes
//! machine-readable wrappers. The TUI keeps those wrappers out of the transcript
//! by translating them into either visible user content or a compact status
//! message.

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UserMessageDisplay {
    History(String),
    TimerInfo { prompt: String, schedule: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UserMessageDisplayOptions {
    pub(crate) allow_timer_info: bool,
}

pub(crate) fn classify_user_message(
    message: String,
    options: UserMessageDisplayOptions,
) -> UserMessageDisplay {
    if options.allow_timer_info
        && let Some(timer) = parse_timer_fired_user_message(&message)
    {
        return UserMessageDisplay::TimerInfo {
            schedule: timer.schedule(),
            prompt: timer.prompt,
        };
    }

    UserMessageDisplay::History(user_message_history_text(&message).unwrap_or(message))
}

pub(crate) fn user_message_history_text(message: &str) -> Option<String> {
    parse_codex_message_content(message).or_else(|| parse_synthetic_user_message_display(message))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TimerFiredUserMessage {
    prompt: String,
    trigger: Option<String>,
    delivery: Option<String>,
    recurring: bool,
}

impl TimerFiredUserMessage {
    fn schedule(&self) -> String {
        let mut parts = vec!["Running thread timer".to_string()];
        if let Some(trigger) = self
            .trigger
            .as_deref()
            .filter(|trigger| !trigger.is_empty())
        {
            parts.push(trigger.to_string());
        }
        if !self.recurring {
            parts.push("one-shot".to_string());
        }
        if let Some(delivery) = self
            .delivery
            .as_deref()
            .filter(|delivery| !delivery.is_empty())
        {
            parts.push(delivery.to_string());
        }
        parts.join(" • ")
    }
}

fn parse_codex_message_content(message: &str) -> Option<String> {
    let trimmed = message.trim();
    if !trimmed.starts_with("<codex_message>") || !trimmed.ends_with("</codex_message>") {
        return None;
    }

    let content = extract_xml_tag(trimmed, "content")?;
    let content = xml_unescape(content.trim_matches('\n'));
    (!content.is_empty()).then_some(content)
}

fn parse_timer_fired_user_message(message: &str) -> Option<TimerFiredUserMessage> {
    let trimmed = message.trim();
    if !trimmed.starts_with("<timer_fired>") || !trimmed.ends_with("</timer_fired>") {
        return None;
    }

    let prompt = extract_xml_tag(trimmed, "prompt")
        .unwrap_or_default()
        .trim_matches('\n')
        .to_string();
    let trigger = extract_xml_tag(trimmed, "trigger").map(|text| text.trim().to_string());
    let delivery = extract_xml_tag(trimmed, "delivery").map(|text| text.trim().to_string());
    let recurring = extract_xml_tag(trimmed, "recurring")
        .as_deref()
        .map(str::trim)
        == Some("true");

    Some(TimerFiredUserMessage {
        prompt,
        trigger,
        delivery,
        recurring,
    })
}

fn parse_synthetic_user_message_display(message: &str) -> Option<String> {
    let trimmed = message.trim();
    if !trimmed.starts_with("<codex_tui_synthetic_user_message>")
        || !trimmed.ends_with("</codex_tui_synthetic_user_message>")
    {
        return None;
    }

    let display = extract_xml_tag(trimmed, "display")?;
    let display = xml_unescape(display.trim());
    (!display.is_empty()).then_some(display)
}

fn extract_xml_tag(message: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let after_open = message.split_once(&open)?.1;
    let value = after_open.split_once(&close)?.0;
    Some(value.to_string())
}

fn xml_unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn codex_message_history_text_uses_content_only() {
        assert_eq!(
            user_message_history_text(
                "<codex_message>
<source>timer timer-1</source>
<queued_at>100</queued_at>
<content>
Run &lt;tests&gt;.
</content>
<instructions>
Hidden
</instructions>
<meta />
</codex_message>"
            ),
            Some("Run <tests>.".to_string())
        );
    }

    #[test]
    fn synthetic_message_history_text_uses_display_only() {
        assert_eq!(
            user_message_history_text(
                "<codex_tui_synthetic_user_message><display>/review src</display><prompt>hidden</prompt></codex_tui_synthetic_user_message>"
            ),
            Some("/review src".to_string())
        );
    }

    #[test]
    fn timer_fired_can_render_as_info_message() {
        assert_eq!(
            classify_user_message(
                "<timer_fired><prompt>Run tests</prompt><trigger>every minute</trigger><delivery>after-turn</delivery><recurring>true</recurring></timer_fired>"
                    .to_string(),
                UserMessageDisplayOptions {
                    allow_timer_info: true
                },
            ),
            UserMessageDisplay::TimerInfo {
                prompt: "Run tests".to_string(),
                schedule: "Running thread timer • every minute • after-turn".to_string(),
            }
        );
    }

    #[test]
    fn timer_fired_can_remain_history_text_when_info_is_not_allowed() {
        let message = "<timer_fired><prompt>Run tests</prompt></timer_fired>".to_string();
        assert_eq!(
            classify_user_message(
                message.clone(),
                UserMessageDisplayOptions {
                    allow_timer_info: false
                },
            ),
            UserMessageDisplay::History(message)
        );
    }
}
