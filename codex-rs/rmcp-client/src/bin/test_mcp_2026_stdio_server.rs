use std::io;
use std::io::BufRead;
use std::io::Write;

use anyhow::Context;
use anyhow::Result;
use serde_json::Value;
use serde_json::json;

const MODERN_VERSION: &str = "2026-07-28";
const LEGACY_VERSION: &str = "2025-06-18";
const MAX_MESSAGE_BYTES: usize = 8 * 1024 * 1024;

fn write_message(stdout: &mut io::StdoutLock<'_>, message: Value) -> Result<()> {
    serde_json::to_writer(&mut *stdout, &message)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}

fn assert_modern_metadata(message: &Value) {
    let metadata = &message["params"]["_meta"];
    assert_eq!(
        metadata["io.modelcontextprotocol/protocolVersion"],
        MODERN_VERSION
    );
    assert!(metadata["io.modelcontextprotocol/clientInfo"].is_object());
    assert!(metadata["io.modelcontextprotocol/clientCapabilities"].is_object());
}

fn legacy_schema_defaults() -> Value {
    json!({
        "name": "John Doe",
        "age": 30,
        "score": 95.5,
        "status": "active",
        "verified": true,
    })
}

fn main() -> Result<()> {
    let mode = std::env::args().nth(1).unwrap_or_default();
    assert!(matches!(
        mode.as_str(),
        "modern"
            | "modern-late"
            | "legacy"
            | "legacy-fallback"
            | "legacy-reject-and-exit"
            | "legacy-init-first"
            | "legacy-reject-late-initialize"
            | "legacy-late"
            | "legacy-silent"
            | "legacy-versions"
            | "oversized-stdout"
            | "oversized-stdout-legacy"
    ));
    assert!(std::env::var_os("CODEX_MCP_PROTOCOL_VERSION").is_none());
    let is_legacy = mode.starts_with("legacy") || mode.ends_with("-legacy");
    let oversized = mode.starts_with("oversized-stdout");
    let mut stdout = io::stdout().lock();
    let mut initialized = false;
    let mut pending_legacy_call = None;
    let mut late_discovery_id = None;

    for (index, line) in io::stdin().lock().lines().enumerate() {
        let message: Value = serde_json::from_str(&line?)?;
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            let pending_call = pending_legacy_call
                .take()
                .ok_or_else(|| anyhow::anyhow!("unexpected server-request response"))?;
            assert_eq!(message["id"], "legacy-approval");
            assert_eq!(message["result"]["action"], "accept");
            assert_eq!(message["result"]["content"], legacy_schema_defaults());
            write_message(
                &mut stdout,
                json!({
                    "jsonrpc": "2.0",
                    "id": pending_call,
                    "result": {"content": [{"type": "text", "text": "legacy approved"}]},
                }),
            )?;
            continue;
        };

        // Some older helpers assume their first request is initialize without inspecting its method.
        let method = if mode == "legacy-init-first" && !initialized {
            "initialize"
        } else {
            method
        };
        match method {
            // Some legacy stdio servers exit if their first message is not initialize.
            "server/discover" if mode == "legacy" => {
                // Some wrappers print a banner before starting the protocol.
                writeln!(stdout, "starting server")?;
                write_message(
                    &mut stdout,
                    json!({"jsonrpc": "2.0", "id": "startup", "method": "ping"}),
                )?;
                return Ok(());
            }
            "server/discover" if mode == "legacy-late" => {
                late_discovery_id = Some(message["id"].clone());
            }
            // The first helper makes its version offer only after it receives initialize.
            "server/discover"
                if mode == "modern-late"
                    && !std::path::Path::new("retry-modern").try_exists()? => {}
            "initialize" if mode == "modern-late" => {
                std::fs::write("retry-modern", [])?;
                write_message(
                    &mut stdout,
                    json!({
                        "jsonrpc": "2.0", "id": null,
                        "error": {"code": -32022, "message": "unsupported protocol", "data": {"supported": [MODERN_VERSION]}},
                    }),
                )?;
                return Ok(());
            }
            "server/discover" if mode == "legacy-silent" => {
                write_message(
                    &mut stdout,
                    json!({
                        "jsonrpc": "2.0", "method": "notifications/message",
                        "params": {"level": "info", "data": "starting server"},
                    }),
                )?;
            }
            "server/discover" if mode == "legacy-versions" => {
                write_message(
                    &mut stdout,
                    json!({
                        "jsonrpc": "2.0", "id": message["id"],
                        "error": {"code": -32022, "message": "unsupported protocol", "data": {"supported": [LEGACY_VERSION]}},
                    }),
                )?;
            }
            "server/discover" if is_legacy => {
                write_message(
                    &mut stdout,
                    json!({
                        "jsonrpc": "2.0",
                        "id": message["id"],
                        "error": {"code": -32601, "message": "method not found"},
                    }),
                )?;
                if mode == "legacy-reject-and-exit" {
                    return Ok(());
                }
            }
            "server/discover" => {
                assert!(!initialized);
                assert_modern_metadata(&message);
                initialized = true;
                write_message(
                    &mut stdout,
                    json!({
                        "jsonrpc": "2.0",
                        "id": message["id"],
                        "result": {
                            "resultType": "complete",
                            "supportedVersions": [MODERN_VERSION],
                            "capabilities": {"tools": {}, "resources": {}},
                            "_meta": {
                                "io.modelcontextprotocol/serverInfo": {
                                    "name": "strict-stdio-test",
                                    "version": "1.0.0",
                                },
                            },
                            "ttlMs": 0,
                            "cacheScope": "private",
                        },
                    }),
                )?;
            }
            "initialize" if mode == "legacy-reject-late-initialize" && index != 0 => {
                write_message(
                    &mut stdout,
                    json!({"jsonrpc": "2.0", "id": message["id"], "error": {
                        "code": -32600, "message": "initialize must be the first request"
                    }}),
                )?;
            }
            "initialize" => {
                assert!(is_legacy);
                if mode == "legacy-late" {
                    let id = late_discovery_id
                        .take()
                        .context("initialize must follow discover on the same pipe")?;
                    write_message(
                        &mut stdout,
                        json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Method not found"}}),
                    )?;
                }
                if message["method"] == "initialize" {
                    assert_eq!(message["params"]["protocolVersion"], LEGACY_VERSION);
                }
                initialized = true;
                write_message(
                    &mut stdout,
                    json!({
                        "jsonrpc": "2.0",
                        "id": message["id"],
                        "result": {
                            "protocolVersion": LEGACY_VERSION,
                            "capabilities": {"tools": {}},
                            "serverInfo": {"name": "legacy-stdio-test", "version": "1.0.0"},
                        },
                    }),
                )?;
            }
            "notifications/initialized" => assert!(is_legacy),
            "tools/list" => {
                assert!(initialized);
                if !is_legacy {
                    assert_modern_metadata(&message);
                }
                if mode == "oversized-stdout-legacy" {
                    write!(stdout, "\u{feff}")?;
                }
                let description = if oversized {
                    "x".repeat(MAX_MESSAGE_BYTES + 1)
                } else {
                    "echo a value".to_owned()
                };
                let mut result = json!({
                    "tools": [{
                        "name": "echo",
                        "description": description,
                        "inputSchema": {"type": "object"},
                    }],
                });
                if !is_legacy {
                    result["resultType"] = json!("complete");
                    result["ttlMs"] = json!(0);
                    result["cacheScope"] = json!("private");
                }
                write_message(
                    &mut stdout,
                    json!({"jsonrpc": "2.0", "id": message["id"], "result": result}),
                )?;
            }
            "tools/call" if is_legacy => {
                assert!(initialized);
                pending_legacy_call = Some(message["id"].clone());
                write_message(
                    &mut stdout,
                    json!({
                        "jsonrpc": "2.0",
                        "id": "legacy-approval",
                        "method": "elicitation/create",
                        "params": {
                            "message": "Accept the schema defaults",
                            "requestedSchema": {
                                "type": "object",
                                "properties": {
                                    "name": {"type": "string", "default": "John Doe"},
                                    "age": {"type": "integer", "default": 30},
                                    "score": {"type": "number", "default": 95.5},
                                    "status": {
                                        "type": "string",
                                        "enum": ["active", "inactive"],
                                        "default": "active",
                                    },
                                    "verified": {"type": "boolean", "default": true},
                                },
                                "required": [],
                            },
                        },
                    }),
                )?;
            }
            "tools/call" => {
                assert!(initialized);
                assert_modern_metadata(&message);
                if message.pointer("/params/inputResponses").is_none() {
                    write_message(
                        &mut stdout,
                        json!({
                            "jsonrpc": "2.0",
                            "id": message["id"],
                            "result": {
                                "resultType": "input_required",
                                "inputRequests": {
                                    "approval": {
                                        "method": "elicitation/create",
                                        "params": {
                                            "mode": "form",
                                            "message": "Approve stdio call?",
                                            "requestedSchema": {
                                                "type": "object",
                                                "properties": {
                                                    "approved": {"type": "boolean"},
                                                },
                                            },
                                        },
                                    },
                                },
                                "requestState": "stdio-state",
                                "_meta": {
                                    "io.modelcontextprotocol/serverInfo": {
                                        "name": "strict-stdio-test",
                                        "version": "1.0.0",
                                    },
                                },
                            },
                        }),
                    )?;
                    continue;
                }

                assert_eq!(message["params"]["requestState"], "stdio-state");
                assert_eq!(
                    message["params"]["inputResponses"]["approval"]["action"],
                    "accept"
                );
                assert_eq!(
                    message["params"]["inputResponses"]["approval"]["content"]["approved"],
                    true
                );
                write_message(
                    &mut stdout,
                    json!({
                        "jsonrpc": "2.0",
                        "id": message["id"],
                        "result": {
                            "resultType": "complete",
                            "content": [{"type": "text", "text": "modern approved"}],
                        },
                    }),
                )?;
            }
            other => anyhow::bail!("unexpected MCP method {other}"),
        }
    }

    Ok(())
}
