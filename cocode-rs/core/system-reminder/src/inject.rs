//! Message injection for system reminders.
//!
//! This module provides utilities for injecting system reminders
//! into the message history.

use tracing::debug;

use crate::types::SystemReminder;

/// Injection position for system reminders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectionPosition {
    /// Before the user's message.
    BeforeUserMessage,
    /// After the user's message.
    AfterUserMessage,
    /// At the end of the conversation.
    EndOfConversation,
}

/// Result of injecting reminders.
#[derive(Debug)]
pub struct InjectionResult {
    /// Number of reminders injected.
    pub count: i32,
    /// Position where reminders were injected.
    pub position: InjectionPosition,
}

/// Inject system reminders and return wrapped content.
///
/// This is a simple helper that wraps each reminder in its XML tags
/// and returns them as a list of strings ready to be converted to messages.
///
/// # Arguments
///
/// * `reminders` - The reminders to inject
///
/// # Returns
///
/// A vector of wrapped reminder content strings.
pub fn inject_reminders(reminders: Vec<SystemReminder>) -> Vec<String> {
    let mut result = Vec::with_capacity(reminders.len());

    for reminder in reminders {
        let wrapped = reminder.wrapped_content();
        debug!(
            "Injecting {} reminder ({} bytes)",
            reminder.attachment_type,
            wrapped.len()
        );
        result.push(wrapped);
    }

    result
}

/// Combine multiple reminders into a single message.
///
/// This is useful when you want to inject all reminders as a single
/// user message rather than multiple messages.
pub fn combine_reminders(reminders: Vec<SystemReminder>) -> Option<String> {
    if reminders.is_empty() {
        return None;
    }

    let parts: Vec<String> = reminders.iter().map(|r| r.wrapped_content()).collect();

    Some(parts.join("\n\n"))
}

/// Information about injected reminders for logging/telemetry.
#[derive(Debug, Default)]
pub struct InjectionStats {
    /// Total number of reminders injected.
    pub total_count: i32,
    /// Total byte size of all reminders.
    pub total_bytes: i64,
    /// Breakdown by attachment type.
    pub by_type: std::collections::HashMap<String, i32>,
}

impl InjectionStats {
    /// Create stats from a list of reminders.
    pub fn from_reminders(reminders: &[SystemReminder]) -> Self {
        let mut stats = Self::default();

        for reminder in reminders {
            stats.total_count += 1;
            stats.total_bytes += reminder.content.len() as i64;
            *stats
                .by_type
                .entry(reminder.attachment_type.name().to_string())
                .or_default() += 1;
        }

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AttachmentType;

    fn test_reminder(content: &str) -> SystemReminder {
        SystemReminder::new(AttachmentType::ChangedFiles, content)
    }

    #[test]
    fn test_inject_reminders() {
        let reminders = vec![
            test_reminder("File a.rs changed"),
            test_reminder("File b.rs changed"),
        ];

        let injected = inject_reminders(reminders);
        assert_eq!(injected.len(), 2);
        assert!(injected[0].contains("<system-reminder>"));
        assert!(injected[0].contains("File a.rs changed"));
    }

    #[test]
    fn test_inject_empty() {
        let injected = inject_reminders(vec![]);
        assert!(injected.is_empty());
    }

    #[test]
    fn test_combine_reminders() {
        let reminders = vec![
            test_reminder("First reminder"),
            test_reminder("Second reminder"),
        ];

        let combined = combine_reminders(reminders);
        assert!(combined.is_some());

        let content = combined.expect("content");
        assert!(content.contains("First reminder"));
        assert!(content.contains("Second reminder"));
        assert!(content.contains("\n\n")); // Separated by double newline
    }

    #[test]
    fn test_combine_empty() {
        let combined = combine_reminders(vec![]);
        assert!(combined.is_none());
    }

    #[test]
    fn test_injection_stats() {
        let reminders = vec![
            SystemReminder::new(AttachmentType::ChangedFiles, "change 1"),
            SystemReminder::new(AttachmentType::ChangedFiles, "change 2"),
            SystemReminder::new(AttachmentType::PlanModeEnter, "plan instructions"),
        ];

        let stats = InjectionStats::from_reminders(&reminders);
        assert_eq!(stats.total_count, 3);
        assert_eq!(stats.by_type.get("changed_files"), Some(&2));
        assert_eq!(stats.by_type.get("plan_mode_enter"), Some(&1));
    }
}
