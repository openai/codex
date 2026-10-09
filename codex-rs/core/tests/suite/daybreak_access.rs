use std::sync::Arc;

use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_login::ExternalAuth;
use codex_login::ExternalAuthFuture;
use codex_login::ExternalAuthRefreshContext;
use codex_protocol::protocol::EventMsg;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::stdio_server_bin;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use test_case::test_case;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use wiremock::Mock;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[derive(Clone, Copy)]
enum AccessCase {
    InstalledStdio,
    UndeclaredDaybreakName,
    ExternalTokens,
    #[cfg(unix)]
    RemoteStdio,
    UnownedStdio,
    InstalledHttp,
    MutableStdio,
    NonemptyArguments,
    ApiKey,
}

#[test_case(AccessCase::InstalledStdio; "installed local plugin")]
#[test_case(AccessCase::UndeclaredDaybreakName; "undeclared Daybreak tool name")]
#[test_case(AccessCase::ExternalTokens; "external ChatGPT tokens")]
#[cfg_attr(unix, test_case(AccessCase::RemoteStdio; "installed remote stdio plugin"))]
#[test_case(AccessCase::UnownedStdio; "unowned local server")]
#[test_case(AccessCase::InstalledHttp; "HTTP plugin in default environment")]
#[test_case(AccessCase::MutableStdio; "mutable plugin tool")]
#[test_case(AccessCase::NonemptyArguments; "nonempty arguments")]
#[test_case(AccessCase::ApiKey; "API key session")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn daybreak_metadata_follows_the_actual_plugin_call(case: AccessCase) -> Result<()> {
    skip_if_no_network!(Ok(()));
    #[cfg(unix)]
    let remote = if matches!(case, AccessCase::RemoteStdio) {
        Some(core_test_support::exec_server::ExecServerProcess::start().await?)
    } else {
        None
    };
    #[cfg(unix)]
    let remote_url = remote.as_ref().map(|remote| remote.websocket_url.clone());
    #[cfg(not(unix))]
    let remote_url: Option<String> = None;
    let tool_name = if matches!(
        case,
        AccessCase::UndeclaredDaybreakName | AccessCase::InstalledHttp
    ) {
        "get_daybreak_access"
    } else {
        "get_codex_security_daybreak_access"
    };
    let server = responses::start_mock_server().await;
    Mock::given(method("GET"))
        .and(path("/accounts/verified_access"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "programs": [{"program":"cyber", "state":"active", "grants":[
                {"level":"tac1", "source":"individual"}
            ]}]
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/remote-mcp"))
        .respond_with(|request: &Request| {
            let body: Value = request.body_json().expect("MCP JSON-RPC request");
            let result = match body["method"].as_str() {
                Some("initialize") => json!({
                    "protocolVersion": body["params"]["protocolVersion"],
                    "capabilities":{"tools":{}},
                    "serverInfo":{"name":"daybreak-test", "version":"1"}
                }),
                Some("tools/list") => json!({"tools":[{
                    "name":"get_daybreak_access", "description":"Check Daybreak access",
                    "inputSchema":{"type":"object", "properties":{}},
                    "annotations":{"readOnlyHint":true}
                }]}),
                Some("tools/call") => json!({
                    "content":[], "structuredContent":body["params"]["_meta"]
                }),
                _ => return ResponseTemplate::new(202),
            };
            ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc":"2.0", "id":body["id"], "result":result
            }))
        })
        .mount(&server)
        .await;

    let arguments = if matches!(case, AccessCase::NonemptyArguments) {
        r#"{"refresh":true}"#
    } else {
        "{}"
    };
    let home = Arc::new(TempDir::new()?);
    let plugin_root = super::plugins::write_sample_plugin_manifest_and_config(&home);
    let server_config = if matches!(case, AccessCase::InstalledHttp) {
        json!({"url":format!("{}/remote-mcp", server.uri())})
    } else {
        json!({
            "command":stdio_server_bin()?,
            "cwd":plugin_root,
            "environment_id": if remote_url.is_some() { "remote" } else { "local" },
            "default_tools_approval_mode":"approve",
            "env":{"MCP_TEST_DAYBREAK_READ_ONLY":
                if matches!(case, AccessCase::MutableStdio) { "false" } else { "true" }}
        })
    };
    if matches!(case, AccessCase::UnownedStdio) {
        let config_path = home.path().join("config.toml");
        let existing = std::fs::read_to_string(&config_path)?;
        let extra = toml::to_string(&json!({"mcp_servers":{"sample":server_config}}))?;
        std::fs::write(config_path, format!("{existing}\n{extra}"))?;
    } else {
        std::fs::write(
            plugin_root.join(".mcp.json"),
            serde_json::to_vec(&json!({
                "mcpServers":{"sample":server_config}
            }))?,
        )?;
    }
    let auth = if matches!(case, AccessCase::ApiKey) {
        CodexAuth::from_api_key("test-api-key")
    } else if matches!(case, AccessCase::ExternalTokens) {
        CodexAuth::from_external_chatgpt_tokens(
            "header.e30.external",
            "external-account",
            /*chatgpt_plan_type*/ None,
        )?
    } else {
        CodexAuth::create_dummy_chatgpt_auth_for_testing()
    };
    let base_url = server.uri();
    let mut builder = test_codex()
        .with_home(home)
        .with_auth(auth)
        .with_config(move |config| config.chatgpt_base_url = base_url);
    if let Some(remote_url) = &remote_url {
        builder = builder.with_exec_server_url(remote_url.clone());
    }
    let test = builder.build_with_remote_and_local_env(&server).await?;
    if remote_url.is_some() {
        assert!(
            test.thread_manager
                .environment_manager()
                .get_environment("remote")
                .expect("remote executor")
                .is_remote()
        );
        let (config, _) = test.codex.current_mcp_config_and_runtime_context().await;
        assert_eq!(
            config.mcp_server_catalog.configured_servers()["sample"].environment_id,
            "remote"
        );
    }
    wait_for_mcp_server(&test.codex, "sample").await?;
    let mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_tool_search_call("search", &json!({"query":tool_name})),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_function_call_with_namespace(
                    "daybreak-call",
                    "mcp__sample",
                    tool_name,
                    arguments,
                ),
                responses::ev_completed("resp-2"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("done", "done"),
                responses::ev_completed("resp-3"),
            ]),
        ],
    )
    .await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Call the sample Daybreak access tool".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let end = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::McpToolCallEnd(_))
    })
    .await;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.codex.shutdown_and_wait().await?;
    let EventMsg::McpToolCallEnd(end) = end else {
        unreachable!()
    };
    let metadata = end
        .result
        .expect("successful MCP call")
        .structured_content
        .expect("echoed metadata");
    let expected_context = match case {
        AccessCase::InstalledStdio | AccessCase::ExternalTokens => {
            Some(json!({"schemaVersion":1, "entitlements":{
                "cyber_trusted_access":{"schemaVersion":1,"status":"granted","grants":[
                    {"level":"tac1","source":"user"}
                ],"stale":false}
            }}))
        }
        #[cfg(unix)]
        AccessCase::RemoteStdio => None,
        AccessCase::UndeclaredDaybreakName
        | AccessCase::UnownedStdio
        | AccessCase::InstalledHttp
        | AccessCase::MutableStdio
        | AccessCase::NonemptyArguments
        | AccessCase::ApiKey => None,
    };
    assert_eq!(
        metadata.get("openai/entitlementContext"),
        expected_context.as_ref()
    );
    assert_eq!(
        end.plugin_id.as_deref(),
        if matches!(case, AccessCase::UnownedStdio) {
            None
        } else {
            Some("sample@test")
        }
    );
    assert!(metadata["threadId"].is_string());
    assert!(!metadata.to_string().contains("Access Token"));
    assert!(!metadata.to_string().contains("header.e30.external"));
    let requests = server.received_requests().await.expect("recorded requests");
    let access_requests: Vec<_> = requests
        .iter()
        .filter(|request| request.url.path() == "/accounts/verified_access")
        .collect();
    assert_eq!(
        access_requests.len(),
        usize::from(matches!(
            case,
            AccessCase::InstalledStdio | AccessCase::ExternalTokens
        ))
    );
    if let Some(request) = access_requests.first() {
        let (authorization, account_id) = if matches!(case, AccessCase::ExternalTokens) {
            ("Bearer header.e30.external", "external-account")
        } else {
            ("Bearer Access Token", "account_id")
        };
        assert_eq!(request.headers["authorization"], authorization);
        assert_eq!(request.headers["chatgpt-account-id"], account_id);
    }
    assert!(
        mock.requests()[2]
            .function_call_output("daybreak-call")
            .is_object()
    );
    Ok(())
}

struct SelectedAuth(CodexAuth);

impl ExternalAuth for SelectedAuth {
    fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async { Ok(self.0.clone()) })
    }

    fn refresh(&self, _context: ExternalAuthRefreshContext) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async { Ok(self.0.clone()) })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn daybreak_discards_in_flight_account_changes_without_apps() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let base_url = format!("http://{}", listener.local_addr()?);
    let (request_tx, request_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let response_task = tokio::spawn(async move {
        let (mut stream, request) = loop {
            let (stream, _) = listener.accept().await?;
            let mut stream = BufReader::new(stream);
            let mut request = String::new();
            loop {
                let mut line = String::new();
                let read = stream.read_line(&mut line).await?;
                if read == 0 || line == "\r\n" {
                    break;
                }
                request.push_str(&line);
            }
            if request.starts_with("GET /accounts/verified_access ") {
                break (stream, request);
            }
            stream
                .get_mut()
                .write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await?;
        };
        request_tx.send(request).expect("request receiver");
        release_rx.await?;
        let body = json!({"programs":[{"program":"cyber","state":"active","grants":[
            {"level":"tac1","source":"individual"}
        ]}]})
        .to_string();
        stream.get_mut().write_all(format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ).as_bytes()).await?;
        Ok::<_, anyhow::Error>(())
    });
    let home = Arc::new(TempDir::new()?);
    let plugin_root = super::plugins::write_sample_plugin_manifest_and_config(&home);
    std::fs::write(
        plugin_root.join(".mcp.json"),
        serde_json::to_vec(&json!({
            "mcpServers":{"sample":{"command":stdio_server_bin()?,"cwd":plugin_root,
                "environment_id":"local","default_tools_approval_mode":"approve",
                "env":{"MCP_TEST_DAYBREAK_READ_ONLY":"true"}}}
        }))?,
    )?;
    let test = test_codex()
        .with_home(home)
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            config.chatgpt_base_url = base_url;
            config
                .features
                .disable(Feature::Apps)
                .expect("disable Apps");
        })
        .build_with_remote_and_local_env(&server)
        .await?;
    wait_for_mcp_server(&test.codex, "sample").await?;
    responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_tool_search_call(
                    "search",
                    &json!({"query":"get_codex_security_daybreak_access"}),
                ),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_function_call_with_namespace(
                    "daybreak-call",
                    "mcp__sample",
                    "get_codex_security_daybreak_access",
                    "{}",
                ),
                responses::ev_completed("resp-2"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("done", "done"),
                responses::ev_completed("resp-3"),
            ]),
        ],
    )
    .await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Call the sample Daybreak access tool".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let call = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::McpToolCallEnd(_))
    });
    let switch = async {
        let request =
            tokio::time::timeout(std::time::Duration::from_secs(10), request_rx).await??;
        assert!(request.starts_with("GET /accounts/verified_access "));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("chatgpt-account-id: account_id")
        );
        test.thread_manager
            .auth_manager()
            .set_external_auth(Arc::new(SelectedAuth(
                CodexAuth::from_external_chatgpt_tokens(
                    "header.e30.changed",
                    "other-account",
                    /*chatgpt_plan_type*/ None,
                )?,
            )))
            .await?;
        release_tx.send(()).expect("release response");
        Ok::<_, anyhow::Error>(())
    };
    let (result, switched) = tokio::join!(call, switch);
    switched?;
    response_task.await??;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.codex.shutdown_and_wait().await?;
    let EventMsg::McpToolCallEnd(result) = result else {
        unreachable!()
    };
    assert_eq!(
        result
            .result
            .expect("successful call")
            .structured_content
            .expect("metadata")["openai/entitlementContext"],
        json!({
            "schemaVersion":1,"entitlements":{"cyber_trusted_access":{
                "schemaVersion":1,"status":"unknown","grants":[],"stale":false
            }}
        })
    );
    Ok(())
}
