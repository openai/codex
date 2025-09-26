use std::collections::HashMap;
use std::env;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use mcp_types::CallToolResult;
use rmcp::model::CallToolResult as RmcpCallToolResult;
use rmcp::service::ServiceError;
use serde_json::Value;
use tokio::time;

pub(crate) async fn run_with_timeout<F, T>(
    fut: F,
    timeout: Option<Duration>,
    label: &str,
) -> Result<T>
where
    F: std::future::Future<Output = Result<T, ServiceError>>,
{
    if let Some(duration) = timeout {
        let result = time::timeout(duration, fut)
            .await
            .with_context(|| anyhow!("timed out awaiting {label} after {duration:?}"))?;
        result.map_err(|err| anyhow!("{label} failed: {err}"))
    } else {
        fut.await.map_err(|err| anyhow!("{label} failed: {err}"))
    }
}

pub(crate) fn convert_call_tool_result(result: RmcpCallToolResult) -> Result<CallToolResult> {
    let mut value = serde_json::to_value(result)?;
    if let Some(obj) = value.as_object_mut()
        && (obj.get("content").is_none()
            || obj.get("content").is_some_and(serde_json::Value::is_null))
    {
        obj.insert("content".to_string(), Value::Array(Vec::new()));
    }
    serde_json::from_value(value).context("failed to convert call tool result")
}

/// Convert from mcp-types to Rust SDK types.
///
/// The Rust SDK types are the same as our mcp-types crate because they are both
/// derived from the same MCP specification.
/// As a result, it should be safe to convert directly from one to the other.
pub(crate) fn convert_to_rmcp<T, U>(value: T) -> Result<U>
where
    T: serde::Serialize,
    U: serde::de::DeserializeOwned,
{
    let json = serde_json::to_value(value)?;
    serde_json::from_value(json).map_err(|err| anyhow!(err))
}

/// Convert from Rust SDK types to mcp-types.
///
/// The Rust SDK types are the same as our mcp-types crate because they are both
/// derived from the same MCP specification.
/// As a result, it should be safe to convert directly from one to the other.
pub(crate) fn convert_to_mcp<T, U>(value: T) -> Result<U>
where
    T: serde::Serialize,
    U: serde::de::DeserializeOwned,
{
    let json = serde_json::to_value(value)?;
    serde_json::from_value(json).map_err(|err| anyhow!(err))
}

pub(crate) fn create_env_for_mcp_server(
    extra_env: Option<HashMap<String, String>>,
) -> HashMap<String, String> {
    DEFAULT_ENV_VARS
        .iter()
        .filter_map(|var| env::var(var).ok().map(|value| (var.to_string(), value)))
        .chain(extra_env.unwrap_or_default())
        .collect()
}

#[cfg(unix)]
pub(crate) const DEFAULT_ENV_VARS: &[&str] = &[
    "HOME",
    "LOGNAME",
    "PATH",
    "SHELL",
    "USER",
    "__CF_USER_TEXT_ENCODING",
    "LANG",
    "LC_ALL",
    "TERM",
    "TMPDIR",
    "TZ",
];

#[cfg(windows)]
pub(crate) const DEFAULT_ENV_VARS: &[&str] = &[
    "PATH",
    "PATHEXT",
    "USERNAME",
    "USERDOMAIN",
    "USERPROFILE",
    "TEMP",
    "TMP",
];

#[cfg(test)]
mod tests {
    use super::*;
    use mcp_types::ContentBlock;
    use pretty_assertions::assert_eq;
    use rmcp::model::CallToolResult as RmcpCallToolResult;
    use serde_json::json;

    #[tokio::test]
    async fn create_env_honors_overrides() {
        let value = "custom".to_string();
        let env = create_env_for_mcp_server(Some(HashMap::from([("TZ".into(), value.clone())])));
        assert_eq!(env.get("TZ"), Some(&value));
    }

    #[test]
    fn convert_call_tool_result_defaults_missing_content() -> Result<()> {
        let structured_content = json!({ "key": "value" });
        let rmcp_result = RmcpCallToolResult {
            content: vec![],
            structured_content: Some(structured_content.clone()),
            is_error: Some(true),
            meta: None,
        };

        let result = convert_call_tool_result(rmcp_result)?;

        assert!(result.content.is_empty());
        assert_eq!(result.structured_content, Some(structured_content));
        assert_eq!(result.is_error, Some(true));

        Ok(())
    }

    #[test]
    fn convert_call_tool_result_preserves_existing_content() -> Result<()> {
        let rmcp_result = RmcpCallToolResult::success(vec![rmcp::model::Content::text("hello")]);

        let result = convert_call_tool_result(rmcp_result)?;

        assert_eq!(result.content.len(), 1);
        match &result.content[0] {
            ContentBlock::TextContent(text_content) => {
                assert_eq!(text_content.text, "hello");
                assert_eq!(text_content.r#type, "text");
            }
            other => panic!("expected text content got {other:?}"),
        }
        assert_eq!(result.structured_content, None);
        assert_eq!(result.is_error, Some(false));

        Ok(())
    }
}
