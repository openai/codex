//! Hook registry for storing and dispatching hooks.
//!
//! The `HookRegistry` is the central coordinator: it stores all registered
//! hooks and, when an event occurs, finds the matching hooks and executes them.

use std::time::Instant;

use tracing::{info, warn};

use crate::context::HookContext;
use crate::definition::{HookDefinition, HookHandler};
use crate::event::HookEventType;
use crate::handlers;
use crate::result::{HookOutcome, HookResult};

/// Central registry that stores hooks and dispatches events.
#[derive(Default)]
pub struct HookRegistry {
    hooks: Vec<HookDefinition>,
}

impl HookRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    /// Registers a hook definition.
    pub fn register(&mut self, hook: HookDefinition) {
        info!(name = %hook.name, event = %hook.event_type, "Registered hook");
        self.hooks.push(hook);
    }

    /// Returns all hooks registered for a given event type.
    pub fn hooks_for_event(&self, event_type: &HookEventType) -> Vec<&HookDefinition> {
        self.hooks
            .iter()
            .filter(|h| h.enabled && h.event_type == *event_type)
            .collect()
    }

    /// Executes all matching hooks for the given context.
    ///
    /// Returns outcomes in registration order. A hook matches if:
    /// 1. Its event type equals the context event type.
    /// 2. It is enabled.
    /// 3. Its matcher (if any) matches the context tool name (or no matcher is set).
    pub async fn execute(&self, ctx: &HookContext) -> Vec<HookOutcome> {
        let matching: Vec<&HookDefinition> = self
            .hooks
            .iter()
            .filter(|h| h.enabled && h.event_type == ctx.event_type)
            .filter(|h| {
                match (&h.matcher, &ctx.tool_name) {
                    (Some(matcher), Some(tool)) => matcher.matches(tool),
                    (Some(_), None) => false, // matcher present but no tool name to match against
                    (None, _) => true,        // no matcher means always match
                }
            })
            .collect();

        let mut outcomes = Vec::with_capacity(matching.len());

        for hook in matching {
            let start = Instant::now();

            let timeout = tokio::time::Duration::from_secs(hook.timeout_secs as u64);
            let result = tokio::time::timeout(timeout, execute_handler(&hook.handler, ctx)).await;

            let duration_ms = start.elapsed().as_millis() as i64;

            let result = match result {
                Ok(r) => r,
                Err(_) => {
                    warn!(
                        hook_name = %hook.name,
                        timeout_secs = hook.timeout_secs,
                        "Hook timed out"
                    );
                    HookResult::Continue
                }
            };

            info!(
                hook_name = %hook.name,
                duration_ms,
                "Hook executed"
            );

            outcomes.push(HookOutcome {
                hook_name: hook.name.clone(),
                result,
                duration_ms,
            });
        }

        outcomes
    }

    /// Removes all registered hooks.
    pub fn clear(&mut self) {
        self.hooks.clear();
    }

    /// Returns the number of registered hooks.
    pub fn len(&self) -> usize {
        self.hooks.len()
    }

    /// Returns `true` if no hooks are registered.
    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty()
    }
}

/// Dispatches execution to the appropriate handler.
async fn execute_handler(handler: &HookHandler, ctx: &HookContext) -> HookResult {
    match handler {
        HookHandler::Command { command, args } => {
            let input = ctx
                .tool_input
                .as_ref()
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            handlers::command::CommandHandler::execute(command, args, &input, &ctx.working_dir)
                .await
        }
        HookHandler::Prompt { template } => {
            let arguments = ctx
                .tool_input
                .as_ref()
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            handlers::prompt::PromptHandler::execute(template, &arguments)
        }
        HookHandler::Agent { max_turns } => handlers::agent::AgentHandler::execute(*max_turns),
        HookHandler::Webhook { url } => handlers::webhook::WebhookHandler::execute(url, ctx).await,
        HookHandler::Inline => {
            warn!("Inline handler cannot be dispatched through the registry");
            HookResult::Continue
        }
    }
}

impl std::fmt::Debug for HookRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookRegistry")
            .field("hooks_count", &self.hooks.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::HookHandler;
    use crate::matcher::HookMatcher;
    use std::path::PathBuf;

    fn make_ctx(event: HookEventType, tool_name: Option<&str>) -> HookContext {
        let mut ctx = HookContext::new(event, "test-session".to_string(), PathBuf::from("/tmp"));
        if let Some(name) = tool_name {
            ctx.tool_name = Some(name.to_string());
        }
        ctx
    }

    fn make_hook(name: &str, event: HookEventType, matcher: Option<HookMatcher>) -> HookDefinition {
        HookDefinition {
            name: name.to_string(),
            event_type: event,
            matcher,
            handler: HookHandler::Prompt {
                template: "test".to_string(),
            },
            enabled: true,
            timeout_secs: 30,
        }
    }

    #[test]
    fn test_register_and_len() {
        let mut registry = HookRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        registry.register(make_hook("h1", HookEventType::PreToolUse, None));
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_hooks_for_event() {
        let mut registry = HookRegistry::new();
        registry.register(make_hook("h1", HookEventType::PreToolUse, None));
        registry.register(make_hook("h2", HookEventType::PostToolUse, None));
        registry.register(make_hook("h3", HookEventType::PreToolUse, None));

        let pre = registry.hooks_for_event(&HookEventType::PreToolUse);
        assert_eq!(pre.len(), 2);
        assert_eq!(pre[0].name, "h1");
        assert_eq!(pre[1].name, "h3");

        let post = registry.hooks_for_event(&HookEventType::PostToolUse);
        assert_eq!(post.len(), 1);

        let start = registry.hooks_for_event(&HookEventType::SessionStart);
        assert!(start.is_empty());
    }

    #[test]
    fn test_disabled_hooks_excluded() {
        let mut registry = HookRegistry::new();
        let mut hook = make_hook("disabled", HookEventType::PreToolUse, None);
        hook.enabled = false;
        registry.register(hook);

        assert!(
            registry
                .hooks_for_event(&HookEventType::PreToolUse)
                .is_empty()
        );
    }

    #[test]
    fn test_clear() {
        let mut registry = HookRegistry::new();
        registry.register(make_hook("h1", HookEventType::PreToolUse, None));
        registry.register(make_hook("h2", HookEventType::PostToolUse, None));
        assert_eq!(registry.len(), 2);

        registry.clear();
        assert!(registry.is_empty());
    }

    #[tokio::test]
    async fn test_execute_no_matching_hooks() {
        let registry = HookRegistry::new();
        let ctx = make_ctx(HookEventType::SessionStart, None);
        let outcomes = registry.execute(&ctx).await;
        assert!(outcomes.is_empty());
    }

    #[tokio::test]
    async fn test_execute_with_matcher() {
        let mut registry = HookRegistry::new();
        registry.register(make_hook(
            "bash-only",
            HookEventType::PreToolUse,
            Some(HookMatcher::Exact {
                value: "bash".to_string(),
            }),
        ));

        // Should match
        let ctx = make_ctx(HookEventType::PreToolUse, Some("bash"));
        let outcomes = registry.execute(&ctx).await;
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].hook_name, "bash-only");

        // Should not match
        let ctx = make_ctx(HookEventType::PreToolUse, Some("python"));
        let outcomes = registry.execute(&ctx).await;
        assert!(outcomes.is_empty());
    }

    #[tokio::test]
    async fn test_execute_matcher_without_tool_name() {
        let mut registry = HookRegistry::new();
        registry.register(make_hook(
            "need-tool",
            HookEventType::PreToolUse,
            Some(HookMatcher::All),
        ));

        // No tool name in context but matcher exists => no match
        let ctx = make_ctx(HookEventType::PreToolUse, None);
        let outcomes = registry.execute(&ctx).await;
        assert!(outcomes.is_empty());
    }

    #[tokio::test]
    async fn test_execute_no_matcher_always_matches() {
        let mut registry = HookRegistry::new();
        registry.register(make_hook("always", HookEventType::SessionStart, None));

        let ctx = make_ctx(HookEventType::SessionStart, None);
        let outcomes = registry.execute(&ctx).await;
        assert_eq!(outcomes.len(), 1);
    }
}
