//! Inline handler type.
//!
//! Allows registering Rust closures as hook handlers. Inline handlers are not
//! serializable and can only be registered programmatically.

use crate::context::HookContext;
use crate::result::HookResult;

/// A function-based hook handler.
///
/// This type alias allows registering closures as hook handlers. The closure
/// receives a `HookContext` and returns a `HookResult`.
///
/// Inline handlers are not serializable and must be registered through the
/// `HookRegistry` API (not via TOML config).
pub type InlineHandler = Box<dyn Fn(&HookContext) -> HookResult + Send + Sync>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::HookEventType;
    use std::path::PathBuf;

    #[test]
    fn test_inline_handler() {
        let handler: InlineHandler = Box::new(|ctx| {
            if ctx.tool_name.as_deref() == Some("bash") {
                HookResult::Reject {
                    reason: "bash is not allowed".to_string(),
                }
            } else {
                HookResult::Continue
            }
        });

        let ctx = HookContext::new(
            HookEventType::PreToolUse,
            "s1".to_string(),
            PathBuf::from("/tmp"),
        )
        .with_tool_name("bash");
        let result = handler(&ctx);
        assert!(matches!(result, HookResult::Reject { .. }));

        let ctx2 = HookContext::new(
            HookEventType::PreToolUse,
            "s1".to_string(),
            PathBuf::from("/tmp"),
        )
        .with_tool_name("read_file");
        let result2 = handler(&ctx2);
        assert!(matches!(result2, HookResult::Continue));
    }
}
