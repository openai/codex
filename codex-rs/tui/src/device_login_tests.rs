#![cfg(test)]
//! Exercise gateway sign-in over the app-server protocol and snapshot the device prompt.

use crate::app_server_session::AppServerSession;
use crate::app_server_session::ThreadParamsMode;
use crate::backend_id::BackendId;
use crate::resume_picker::SessionSelection;
use crate::resume_picker::SessionTarget;
use codex_protocol::ThreadId;
use core_test_support::load_default_config_for_test;
use core_test_support::responses::WebSocketTestServer;
use core_test_support::responses::start_websocket_server;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

#[test]
fn device_prompt_shows_where_to_enter_the_code_and_how_to_cancel() {
    let prompt = codex_app_server_protocol::GatewayOAuthPrompt {
        login_id: "attempt".into(),
        verification_url: "https://login.example.com/device".into(),
        user_code: "ABCD-EFGH".into(),
    };
    let backend = crate::test_backend::VT100Backend::new(/*width*/ 80, /*height*/ 16);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            crate::onboarding::onboarding_screen::render_gateway_device_prompt(
                &prompt,
                frame.area(),
                frame.buffer_mut(),
            )
        })
        .unwrap();
    insta::assert_snapshot!(terminal.backend().to_string());
}

#[tokio::test]
async fn login_success_is_not_lost_while_waiting_for_the_device_code() -> color_eyre::Result<()> {
    let (mut session, gateway) = gateway_session([
        vec![gateway_needs_sign_in(/*id*/ 1)],
        vec![
            json!({"id":"approval","method":"item/fileChange/requestApproval","params":{
                "threadId":"thread", "turnId":"turn", "itemId":"edit", "startedAtMs":0
            }}),
            json!({"method":"account/gatewayOAuth/changed","params":{
                "providerId":"company","loginId":"old","status":"failed","authUrl":null,"error":"Cancelled"
            }}),
            json!({"method":"account/gatewayOAuth/changed","params":{
                "providerId":"company","loginId":"device","status":"succeeded","authUrl":null,"error":null
            }}),
            device_login_response(/*id*/ 2),
        ],
    ]).await?;
    let prompt = start_at_startup(&mut session).await?.expect("device code");
    let origin = session.request_handle();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let event = session.next_event().await.expect("session open");
            if super::handle_event_during_device_login(&prompt, event.event, &origin).await? {
                break;
            }
        }
        Ok::<_, std::io::Error>(())
    })
    .await??;
    let received = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        gateway.wait_for_request(/*connection_index*/ 0, /*request_index*/ 4),
    )
    .await?;
    assert_eq!(received.body_json()["id"], "approval");
    assert_eq!(received.body_json()["error"]["code"], -32000);
    Ok(())
}

#[tokio::test]
async fn losing_the_connection_during_device_login_explains_how_to_recover()
-> color_eyre::Result<()> {
    let (mut session, gateway) = gateway_session([
        vec![gateway_needs_sign_in(/*id*/ 1)],
        vec![device_login_response(/*id*/ 2)],
    ])
    .await?;
    let prompt = start_at_startup(&mut session).await?.expect("device code");
    gateway.shutdown().await;
    let event = tokio::time::timeout(std::time::Duration::from_secs(10), session.next_event())
        .await?
        .expect("disconnect event");
    let error =
        super::handle_event_during_device_login(&prompt, event.event, &session.request_handle())
            .await
            .expect_err("sign-in should stop when the connection closes");
    insta::assert_snapshot!(error.to_string(), @"Codex lost its connection during setup. Restart Codex to continue.");
    Ok(())
}

#[tokio::test]
async fn selecting_openai_skips_gateway_sign_in() -> color_eyre::Result<()> {
    let (mut session, gateway) = gateway_session([vec![json!({"id":1,"result":{
        "providerId":"openai","providerName":"OpenAI","required":false,"status":null
    }})]])
    .await?;
    session.model_provider_override = Some("openai".into());
    let home = tempfile::tempdir()?;
    let config = load_default_config_for_test(&home).await;
    let prompt = super::start_if_needed(
        &mut session,
        &SessionSelection::StartFresh,
        &config,
        async { panic!("OpenAI sign-in should not probe for a browser") },
    )
    .await?;
    assert_eq!(prompt, None);
    assert_eq!(
        gateway.single_connection()[2].body_json()["params"],
        json!({"modelProvider":"openai"})
    );
    assert!(
        gateway
            .single_connection()
            .iter()
            .all(|request| request.body_json()["method"] != "account/gatewayOAuth/login")
    );
    Ok(())
}

#[tokio::test]
async fn legacy_servers_skip_device_login_but_current_servers_report_read_errors()
-> color_eyre::Result<()> {
    let reason = "Named read rejected";
    for legacy in [true, false] {
        let mut state =
            json!({"providerId":"openai","providerName":"OpenAI","required":false,"status":null});
        if !legacy {
            state["supportsDeviceAuth"] = json!(false);
        }
        let (mut session, gateway) = gateway_session([
            vec![json!({"id":1,"error":{"code":-32600,"message":reason}})],
            vec![json!({"id":2,"result":state})],
        ])
        .await?;
        session.model_provider_override = Some("company".into());
        let result = start_at_startup(&mut session).await;
        if legacy {
            assert_eq!(result?, None);
        } else {
            assert!(result.unwrap_err().to_string().contains(reason));
        }
        let probe = gateway.single_connection()[3].body_json();
        assert_eq!(probe["method"], "account/gatewayOAuth/read");
        assert!(probe["params"].is_null());
    }
    Ok(())
}

#[tokio::test]
async fn resuming_a_running_thread_signs_in_to_its_saved_gateway() -> color_eyre::Result<()> {
    let home = tempfile::tempdir()?;
    let id = ThreadId::new();
    let thread = json!({"id":id, "sessionId":id, "preview":"", "ephemeral":false,
        "modelProvider":"company", "status":{"type":"idle"}, "createdAt":0, "updatedAt":0,
        "cwd":home.path(), "cliVersion":"1", "source":"cli", "turns":[]});
    let (mut session, gateway) = gateway_session([
        vec![json!({"id":1,"result":{"thread":thread}})],
        vec![gateway_needs_sign_in(/*id*/ 2)],
        vec![device_login_response(/*id*/ 3)],
    ])
    .await?;
    session.model_provider_override = Some("openai".into());
    let resume = SessionSelection::Resume(SessionTarget {
        backend_id: session.backend_id(),
        thread_id: id,
        path: None,
        cwd: None,
        history_mode: None,
    });
    let config = load_default_config_for_test(&home).await;
    let prompt =
        super::start_if_needed(&mut session, &resume, &config, std::future::ready(true)).await?;
    assert!(prompt.is_some());
    let requests = gateway.single_connection();
    assert_eq!(
        requests[3].body_json()["params"],
        json!({"modelProvider":"company"})
    );
    assert_eq!(
        requests[4].body_json()["params"],
        json!({"modelProvider":"company","deviceAuth":true})
    );
    Ok(())
}

async fn start_at_startup(
    session: &mut AppServerSession,
) -> color_eyre::Result<Option<codex_app_server_protocol::GatewayOAuthPrompt>> {
    let home = tempfile::tempdir()?;
    let config = load_default_config_for_test(&home).await;
    Ok(super::start_if_needed(
        session,
        &SessionSelection::StartFresh,
        &config,
        std::future::ready(true),
    )
    .await?)
}

async fn gateway_session(
    replies: impl IntoIterator<Item = Vec<Value>>,
) -> color_eyre::Result<(AppServerSession, WebSocketTestServer)> {
    let mut script = vec![
        vec![json!({"id": "initialize", "result": {"userAgent": "gateway-test/1.0"}})],
        vec![], // initialized notification
    ];
    script.extend(replies);
    // Keep the connection open after the final reply. An extra request is recorded and closes it.
    script.push(vec![]);
    let gateway = start_websocket_server(vec![script]).await;
    let client =
        crate::connect_remote_app_server(crate::resolve_remote_addr(gateway.uri())?).await?;
    Ok((
        AppServerSession::new(BackendId::new(), client, ThreadParamsMode::Embedded),
        gateway,
    ))
}

fn device_login_response(id: i64) -> Value {
    json!({"id": id, "result": {"prompt": {
        "loginId": "device", "verificationUrl": "https://login.example/device", "userCode": "ABCD-EFGH"
    }}})
}

fn gateway_needs_sign_in(id: i64) -> Value {
    json!({"id": id, "result": {
        "providerId": "company", "providerName": "Company", "required": true,
        "supportsDeviceAuth": true, "status": "notReady"
    }})
}
