//! Callback hook executor.
//!
//! Executes native Rust callbacks aligned with Claude Code's callback hook pattern.
//!
//! ## Callback Signature
//!
//! Aligned with Claude Code's callback signature:
//! ```typescript
//! callback: (hookInput, toolUseID, signal, hookIndex) => Promise<HookOutput>
//! ```
//!
//! ## Key Differences from Command Hooks
//!
//! - Callbacks are NEVER deduplicated (all execute)
//! - Timeout is in milliseconds (command hooks use seconds)
//! - No exit code semantics - errors are returned directly

use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use futures::future::BoxFuture;
use tokio_util::sync::CancellationToken;
use tracing::debug;

use crate::error::HookError;
use crate::input::HookInput;
use crate::output::HookOutcome;
use crate::output::HookOutput;
use crate::output::HookResult;
use crate::types::HookCallback;

/// Default timeout for callback execution (60 seconds).
const DEFAULT_TIMEOUT_MS: u64 = 60_000;

/// Execute a native callback hook.
///
/// # Arguments
///
/// * `callback` - The callback implementation
/// * `input` - The hook input
/// * `tool_use_id` - Optional tool use ID for tool-related events
/// * `cancel` - Cancellation token for timeout/abort handling
/// * `hook_index` - Index of this hook in the execution order
/// * `timeout_ms` - Optional timeout in milliseconds
pub async fn execute_callback(
    callback: &dyn HookCallback,
    input: HookInput,
    tool_use_id: Option<String>,
    cancel: CancellationToken,
    hook_index: i32,
    timeout_ms: Option<u64>,
) -> Result<HookResult, HookError> {
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS));

    debug!(
        hook_index,
        timeout_ms = timeout.as_millis(),
        "Executing callback hook"
    );

    let result = tokio::select! {
        _ = cancel.cancelled() => {
            return Ok(HookResult::cancelled());
        }
        result = tokio::time::timeout(
            timeout,
            callback.execute(input, tool_use_id, cancel.clone(), hook_index)
        ) => {
            match result {
                Ok(Ok(output)) => output,
                Ok(Err(e)) => return Err(e),
                Err(_) => return Err(HookError::Timeout),
            }
        }
    };

    Ok(HookResult {
        outcome: if result.is_blocking() {
            HookOutcome::Blocking
        } else {
            HookOutcome::Success
        },
        output: Some(result),
        blocking_error: None,
        stdout: None,
        stderr: None,
        exit_code: None,
    })
}

/// Create a callback from a closure.
///
/// This is a convenience function for creating callbacks without implementing
/// the `HookCallback` trait manually.
///
/// # Example
///
/// ```rust,ignore
/// use codex_hooks::{callback_from_fn, HookInput, HookOutput, HookError};
///
/// let callback = callback_from_fn(|input, _tool_use_id, _cancel, _index| async move {
///     println!("Hook triggered: {:?}", input.hook_event_name);
///     Ok(HookOutput::default())
/// });
/// ```
pub fn callback_from_fn<F, Fut>(f: F) -> impl HookCallback
where
    F: Fn(HookInput, Option<String>, CancellationToken, i32) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<HookOutput, HookError>> + Send + 'static,
{
    ClosureCallback {
        f: Arc::new(f),
        name: None,
    }
}

/// Create a named callback from a closure.
///
/// The name is used for debugging and logging purposes.
pub fn callback_from_fn_named<F, Fut>(name: impl Into<String>, f: F) -> impl HookCallback
where
    F: Fn(HookInput, Option<String>, CancellationToken, i32) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<HookOutput, HookError>> + Send + 'static,
{
    ClosureCallback {
        f: Arc::new(f),
        name: Some(name.into()),
    }
}

/// Wrapper for closure-based callbacks.
struct ClosureCallback<F> {
    f: Arc<F>,
    name: Option<String>,
}

impl<F, Fut> HookCallback for ClosureCallback<F>
where
    F: Fn(HookInput, Option<String>, CancellationToken, i32) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<HookOutput, HookError>> + Send + 'static,
{
    fn execute(
        &self,
        input: HookInput,
        tool_use_id: Option<String>,
        cancel: CancellationToken,
        hook_index: i32,
    ) -> BoxFuture<'static, Result<HookOutput, HookError>> {
        let f = Arc::clone(&self.f);
        Box::pin(async move { f(input, tool_use_id, cancel, hook_index).await })
    }

    fn dedupe_key(&self) -> Option<String> {
        // Callbacks are never deduplicated
        None
    }
}

impl<F> Debug for ClosureCallback<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref name) = self.name {
            write!(f, "ClosureCallback({name})")
        } else {
            write!(f, "ClosureCallback(<anonymous>)")
        }
    }
}

/// A simple callback that always returns success.
#[derive(Debug, Clone)]
pub struct NoOpCallback;

impl HookCallback for NoOpCallback {
    fn execute(
        &self,
        _input: HookInput,
        _tool_use_id: Option<String>,
        _cancel: CancellationToken,
        _hook_index: i32,
    ) -> BoxFuture<'static, Result<HookOutput, HookError>> {
        Box::pin(async { Ok(HookOutput::default()) })
    }
}

/// A callback that always blocks execution.
#[derive(Debug, Clone)]
pub struct BlockingCallback {
    reason: String,
}

impl BlockingCallback {
    /// Create a new blocking callback with a reason.
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl HookCallback for BlockingCallback {
    fn execute(
        &self,
        _input: HookInput,
        _tool_use_id: Option<String>,
        _cancel: CancellationToken,
        _hook_index: i32,
    ) -> BoxFuture<'static, Result<HookOutput, HookError>> {
        let reason = self.reason.clone();
        Box::pin(async move { Ok(HookOutput::block(reason)) })
    }
}

/// A callback that injects a system message.
#[derive(Debug, Clone)]
pub struct SystemMessageCallback {
    message: String,
}

impl SystemMessageCallback {
    /// Create a new system message callback.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl HookCallback for SystemMessageCallback {
    fn execute(
        &self,
        _input: HookInput,
        _tool_use_id: Option<String>,
        _cancel: CancellationToken,
        _hook_index: i32,
    ) -> BoxFuture<'static, Result<HookOutput, HookError>> {
        let message = self.message.clone();
        Box::pin(async move { Ok(HookOutput::with_system_message(message)) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::HookEventType;

    fn make_input() -> HookInput {
        HookInput {
            hook_event_name: HookEventType::PreToolUse,
            session_id: "test-session".to_string(),
            transcript_path: "/tmp/transcript.json".to_string(),
            cwd: "/tmp".to_string(),
            permission_mode: serde_json::Value::Null,
            event_data: crate::input::HookEventData::Empty {},
        }
    }

    #[tokio::test]
    async fn test_execute_noop_callback() {
        let callback = NoOpCallback;
        let cancel = CancellationToken::new();

        let result = execute_callback(&callback, make_input(), None, cancel, 0, None)
            .await
            .expect("Callback should succeed");

        assert_eq!(result.outcome, HookOutcome::Success);
        assert!(result.output.is_some());
    }

    #[tokio::test]
    async fn test_execute_blocking_callback() {
        let callback = BlockingCallback::new("Security violation");
        let cancel = CancellationToken::new();

        let result = execute_callback(&callback, make_input(), None, cancel, 0, None)
            .await
            .expect("Callback should complete");

        assert_eq!(result.outcome, HookOutcome::Blocking);
        assert!(result.output.as_ref().is_some_and(|o| o.is_blocking()));
    }

    #[tokio::test]
    async fn test_execute_system_message_callback() {
        let callback = SystemMessageCallback::new("Important context");
        let cancel = CancellationToken::new();

        let result = execute_callback(&callback, make_input(), None, cancel, 0, None)
            .await
            .expect("Callback should succeed");

        assert_eq!(result.outcome, HookOutcome::Success);
        let output = result.output.expect("Should have output");
        assert_eq!(output.system_message, Some("Important context".to_string()));
    }

    #[tokio::test]
    async fn test_closure_callback() {
        let callback = callback_from_fn(|input, _tool_use_id, _cancel, hook_index| async move {
            assert_eq!(input.hook_event_name, HookEventType::PreToolUse);
            assert_eq!(hook_index, 42);
            Ok(HookOutput::default())
        });

        let cancel = CancellationToken::new();
        let result = execute_callback(&callback, make_input(), None, cancel, 42, None)
            .await
            .expect("Callback should succeed");

        assert_eq!(result.outcome, HookOutcome::Success);
    }

    #[tokio::test]
    async fn test_named_closure_callback() {
        let callback = callback_from_fn_named(
            "test_callback",
            |_input, _tool_use_id, _cancel, _index| async { Ok(HookOutput::default()) },
        );

        let debug_str = format!("{callback:?}");
        assert!(debug_str.contains("test_callback"));
    }

    #[tokio::test]
    async fn test_callback_with_tool_use_id() {
        let callback = callback_from_fn(|_input, tool_use_id, _cancel, _index| async move {
            assert_eq!(tool_use_id, Some("tool-123".to_string()));
            Ok(HookOutput::default())
        });

        let cancel = CancellationToken::new();
        execute_callback(
            &callback,
            make_input(),
            Some("tool-123".to_string()),
            cancel,
            0,
            None,
        )
        .await
        .expect("Callback should succeed");
    }

    #[tokio::test]
    async fn test_callback_timeout() {
        let callback = callback_from_fn(|_input, _tool_use_id, _cancel, _index| async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok(HookOutput::default())
        });

        let cancel = CancellationToken::new();
        let result = execute_callback(
            &callback,
            make_input(),
            None,
            cancel,
            0,
            Some(100), // 100ms timeout
        )
        .await;

        assert!(matches!(result, Err(HookError::Timeout)));
    }

    #[tokio::test]
    async fn test_callback_cancellation() {
        let callback = callback_from_fn(|_input, _tool_use_id, _cancel, _index| async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok(HookOutput::default())
        });

        let cancel = CancellationToken::new();
        cancel.cancel();

        let result = execute_callback(&callback, make_input(), None, cancel, 0, None)
            .await
            .expect("Should return cancelled result");

        assert_eq!(result.outcome, HookOutcome::Cancelled);
    }

    #[tokio::test]
    async fn test_callback_error() {
        let callback = callback_from_fn(|_input, _tool_use_id, _cancel, _index| async {
            Err(HookError::ValidationFailed("Test error".to_string()))
        });

        let cancel = CancellationToken::new();
        let result = execute_callback(&callback, make_input(), None, cancel, 0, None).await;

        assert!(matches!(result, Err(HookError::ValidationFailed(_))));
    }

    #[test]
    fn test_noop_callback_debug() {
        let callback = NoOpCallback;
        let debug_str = format!("{callback:?}");
        assert!(debug_str.contains("NoOpCallback"));
    }

    #[test]
    fn test_blocking_callback_debug() {
        let callback = BlockingCallback::new("reason");
        let debug_str = format!("{callback:?}");
        assert!(debug_str.contains("BlockingCallback"));
    }
}
