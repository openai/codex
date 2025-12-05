use std::borrow::Cow;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;

use anyhow::Result;
use pretty_assertions::assert_eq;
use rmcp::ServiceExt;
use rmcp::model::Tool;
use rmcp::model::object;
use rmcp::transport::ConfigureCommandExt;
use rmcp::transport::TokioChildProcess;
use serde_json::json;
use tokio::process::Command;

#[tokio::test(flavor = "current_thread")]
async fn auto_approve() -> Result<()> {
    let mcp_executable = assert_cmd::Command::cargo_bin("codex-exec-mcp-server")?;
    let execve_wrapper = assert_cmd::Command::cargo_bin("codex-execve-wrapper")?;
    let bash = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("suite")
        .join("bash");
    let transport =
        TokioChildProcess::new(Command::new(mcp_executable.get_program()).configure(|cmd| {
            cmd.arg("--bash").arg(bash);
            cmd.arg("--execve").arg(execve_wrapper.get_program());

            // Important: pipe stdio so rmcp can speak JSON-RPC over stdin/stdout
            cmd.stdin(Stdio::piped());
            cmd.stdout(Stdio::piped());

            // Optional but very helpful while debugging:
            cmd.stderr(Stdio::inherit());
        }))?;

    let service = ().serve(transport).await?;
    let tools = service.list_tools(Default::default()).await?.tools;
    assert_eq!(
        vec![Tool {
            name: Cow::Borrowed("shell"),
            title: None,
            description: Some(Cow::Borrowed(
                "Runs a shell command and returns its output. You MUST provide the workdir as an absolute path."
            )),
            input_schema: Arc::new(object(json!(        {
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
            annotations: None,
            icons: None,
            meta: None
        }],
        tools
    );

    // TODO(mbolin): Make shell tool calls and verify they work.

    Ok(())
}
