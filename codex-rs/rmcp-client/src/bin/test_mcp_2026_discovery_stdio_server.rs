use std::io;
use std::io::BufRead;
use std::io::Write;

use anyhow::Result;
use serde_json::Value;
use serde_json::json;

fn main() -> Result<()> {
    assert!(std::env::var_os("CODEX_MCP_PROTOCOL_VERSION").is_none());
    let mut stdout = io::stdout().lock();

    for line in io::stdin().lock().lines() {
        let request: Value = serde_json::from_str(&line?)?;
        let result = match request["method"].as_str() {
            Some("server/discover") => {
                assert_eq!(
                    request["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"],
                    "2026-07-28"
                );
                json!({
                    "resultType": "complete",
                    "supportedVersions": ["2026-07-28"],
                    "capabilities": {"tools": {}, "resources": {}},
                    "_meta": {
                        "io.modelcontextprotocol/serverInfo": {
                            "name": "strict-stdio-discovery",
                            "version": "1.0.0",
                        },
                    },
                    "ttlMs": 0,
                    "cacheScope": "private",
                })
            }
            Some("tools/list") => json!({
                "resultType": "complete",
                "tools": [{
                    "name": "stdio_echo",
                    "inputSchema": {"type": "object"},
                }],
            }),
            Some("resources/list") => json!({
                "resultType": "complete",
                "resources": [{
                    "uri": "test://stdio/resource",
                    "name": "stdio resource",
                }],
            }),
            Some("resources/templates/list") => json!({
                "resultType": "complete",
                "resourceTemplates": [{
                    "uriTemplate": "test://stdio/{name}",
                    "name": "stdio resource template",
                }],
            }),
            method => anyhow::bail!("unexpected modern stdio discovery method: {method:?}"),
        };
        serde_json::to_writer(
            &mut stdout,
            &json!({"jsonrpc": "2.0", "id": request["id"], "result": result}),
        )?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }
    Ok(())
}
