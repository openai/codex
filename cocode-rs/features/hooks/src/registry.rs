//! Hook registry for storing and dispatching hooks.
//!
//! The `HookRegistry` is the central coordinator: it stores all registered
//! hooks and, when an event occurs, finds the matching hooks and executes them.

use std::collections::HashSet;
use std::sync::RwLock;
use std::time::Instant;

use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::context::HookContext;
use crate::definition::HookDefinition;
use crate::definition::HookHandler;
use crate::event::HookEventType;
use crate::handlers;
use crate::result::HookOutcome;
use crate::result::HookResult;

/// Central registry that stores hooks and dispatches events.
///
/// The registry supports one-shot hooks (`once: true`) which are automatically
/// removed after successful execution.
///
/// This registry uses interior mutability (`RwLock`) to allow execution through
/// shared references (`Arc<HookRegistry>`), which is needed for concurrent access
/// from the executor.
pub struct HookRegistry {
    hooks: RwLock<Vec<HookDefinition>>,
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HookRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self {
            hooks: RwLock::new(Vec::new()),
        }
    }

    /// Registers a hook definition.
    pub fn register(&self, hook: HookDefinition) {
        info!(
            name = %hook.name,
            event = %hook.event_type,
            once = hook.once,
            "Registered hook"
        );
        if let Ok(mut hooks) = self.hooks.write() {
            hooks.push(hook);
        }
    }

    /// Registers multiple hook definitions.
    pub fn register_all(&self, hooks: impl IntoIterator<Item = HookDefinition>) {
        for hook in hooks {
            self.register(hook);
        }
    }

    /// Returns all hooks registered for a given event type.
    pub fn hooks_for_event(&self, event_type: &HookEventType) -> Vec<HookDefinition> {
        if let Ok(hooks) = self.hooks.read() {
            hooks
                .iter()
                .filter(|h| h.enabled && h.event_type == *event_type)
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Executes all matching hooks for the given context.
    ///
    /// Returns outcomes in registration order. A hook matches if:
    /// 1. Its event type equals the context event type.
    /// 2. It is enabled.
    /// 3. Its matcher (if any) matches the context tool name (or no matcher is set).
    ///
    /// One-shot hooks (`once: true`) are removed after successful execution.
    /// They are NOT removed on timeout or failure, allowing retry.
    pub async fn execute(&self, ctx: &HookContext) -> Vec<HookOutcome> {
        // Get matching hooks (clone to release lock during execution)
        let matching: Vec<HookDefinition> = if let Ok(hooks) = self.hooks.read() {
            hooks
                .iter()
                .filter(|h| h.enabled && h.event_type == ctx.event_type)
                .filter(|h| {
                    match (&h.matcher, &ctx.tool_name) {
                        (Some(matcher), Some(tool)) => matcher.matches(tool),
                        (Some(_), None) => false, // matcher present but no tool name to match against
                        (None, _) => true,        // no matcher means always match
                    }
                })
                .cloned()
                .collect()
        } else {
            return Vec::new();
        };

        let mut outcomes = Vec::with_capacity(matching.len());
        let mut once_hooks_to_remove: Vec<String> = Vec::new();

        for hook in &matching {
            let start = Instant::now();

            let timeout = tokio::time::Duration::from_secs(hook.timeout_secs as u64);
            let result = tokio::time::timeout(timeout, execute_handler(&hook.handler, ctx)).await;

            let duration_ms = start.elapsed().as_millis() as i64;

            let (result, is_success) = match result {
                Ok(r) => {
                    // Consider it a success unless it's a Reject
                    let success = !matches!(r, HookResult::Reject { .. });
                    (r, success)
                }
                Err(_) => {
                    warn!(
                        hook_name = %hook.name,
                        timeout_secs = hook.timeout_secs,
                        "Hook timed out"
                    );
                    (HookResult::Continue, false) // Timeout is not a success
                }
            };

            info!(
                hook_name = %hook.name,
                duration_ms,
                once = hook.once,
                success = is_success,
                "Hook executed"
            );

            // Mark one-shot hook for removal if successful
            if hook.once && is_success {
                debug!(hook_name = %hook.name, "One-shot hook will be removed");
                once_hooks_to_remove.push(hook.name.clone());
            }

            outcomes.push(HookOutcome {
                hook_name: hook.name.clone(),
                result,
                duration_ms,
            });
        }

        // Remove one-shot hooks that executed successfully
        if !once_hooks_to_remove.is_empty() {
            self.remove_hooks_by_name(&once_hooks_to_remove);
        }

        outcomes
    }

    /// Removes hooks by their names.
    fn remove_hooks_by_name(&self, names: &[String]) {
        let names_set: HashSet<_> = names.iter().collect();
        if let Ok(mut hooks) = self.hooks.write() {
            let before = hooks.len();
            hooks.retain(|h| !names_set.contains(&h.name));
            let removed = before - hooks.len();
            if removed > 0 {
                info!(
                    count = removed,
                    "Removed one-shot hooks after successful execution"
                );
            }
        }
    }

    /// Removes all hooks from a specific source (e.g., when a skill ends).
    pub fn remove_hooks_by_source_name(&self, source_name: &str) {
        if let Ok(mut hooks) = self.hooks.write() {
            let before = hooks.len();
            hooks.retain(|h| h.source.name() != Some(source_name));
            let removed = before - hooks.len();
            if removed > 0 {
                info!(
                    source = source_name,
                    count = removed,
                    "Removed hooks by source"
                );
            }
        }
    }

    /// Removes all hooks with the specified scope.
    pub fn remove_hooks_by_scope(&self, scope: crate::scope::HookScope) {
        if let Ok(mut hooks) = self.hooks.write() {
            let before = hooks.len();
            hooks.retain(|h| h.source.scope() != scope);
            let removed = before - hooks.len();
            if removed > 0 {
                info!(scope = %scope, count = removed, "Removed hooks by scope");
            }
        }
    }

    /// Removes all registered hooks.
    pub fn clear(&self) {
        if let Ok(mut hooks) = self.hooks.write() {
            hooks.clear();
        }
    }

    /// Returns the number of registered hooks.
    pub fn len(&self) -> usize {
        self.hooks.read().map(|h| h.len()).unwrap_or(0)
    }

    /// Returns `true` if no hooks are registered.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a copy of all registered hooks.
    pub fn all_hooks(&self) -> Vec<HookDefinition> {
        self.hooks.read().map(|h| h.clone()).unwrap_or_default()
    }
}

/// Dispatches execution to the appropriate handler.
async fn execute_handler(handler: &HookHandler, ctx: &HookContext) -> HookResult {
    match handler {
        HookHandler::Command { command, args } => {
            // Pass full HookContext to command handler for env vars and stdin JSON
            handlers::command::CommandHandler::execute(command, args, ctx).await
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
            .field("hooks_count", &self.len())
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
            source: Default::default(),
            enabled: true,
            timeout_secs: 30,
            once: false,
        }
    }

    fn make_once_hook(name: &str, event: HookEventType) -> HookDefinition {
        HookDefinition {
            name: name.to_string(),
            event_type: event,
            matcher: None,
            handler: HookHandler::Prompt {
                template: "test".to_string(),
            },
            source: Default::default(),
            enabled: true,
            timeout_secs: 30,
            once: true,
        }
    }

    #[test]
    fn test_register_and_len() {
        let registry = HookRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        registry.register(make_hook("h1", HookEventType::PreToolUse, None));
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_hooks_for_event() {
        let registry = HookRegistry::new();
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
        let registry = HookRegistry::new();
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
        let registry = HookRegistry::new();
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
        let registry = HookRegistry::new();
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
        let registry = HookRegistry::new();
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
        let registry = HookRegistry::new();
        registry.register(make_hook("always", HookEventType::SessionStart, None));

        let ctx = make_ctx(HookEventType::SessionStart, None);
        let outcomes = registry.execute(&ctx).await;
        assert_eq!(outcomes.len(), 1);
    }

    #[tokio::test]
    async fn test_once_hook_removed_after_success() {
        let registry = HookRegistry::new();
        registry.register(make_once_hook("one-shot", HookEventType::SessionStart));

        assert_eq!(registry.len(), 1);

        // First execution - hook should run and be removed
        let ctx = make_ctx(HookEventType::SessionStart, None);
        let outcomes = registry.execute(&ctx).await;
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].hook_name, "one-shot");

        // Hook should be removed after successful execution
        assert_eq!(registry.len(), 0);

        // Second execution - no hook should run
        let outcomes = registry.execute(&ctx).await;
        assert!(outcomes.is_empty());
    }

    #[tokio::test]
    async fn test_regular_hook_not_removed() {
        let registry = HookRegistry::new();
        registry.register(make_hook("regular", HookEventType::SessionStart, None));

        assert_eq!(registry.len(), 1);

        let ctx = make_ctx(HookEventType::SessionStart, None);

        // First execution
        let outcomes = registry.execute(&ctx).await;
        assert_eq!(outcomes.len(), 1);

        // Hook should still exist
        assert_eq!(registry.len(), 1);

        // Second execution - hook should still run
        let outcomes = registry.execute(&ctx).await;
        assert_eq!(outcomes.len(), 1);
    }

    #[test]
    fn test_remove_hooks_by_source_name() {
        let registry = HookRegistry::new();
        let mut h1 = make_hook("h1", HookEventType::PreToolUse, None);
        h1.source = crate::scope::HookSource::Skill {
            name: "my-skill".to_string(),
        };
        let mut h2 = make_hook("h2", HookEventType::PreToolUse, None);
        h2.source = crate::scope::HookSource::Skill {
            name: "other-skill".to_string(),
        };
        let h3 = make_hook("h3", HookEventType::PreToolUse, None); // Session source

        registry.register(h1);
        registry.register(h2);
        registry.register(h3);

        assert_eq!(registry.len(), 3);

        registry.remove_hooks_by_source_name("my-skill");

        assert_eq!(registry.len(), 2);
        let hooks = registry.all_hooks();
        assert!(hooks.iter().all(|h| h.name != "h1"));
    }

    #[test]
    fn test_remove_hooks_by_scope() {
        let registry = HookRegistry::new();
        let mut h1 = make_hook("h1", HookEventType::PreToolUse, None);
        h1.source = crate::scope::HookSource::Skill {
            name: "skill".to_string(),
        };
        let mut h2 = make_hook("h2", HookEventType::PreToolUse, None);
        h2.source = crate::scope::HookSource::Policy;
        let h3 = make_hook("h3", HookEventType::PreToolUse, None); // Session source

        registry.register(h1);
        registry.register(h2);
        registry.register(h3);

        assert_eq!(registry.len(), 3);

        registry.remove_hooks_by_scope(crate::scope::HookScope::Session);

        assert_eq!(registry.len(), 2);
        let hooks = registry.all_hooks();
        assert!(hooks.iter().all(|h| h.name != "h3"));
    }
}
