//! Exercises the child protocol without the parent supervisor or session APIs.

use anyhow::Context;
use codex_plugin_host::PluginProcess;
use codex_plugin_host::protocol::MAX_PENDING_PROMISES;
use codex_plugin_host::protocol::MAX_SCRIPT_ERROR_BYTES;
use codex_plugin_host::protocol::Packet;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test]
async fn child_initializes_invokes_and_closes_an_isolate() -> anyhow::Result<()> {
    let program = codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?;
    let mut child =
        PluginProcess::spawn(&program, &tokio_util::sync::CancellationToken::new()).await?;
    let result = async {
        let source = format!("/*{}*/ export default () => 'x'.repeat(5000);", "p".repeat(40 * 1024));
        for (message, expected) in [
            (json!({"type": "initialize", "source": source}), json!({"type": "initialized"})),
            (json!({"type": "dispatch", "id": 0, "export": "default", "returnMode": "maybeAsync"}), json!({"type": "reply", "id": 0, "value": "x".repeat(5000)})),
            (json!({"type": "close"}), json!({"type": "closed"})),
        ] {
            send(&mut child, /*instance*/ 7, message).await?;
            assert_eq!(receive(&mut child).await?, (7, expected));
        }
        anyhow::Ok(())
    }
    .await;
    child.close().await?;
    result
}

async fn send(
    child: &mut PluginProcess,
    instance: u64,
    message: serde_json::Value,
) -> anyhow::Result<()> {
    let (input, _) = child.pipes();
    input
        .send(serde_json::to_vec(&Packet { instance, message })?.into())
        .await?;
    Ok(())
}

async fn receive(child: &mut PluginProcess) -> anyhow::Result<(u64, serde_json::Value)> {
    let (_, output) = child.pipes();
    let packet: Packet<serde_json::Value> = serde_json::from_slice(
        &output
            .next()
            .await
            .context("plugin child closed stdout")??,
    )?;
    Ok((packet.instance, packet.message))
}

#[tokio::test]
async fn promise_errors_close_the_isolate_in_both_return_modes() -> anyhow::Result<()> {
    let program = codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?;
    let mut child =
        PluginProcess::spawn(&program, &tokio_util::sync::CancellationToken::new()).await?;
    let result = async {
        let source = r#"
            export function run() {
                const promise = Promise.reject(Error('tracked failure'));
                promise.catch(() => {});
                return promise;
            }
        "#;
        for (instance, return_mode, code) in [
            (1, "sync", "invalid_result"),
            (2, "maybeAsync", "execution_failed"),
        ] {
            send(
                &mut child,
                instance,
                json!({"type": "initialize", "source": source}),
            )
            .await?;
            assert_eq!(
                receive(&mut child).await?,
                (instance, json!({"type": "initialized"}))
            );
            send(
                &mut child,
                instance,
                json!({"type": "dispatch", "id": 0, "export": "run", "returnMode": return_mode}),
            )
            .await?;
            let (id, error) = receive(&mut child).await?;
            assert_eq!(
                (id, error["type"].as_str(), error["code"].as_str()),
                (instance, Some("error"), Some(code))
            );
            if return_mode == "maybeAsync" {
                assert!(
                    error["scriptError"]["message"]
                        .as_str()
                        .is_some_and(|diagnostic| diagnostic.contains("tracked failure"))
                );
            }
            assert_eq!(
                receive(&mut child).await?,
                (instance, json!({"type": "closed"}))
            );
        }
        anyhow::Ok(())
    }
    .await;
    child.close().await?;
    result
}

#[tokio::test]
async fn cancellation_closes_an_isolate_with_pending_work() -> anyhow::Result<()> {
    let program = codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?;
    let mut child =
        PluginProcess::spawn(&program, &tokio_util::sync::CancellationToken::new()).await?;
    let result = async {
        let source = "export function run() { return new Promise(() => {}); }";
        send(
            &mut child,
            /*instance*/ 1,
            json!({"type": "initialize", "source": source}),
        )
        .await?;
        assert_eq!(
            receive(&mut child).await?,
            (1, json!({"type": "initialized"}))
        );
        send(
            &mut child,
            /*instance*/ 1,
            json!({"type": "dispatch", "id": 0, "export": "run", "returnMode": "maybeAsync"}),
        )
        .await?;
        assert_eq!(
            receive(&mut child).await?,
            (1, json!({"type": "pending", "id": 0}))
        );
        send(&mut child, /*instance*/ 1, json!({"type": "close"})).await?;
        assert_eq!(receive(&mut child).await?, (1, json!({"type": "closed"})));
        anyhow::Ok(())
    }
    .await;
    child.close().await?;
    result
}

#[tokio::test]
async fn sync_dispatch_settles_pending_invocations() -> anyhow::Result<()> {
    let program = codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?;
    let mut child =
        PluginProcess::spawn(&program, &tokio_util::sync::CancellationToken::new()).await?;
    let result = async {
        let source = r#"
            const resolvers = [];
            export function wait() { return new Promise(resolve => resolvers.push(resolve)); }
            export function finish() {
                for (const resolve of resolvers) resolve('done');
                return 'finished';
            }
        "#;
        send(
            &mut child,
            /*instance*/ 1,
            json!({"type": "initialize", "source": source}),
        )
        .await?;
        assert_eq!(
            receive(&mut child).await?,
            (1, json!({"type": "initialized"}))
        );
        let sync_id = MAX_PENDING_PROMISES as u64;
        for id in 0..sync_id {
            send(
                &mut child,
                /*instance*/ 1,
                json!({"type": "dispatch", "id": id, "export": "wait", "returnMode": "maybeAsync"}),
            )
            .await?;
            assert_eq!(
                receive(&mut child).await?,
                (1, json!({"type": "pending", "id": id}))
            );
        }
        send(
            &mut child,
            /*instance*/ 1,
            json!({"type": "dispatch", "id": sync_id, "export": "finish", "returnMode": "sync"}),
        )
        .await?;
        assert_eq!(
            receive(&mut child).await?,
            (
                1,
                json!({"type": "reply", "id": sync_id, "value": "finished"})
            )
        );
        let mut resolved = Vec::new();
        for _ in 0..sync_id {
            let (instance, reply) = receive(&mut child).await?;
            let id = reply["id"].as_u64().unwrap();
            assert_eq!(
                (instance, reply),
                (1, json!({"type": "reply", "id": id, "value": "done"}))
            );
            resolved.push(id);
        }
        resolved.sort_unstable();
        assert_eq!(resolved, (0..sync_id).collect::<Vec<_>>());
        anyhow::Ok(())
    }
    .await;
    child.close().await?;
    result
}

#[tokio::test]
async fn syntax_and_execution_failures_preserve_bounded_script_errors() -> anyhow::Result<()> {
    let program = codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?;
    let mut child =
        PluginProcess::spawn(&program, &tokio_util::sync::CancellationToken::new()).await?;
    let result = async {
        send(
            &mut child,
            /*instance*/ 1,
            json!({"type": "initialize", "source": "export default => ;"}),
        )
        .await?;
        let (id, error) = receive(&mut child).await?;
        assert_eq!(id, 1);
        assert_eq!(error["code"], "invalid_program");
        assert_eq!(error["scriptError"]["line"], 1);
        assert!(error["scriptError"]["column"].is_number());
        assert_eq!(error["scriptError"]["truncated"], false);
        assert_eq!(
            receive(&mut child).await?,
            (1, json!({"type": "closed"}))
        );

        let source = format!(
            r#"export function fail() {{ throw Error('xxx\u001b[31m' + 'x'.repeat({MAX_SCRIPT_ERROR_BYTES})); }}"#
        );
        send(
            &mut child,
            /*instance*/ 2,
            json!({"type": "initialize", "source": source}),
        )
        .await?;
        assert_eq!(
            receive(&mut child).await?,
            (2, json!({"type": "initialized"}))
        );
        send(
            &mut child,
            /*instance*/ 2,
            json!({"type": "dispatch", "id": 0, "export": "fail", "returnMode": "sync"}),
        )
        .await?;
        let (id, error) = receive(&mut child).await?;
        assert_eq!(id, 2);
        let message = error["scriptError"]["message"].as_str().unwrap();
        assert!(message.contains("Error: xxx\u{001b}[31m"));
        assert!(message.len() <= MAX_SCRIPT_ERROR_BYTES);
        assert_eq!(error["scriptError"]["truncated"], true);
        assert_eq!(
            receive(&mut child).await?,
            (2, json!({"type": "closed"}))
        );
        anyhow::Ok(())
    }
    .await;
    child.close().await?;
    result
}

#[tokio::test]
async fn loading_restrictions_close_the_isolate() -> anyhow::Result<()> {
    let program = codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?;
    let mut child =
        PluginProcess::spawn(&program, &tokio_util::sync::CancellationToken::new()).await?;
    let result = async {
        let cases = [
            (
                "import 'forbidden';",
                "invalid_program",
                "module imports are unavailable",
            ),
            (
                "await import('forbidden');",
                "execution_failed",
                "Not supported",
            ),
            ("eval('1 + 1');", "execution_failed", "EvalError"),
            (
                "new Function('return 1')();",
                "execution_failed",
                "EvalError",
            ),
            (
                "WebAssembly;",
                "execution_failed",
                "WebAssembly is not defined",
            ),
            (
                "new ArrayBuffer(1);",
                "execution_failed",
                "ArrayBuffer is not defined",
            ),
            (
                "new Uint8Array(1);",
                "execution_failed",
                "Uint8Array is not defined",
            ),
        ];
        for (instance, (source, code, diagnostic)) in (1_u64..).zip(cases) {
            send(
                &mut child,
                instance,
                json!({"type": "initialize", "source": source}),
            )
            .await?;
            let (id, error) = receive(&mut child)
                .await
                .with_context(|| format!("loading {source}"))?;
            assert_eq!(
                (id, error["type"].as_str(), error["code"].as_str()),
                (instance, Some("error"), Some(code)),
                "{source}"
            );
            assert!(
                error["scriptError"]["message"]
                    .as_str()
                    .is_some_and(|message| message.contains(diagnostic)),
                "{source}: {error}"
            );
            assert_eq!(
                receive(&mut child).await?,
                (instance, json!({"type": "closed"})),
                "{source}"
            );
        }
        anyhow::Ok(())
    }
    .await;
    let exit = child.close().await?;
    result.with_context(|| {
        format!(
            "plugin helper exited with {}; stderr: {:?}",
            exit.status,
            String::from_utf8_lossy(&exit.stderr_tail)
        )
    })
}

#[tokio::test]
async fn isolate_failures_do_not_affect_healthy_peers() -> anyhow::Result<()> {
    const HEALTHY: u64 = 1;
    const REJECTING: u64 = 2;
    const LOOPING: u64 = 3;

    let program = codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?;
    let mut child =
        PluginProcess::spawn(&program, &tokio_util::sync::CancellationToken::new()).await?;
    let result = async {
        let source = r#"
            let n = 0;
            export function count() { return String(++n); }
            export function caught() {
                const p = Promise.reject('handled');
                p.catch(() => {});
                return 'ok';
            }
            export function detached() {
                Promise.resolve().then(() => { throw Error('detached failure'); });
                return 'ok';
            }
        "#;
        for instance in [HEALTHY, REJECTING] {
            send(
                &mut child,
                instance,
                json!({"type": "initialize", "source": source}),
            )
            .await?;
            assert_eq!(
                receive(&mut child).await?,
                (instance, json!({"type": "initialized"}))
            );
        }
        for (instance, export, expected) in [
            (HEALTHY, "count", "1"),
            (HEALTHY, "count", "2"),
            (REJECTING, "caught", "ok"),
            (REJECTING, "count", "1"),
        ] {
            send(
                &mut child,
                instance,
                json!({"type": "dispatch", "id": 0, "export": export, "returnMode": "maybeAsync"}),
            )
            .await?;
            assert_eq!(
                receive(&mut child).await?,
                (
                    instance,
                    json!({"type": "reply", "id": 0, "value": expected})
                )
            );
        }

        send(
            &mut child,
            REJECTING,
            json!({"type": "dispatch", "id": 0, "export": "detached", "returnMode": "maybeAsync"}),
        )
        .await?;
        let (id, error) = receive(&mut child).await?;
        assert_eq!(id, REJECTING);
        assert_eq!(error["code"], "execution_failed");
        assert!(
            error["scriptError"]["message"]
                .as_str()
                .unwrap()
                .contains("detached failure")
        );
        assert_eq!(
            receive(&mut child).await?,
            (REJECTING, json!({"type": "closed"}))
        );

        let source = "export function loop() { while (true) {} }";
        send(
            &mut child,
            LOOPING,
            json!({"type": "initialize", "source": source}),
        )
        .await?;
        assert_eq!(
            receive(&mut child).await?,
            (LOOPING, json!({"type": "initialized"}))
        );
        send(
            &mut child,
            LOOPING,
            json!({"type": "dispatch", "id": 0, "export": "loop", "returnMode": "maybeAsync"}),
        )
        .await?;
        send(
            &mut child,
            HEALTHY,
            json!({"type": "dispatch", "id": 0, "export": "count", "returnMode": "maybeAsync"}),
        )
        .await?;
        assert_eq!(
            receive(&mut child).await?,
            (
                LOOPING,
                json!({"type": "error", "code": "execution_timed_out", "scriptError": null})
            )
        );
        assert_eq!(
            receive(&mut child).await?,
            (LOOPING, json!({"type": "closed"}))
        );
        assert_eq!(
            receive(&mut child).await?,
            (HEALTHY, json!({"type": "reply", "id": 0, "value": "3"}))
        );
        anyhow::Ok(())
    }
    .await;
    child.close().await?;
    result
}

#[tokio::test]
async fn closed_instance_can_be_replaced_at_capacity() -> anyhow::Result<()> {
    let program = codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?;
    let mut child =
        PluginProcess::spawn(&program, &tokio_util::sync::CancellationToken::new()).await?;
    let result = async {
        let limit = codex_plugin_host::MAX_INSTANCES as u64;
        let initialize = json!({"type":"initialize", "source":"export default () => 'ok'"});
        for instance in 0..limit {
            send(&mut child, instance, initialize.clone()).await?;
            assert_eq!(
                receive(&mut child).await?,
                (instance, json!({"type":"initialized"}))
            );
        }
        send(&mut child, /*instance*/ 0, json!({"type":"close"})).await?;
        assert_eq!(receive(&mut child).await?, (0, json!({"type":"closed"})));
        send(&mut child, limit, initialize).await?;
        assert_eq!(
            receive(&mut child).await?,
            (limit, json!({"type":"initialized"}))
        );
        anyhow::Ok(())
    }
    .await;
    child.close().await?;
    result
}
