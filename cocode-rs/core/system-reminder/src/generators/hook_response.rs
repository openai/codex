//! Hook response generators for system reminders.
//!
//! This module provides generators for injecting hook-related context into
//! the conversation, such as:
//! - Results from async hooks that completed in the background
//! - Context added by hooks (additional_context field)
//! - Error information when hooks block execution

use async_trait::async_trait;

use crate::Result;
use crate::config::SystemReminderConfig;
use crate::generator::AttachmentGenerator;
use crate::generator::GeneratorContext;
use crate::throttle::ThrottleConfig;
use crate::types::AttachmentType;
use crate::types::ReminderTier;
use crate::types::SystemReminder;

/// Information about a completed async hook response.
#[derive(Debug, Clone)]
pub struct AsyncHookResponseInfo {
    /// Name of the hook that completed.
    pub hook_name: String,
    /// The additional context returned by the hook.
    pub additional_context: Option<String>,
    /// Whether the hook blocked execution.
    pub was_blocking: bool,
    /// Reason for blocking (if was_blocking is true).
    pub blocking_reason: Option<String>,
    /// Execution duration in milliseconds.
    pub duration_ms: i64,
}

/// Information about hook context to inject.
#[derive(Debug, Clone)]
pub struct HookContextInfo {
    /// Name of the hook.
    pub hook_name: String,
    /// Event type (e.g., "pre_tool_use").
    pub event_type: String,
    /// Tool name if applicable.
    pub tool_name: Option<String>,
    /// Additional context from the hook.
    pub additional_context: String,
}

/// Information about a hook that blocked execution.
#[derive(Debug, Clone)]
pub struct HookBlockingInfo {
    /// Name of the hook that blocked.
    pub hook_name: String,
    /// Event type (e.g., "pre_tool_use").
    pub event_type: String,
    /// Tool name that was blocked.
    pub tool_name: Option<String>,
    /// Reason for blocking.
    pub reason: String,
}

/// Extension key for async hook responses.
pub const ASYNC_HOOK_RESPONSES_KEY: &str = "async_hook_responses";
/// Extension key for hook context to inject.
pub const HOOK_CONTEXT_KEY: &str = "hook_context";
/// Extension key for hook blocking errors.
pub const HOOK_BLOCKING_KEY: &str = "hook_blocking";

/// Generator for async hook responses.
///
/// This generator injects context from hooks that completed in the background
/// or from hooks that returned additional_context.
#[derive(Debug, Default)]
pub struct AsyncHookResponseGenerator;

#[async_trait]
impl AttachmentGenerator for AsyncHookResponseGenerator {
    fn name(&self) -> &str {
        "async_hook_response"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::AsyncHookResponse
    }

    fn tier(&self) -> ReminderTier {
        ReminderTier::MainAgentOnly
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        // Get async hook responses from extension data
        let responses = ctx
            .extension_data
            .get(ASYNC_HOOK_RESPONSES_KEY)
            .and_then(|v| v.downcast_ref::<Vec<AsyncHookResponseInfo>>());

        let responses = match responses {
            Some(r) if !r.is_empty() => r,
            _ => return Ok(None),
        };

        let mut content = String::from("# Async Hook Results\n\n");
        content.push_str("The following hooks completed in the background:\n\n");

        for response in responses {
            content.push_str(&format!("## Hook: {}\n", response.hook_name));
            content.push_str(&format!("- Duration: {}ms\n", response.duration_ms));

            if response.was_blocking {
                content.push_str("- **BLOCKED** execution\n");
                if let Some(reason) = &response.blocking_reason {
                    content.push_str(&format!("- Reason: {reason}\n"));
                }
            }

            if let Some(context) = &response.additional_context {
                content.push_str(&format!("\n### Additional Context\n{context}\n"));
            }

            content.push('\n');
        }

        Ok(Some(SystemReminder::new(
            AttachmentType::AsyncHookResponse,
            content,
        )))
    }

    fn is_enabled(&self, _config: &SystemReminderConfig) -> bool {
        true // Always enabled, generates nothing if no responses
    }

    fn throttle_config(&self) -> ThrottleConfig {
        ThrottleConfig::none() // No throttling - inject whenever available
    }
}

/// Generator for hook additional context.
///
/// This generator injects additional context provided by hooks via
/// the `ContinueWithContext` result.
#[derive(Debug, Default)]
pub struct HookAdditionalContextGenerator;

#[async_trait]
impl AttachmentGenerator for HookAdditionalContextGenerator {
    fn name(&self) -> &str {
        "hook_additional_context"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::HookAdditionalContext
    }

    fn tier(&self) -> ReminderTier {
        ReminderTier::MainAgentOnly
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        // Get hook context from extension data
        let contexts = ctx
            .extension_data
            .get(HOOK_CONTEXT_KEY)
            .and_then(|v| v.downcast_ref::<Vec<HookContextInfo>>());

        let contexts = match contexts {
            Some(c) if !c.is_empty() => c,
            _ => return Ok(None),
        };

        let mut content = String::from("# Hook Context\n\n");
        content.push_str("The following hooks added context:\n\n");

        for info in contexts {
            content.push_str(&format!("## From hook: {}\n", info.hook_name));
            content.push_str(&format!("- Event: {}\n", info.event_type));
            if let Some(tool) = &info.tool_name {
                content.push_str(&format!("- Tool: {tool}\n"));
            }
            content.push_str(&format!("\n{}\n\n", info.additional_context));
        }

        Ok(Some(SystemReminder::new(
            AttachmentType::HookAdditionalContext,
            content,
        )))
    }

    fn is_enabled(&self, _config: &SystemReminderConfig) -> bool {
        true
    }

    fn throttle_config(&self) -> ThrottleConfig {
        ThrottleConfig::none()
    }
}

/// Generator for hook blocking errors.
///
/// This generator injects information about hooks that blocked execution,
/// helping the model understand why an action was rejected.
#[derive(Debug, Default)]
pub struct HookBlockingErrorGenerator;

#[async_trait]
impl AttachmentGenerator for HookBlockingErrorGenerator {
    fn name(&self) -> &str {
        "hook_blocking_error"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::HookBlockingError
    }

    fn tier(&self) -> ReminderTier {
        ReminderTier::MainAgentOnly
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        // Get hook blocking info from extension data
        let blocking = ctx
            .extension_data
            .get(HOOK_BLOCKING_KEY)
            .and_then(|v| v.downcast_ref::<Vec<HookBlockingInfo>>());

        let blocking = match blocking {
            Some(b) if !b.is_empty() => b,
            _ => return Ok(None),
        };

        let mut content = String::from("# Hook Blocked Execution\n\n");
        content.push_str(
            "The following hooks blocked execution. Review the reasons and adjust your approach:\n\n",
        );

        for info in blocking {
            content.push_str(&format!("## Hook: {}\n", info.hook_name));
            content.push_str(&format!("- Event: {}\n", info.event_type));
            if let Some(tool) = &info.tool_name {
                content.push_str(&format!("- Tool: {tool}\n"));
            }
            content.push_str(&format!("- **Reason**: {}\n\n", info.reason));
        }

        Ok(Some(SystemReminder::new(
            AttachmentType::HookBlockingError,
            content,
        )))
    }

    fn is_enabled(&self, _config: &SystemReminderConfig) -> bool {
        true
    }

    fn throttle_config(&self) -> ThrottleConfig {
        ThrottleConfig::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_config() -> SystemReminderConfig {
        SystemReminderConfig::default()
    }

    fn make_ctx_with_extension<T: Send + Sync + 'static>(
        key: &str,
        value: T,
    ) -> GeneratorContext<'static> {
        let config = Box::leak(Box::new(test_config()));
        GeneratorContext::builder()
            .config(config)
            .turn_number(1)
            .is_main_agent(true)
            .cwd(PathBuf::from("/tmp"))
            .extension(key, value)
            .build()
    }

    #[tokio::test]
    async fn test_async_hook_response_empty() {
        let ctx =
            make_ctx_with_extension::<Vec<AsyncHookResponseInfo>>(ASYNC_HOOK_RESPONSES_KEY, vec![]);
        let generator = AsyncHookResponseGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_async_hook_response_with_data() {
        let responses = vec![AsyncHookResponseInfo {
            hook_name: "test-hook".to_string(),
            additional_context: Some("Test context".to_string()),
            was_blocking: false,
            blocking_reason: None,
            duration_ms: 100,
        }];

        let ctx = make_ctx_with_extension(ASYNC_HOOK_RESPONSES_KEY, responses);
        let generator = AsyncHookResponseGenerator;
        let result = generator.generate(&ctx).await.expect("generate");

        assert!(result.is_some());
        let reminder = result.unwrap();
        assert_eq!(reminder.attachment_type, AttachmentType::AsyncHookResponse);
        assert!(reminder.content().unwrap().contains("test-hook"));
        assert!(reminder.content().unwrap().contains("Test context"));
    }

    #[tokio::test]
    async fn test_hook_blocking_generator() {
        let blocking = vec![HookBlockingInfo {
            hook_name: "security-check".to_string(),
            event_type: "pre_tool_use".to_string(),
            tool_name: Some("bash".to_string()),
            reason: "Command not allowed".to_string(),
        }];

        let ctx = make_ctx_with_extension(HOOK_BLOCKING_KEY, blocking);
        let generator = HookBlockingErrorGenerator;
        let result = generator.generate(&ctx).await.expect("generate");

        assert!(result.is_some());
        let reminder = result.unwrap();
        assert_eq!(reminder.attachment_type, AttachmentType::HookBlockingError);
        assert!(reminder.content().unwrap().contains("security-check"));
        assert!(reminder.content().unwrap().contains("Command not allowed"));
    }

    #[tokio::test]
    async fn test_hook_context_generator() {
        let contexts = vec![HookContextInfo {
            hook_name: "context-hook".to_string(),
            event_type: "session_start".to_string(),
            tool_name: None,
            additional_context: "Session initialized with defaults".to_string(),
        }];

        let ctx = make_ctx_with_extension(HOOK_CONTEXT_KEY, contexts);
        let generator = HookAdditionalContextGenerator;
        let result = generator.generate(&ctx).await.expect("generate");

        assert!(result.is_some());
        let reminder = result.unwrap();
        assert_eq!(
            reminder.attachment_type,
            AttachmentType::HookAdditionalContext
        );
        assert!(reminder.content().unwrap().contains("context-hook"));
        assert!(reminder.content().unwrap().contains("Session initialized"));
    }

    #[test]
    fn test_generator_names() {
        let gen1 = AsyncHookResponseGenerator;
        let gen2 = HookAdditionalContextGenerator;
        let gen3 = HookBlockingErrorGenerator;

        assert_eq!(gen1.name(), "async_hook_response");
        assert_eq!(gen2.name(), "hook_additional_context");
        assert_eq!(gen3.name(), "hook_blocking_error");
    }

    #[test]
    fn test_generator_tiers() {
        let gen1 = AsyncHookResponseGenerator;
        let gen2 = HookAdditionalContextGenerator;
        let gen3 = HookBlockingErrorGenerator;

        assert_eq!(gen1.tier(), ReminderTier::MainAgentOnly);
        assert_eq!(gen2.tier(), ReminderTier::MainAgentOnly);
        assert_eq!(gen3.tier(), ReminderTier::MainAgentOnly);
    }
}
