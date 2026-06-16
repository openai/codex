use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use codex_core::config::Config;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_features::Feature;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::time::sleep;
use tokio::time::timeout;

async fn start_exec_server() -> Result<(tokio::task::JoinHandle<()>, String)> {
    let port = TcpListener::bind("127.0.0.1:0").await?.local_addr()?.port();
    let websocket_url = format!("ws://127.0.0.1:{port}");
    let runtime_paths = codex_exec_server::ExecServerRuntimePaths::new(
        std::env::current_exe()?,
        /*codex_linux_sandbox_exe*/ None,
    )?;
    let listen_url = websocket_url.clone();
    let task = tokio::spawn(async move {
        codex_exec_server::run_main(&listen_url, runtime_paths)
            .await
            .expect("test exec-server should run");
    });
    Ok((task, websocket_url))
}

async fn start_paused_proxy(target_url: &str) -> Result<(String, oneshot::Sender<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let proxy_url = format!("ws://{}", listener.local_addr()?);
    let target = target_url.trim_start_matches("ws://").to_string();
    let (release_tx, release_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut client, _) = listener.accept().await?;
        release_rx.await?;
        let mut server = TcpStream::connect(target).await?;
        tokio::io::copy_bidirectional(&mut client, &mut server).await?;
        Ok::<(), anyhow::Error>(())
    });
    Ok((proxy_url, release_tx))
}

fn has_tool(body: &Value, name: &str) -> bool {
    body["tools"]
        .as_array()
        .is_some_and(|tools| tools.iter().any(|tool| tool["name"] == name))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn turn_starts_before_environment_is_ready_and_updates_next_request() -> Result<()> {
    let (exec_server_task, exec_server_url) = start_exec_server().await?;
    let (proxy_url, release_proxy) = start_paused_proxy(&exec_server_url).await?;
    let server = start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call("wait-call", "wait_for_environment", "{}"),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    codex_deferred_executor_extension::install(&mut extensions);
    let mut builder = test_codex()
        .with_exec_server_url(proxy_url)
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            for feature in [Feature::DeferredExecutor, Feature::UnifiedExec] {
                let _ = config.features.enable(feature);
            }
        });
    let test = Arc::new(builder.build(&server).await?);

    let submit_task = {
        let test = Arc::clone(&test);
        tokio::spawn(async move { test.submit_turn("inspect the workspace").await })
    };
    timeout(Duration::from_secs(5), async {
        while response_mock.requests().is_empty() {
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;

    assert!(release_proxy.send(()).is_ok());
    timeout(Duration::from_secs(10), submit_task).await???;

    let requests = response_mock.requests();
    let first_body = requests[0].body_json();
    assert!(has_tool(&first_body, "wait_for_environment"));
    assert!(!has_tool(&first_body, "exec_command"));
    assert!(
        requests[0]
            .message_input_texts("user")
            .iter()
            .any(|text| text.contains("<shell>still loading</shell>"))
    );

    let second_body = requests[1].body_json();
    assert!(!has_tool(&second_body, "wait_for_environment"));
    assert!(has_tool(&second_body, "exec_command"));
    let last_environment_context = requests[1]
        .message_input_texts("user")
        .into_iter()
        .rfind(|text| text.starts_with("<environment_context>"))
        .expect("ready environment context should be recorded");
    assert!(!last_environment_context.contains("still loading"));

    drop(test);
    exec_server_task.abort();
    Ok(())
}
