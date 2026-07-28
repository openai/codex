use std::io;
use std::io::BufRead;
use std::io::Write;

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
        "modern" | "legacy" | "legacy-fallback" | "oversized-stdout" | "oversized-stdout-legacy"
    ));
    assert!(std::env::var_os("CODEX_MCP_PROTOCOL_VERSION").is_none());
    let is_legacy = mode.starts_with("legacy") || mode.ends_with("-legacy");
    let oversized = mode.starts_with("oversized-stdout");
    let mut stdout = io::stdout().lock();
    let mut initialized = false;
    let mut pending_legacy_call = None;

    for line in io::stdin().lock().lines() {
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

        match method {
            "server/discover" if is_legacy => {
                assert_eq!(mode, "legacy-fallback");
                write_message(
                    &mut stdout,
                    json!({
                        "jsonrpc": "2.0",
                        "id": message["id"],
                        "error": {"code": -32601, "message": "method not found"},
                    }),
                )?;
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
            "initialize" => {
                assert!(is_legacy);
                assert_eq!(message["params"]["protocolVersion"], LEGACY_VERSION);
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
