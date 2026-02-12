#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::borrow::Cow;
use std::fs;
use std::sync::Arc;

use anyhow::Result;
use exec_server_test_support::create_transport;
use pretty_assertions::assert_eq;
use rmcp::ServiceExt;
use rmcp::model::Tool;
use rmcp::model::object;
use serde_json::json;
use tempfile::TempDir;

/// Verify the list_tools call to the MCP server returns the expected response.
#[tokio::test(flavor = "current_thread")]
async fn list_tools() -> Result<()> {
    let codex_home = TempDir::new()?;
    let policy_dir = codex_home.path().join("rules");
    fs::create_dir_all(&policy_dir)?;
    fs::write(
        policy_dir.join("default.rules"),
        r#"prefix_rule(pattern=["ls"], decision="prompt")"#,
    )?;
    let dotslash_cache_temp_dir = TempDir::new()?;
    let dotslash_cache = dotslash_cache_temp_dir.path();
    let transport = create_transport(codex_home.path(), dotslash_cache).await?;

    let service = ().serve(transport).await?;
    let tools = service.list_tools(Default::default()).await?.tools;
    assert_eq!(
        vec![Tool {
            name: Cow::Borrowed("shell"),
            title: None,
            description: Some(Cow::Borrowed(
                "Runs a shell command and returns its output. You MUST provide the workdir as an absolute path."
            )),
            input_schema: Arc::new(object(json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "properties": {
                    "command": {
                        "description": "The bash string to execute.",
                        "type": "string",
                    },
                    "login": {
                        "description": "Launch Bash with -lc instead of -c: defaults to true.",
                        "nullable": true,
                        "type": "boolean",
                    },
                    "timeout_ms": {
                        "description": "The timeout for the command in milliseconds.",
                        "format": "uint64",
                        "minimum": 0,
                        "nullable": true,
                        "type": "integer",
                    },
                    "workdir": {
                        "description": "The working directory to execute the command in. Must be an absolute path.",
                        "type": "string",
                    },
                },
                "required": [
                    "command",
                    "workdir",
                ],
                "title": "ExecParams",
                "type": "object",
            }))),
            output_schema: None,
            execution: None,
            annotations: None,
            icons: None,
            meta: None
        }],
        tools
    );

    Ok(())
}
