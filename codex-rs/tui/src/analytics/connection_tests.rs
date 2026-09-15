//! Exercise local Analytics plan discovery and report recovery while connected to an app-server.

use super::tests::live;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_client::RemoteAppServerClient;
use codex_app_server_client::RemoteAppServerConnectArgs;
use codex_app_server_client::RemoteAppServerEndpoint;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;

async fn read(socket: &mut WebSocketStream<TcpStream>) -> Value {
    let frame = socket.next().await.unwrap().unwrap();
    serde_json::from_str(frame.to_text().unwrap()).unwrap()
}

async fn remote() -> (RemoteAppServerClient, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
        let initialize = read(&mut socket).await;
        assert_eq!(initialize["method"], "initialize");
        socket
            .send(Message::Text(
                json!({"id": initialize["id"], "result": {
                    "userAgent": "analytics-test", "codexHome": "/server/.codex",
                }})
                .to_string()
                .into(),
            ))
            .await
            .unwrap();
        assert_eq!(read(&mut socket).await["method"], "initialized");
        // Analytics uses the local account without requesting connected-server account data.
        assert!(socket.next().await.unwrap().unwrap().is_close());
    });
    let client = RemoteAppServerClient::connect(RemoteAppServerConnectArgs {
        endpoint: RemoteAppServerEndpoint::WebSocket {
            websocket_url: format!("ws://{address}"),
            auth_token: None,
        },
        client_name: "analytics-test".into(),
        client_version: "0.0.0-test".into(),
        experimental_api: true,
        mcp_server_openai_form_elicitation: false,
        opt_out_notification_methods: Vec::new(),
        channel_capacity: 8,
    })
    .await
    .unwrap();
    (client, server)
}

#[tokio::test]
async fn local_reports_share_an_account_and_recover_profile_errors() {
    use crate::analytics::AnalyticsView;
    use crate::analytics::data::Load;
    use crate::analytics::sections::Section;

    let http = crate::analytics::test_support::server().await;
    let (_home, local) = live(&http, "enterprise").await;
    let (client, server) = remote().await;
    let failed_profile = Mock::given(method("GET"))
        .and(wiremock::matchers::path("/backend-api/wham/profiles/me"))
        .and(header("chatgpt-account-id", "account-a"))
        .respond_with(ResponseTemplate::new(/*s*/ 503))
        .expect(/*r*/ 1)
        .mount_as_scoped(&http)
        .await;
    let mut view = AnalyticsView::new(crate::keymap::RuntimeKeymap::defaults().list);
    view.open(
        AppServerRequestHandle::Remote(client.request_handle()),
        crate::tui::FrameRequester::test_dummy(),
        Vec::new(),
        std::sync::Arc::clone(&local.config),
    );
    crate::analytics::test_support::settle(&mut view).await;
    assert_eq!(
        view.visible_sections(),
        &[
            Section::Summary,
            Section::Credits,
            Section::Usage,
            Section::Plugins,
            Section::Skills,
        ]
    );
    assert!(matches!(view.profile, Load::Error(_)));
    for section in view.visible_sections() {
        if section.report().is_some() {
            assert!(view.sections[*section].history.ready().is_some());
        }
    }
    view.select_summary(/*view*/ None);
    view.end_date = "2026-09-09".parse().unwrap();
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(
        /*width*/ 100, /*height*/ 48,
    ))
    .unwrap();
    terminal
        .draw(|frame| view.render(frame.area(), frame.buffer_mut()))
        .unwrap();
    insta::assert_snapshot!("local_profile_error", terminal.backend().to_string());
    drop(failed_profile);

    let profile = json!({
        "profile":{"display_name":"Local account","username":"local-user"},
        "stats":{"lifetime_tokens":12345,"peak_daily_tokens":12345,
            "daily_usage_buckets":[{"start_date":"2026-09-09","tokens":12345}]}
    });
    Mock::given(method("GET"))
        .and(wiremock::matchers::path("/backend-api/wham/profiles/me"))
        .and(header("chatgpt-account-id", "account-a"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(profile.clone()))
        .expect(/*r*/ 1)
        .mount(&http)
        .await;
    view.refresh();
    crate::analytics::test_support::settle(&mut view).await;
    assert_eq!(
        view.profile.ready(),
        Some(&serde_json::from_value(profile).unwrap())
    );
    view.end_date = "2026-09-09".parse().unwrap();
    terminal
        .draw(|frame| view.render(frame.area(), frame.buffer_mut()))
        .unwrap();
    insta::assert_snapshot!("local_profile_recovered", terminal.backend().to_string());
    for request in http.received_requests().await.unwrap() {
        assert_eq!(
            request.headers.get("chatgpt-account-id").unwrap(),
            "account-a"
        );
    }
    view.cancel_loads();
    client.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn account_lookup_failure_recovers_on_refresh_with_the_server_plan() {
    use crate::analytics::AnalyticsView;
    use crate::analytics::data::Load;
    use crate::analytics::sections::Section;

    let http = crate::analytics::test_support::server().await;
    let (_home, local) = live(&http, "plus").await;
    let (client, server) = remote().await;
    Mock::given(method("GET"))
        .and(wiremock::matchers::path("/backend-api/wham/accounts/check"))
        .respond_with(ResponseTemplate::new(/*s*/ 503))
        .with_priority(/*p*/ 1)
        .expect(/*r*/ 1)
        .up_to_n_times(/*n*/ 1)
        .mount(&http)
        .await;
    let mut view = AnalyticsView::new(crate::keymap::RuntimeKeymap::defaults().list);
    view.open(
        AppServerRequestHandle::Remote(client.request_handle()),
        crate::tui::FrameRequester::test_dummy(),
        Vec::new(),
        std::sync::Arc::clone(&local.config),
    );
    crate::analytics::test_support::settle(&mut view).await;
    assert!(matches!(view.account, Load::Error(_)));
    assert_eq!(
        http.received_requests()
            .await
            .unwrap()
            .iter()
            .map(|request| request.url.path())
            .collect::<Vec<_>>(),
        ["/backend-api/wham/accounts/check"]
    );
    view.end_date = "2026-09-09".parse().unwrap();
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(
        /*width*/ 100, /*height*/ 40,
    ))
    .unwrap();
    terminal
        .draw(|frame| view.render(frame.area(), frame.buffer_mut()))
        .unwrap();
    insta::assert_snapshot!("account_plan_error", terminal.backend().to_string());

    Mock::given(method("GET"))
        .and(wiremock::matchers::path("/backend-api/wham/accounts/check"))
        .and(header("chatgpt-account-id", "account-a"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "accounts": [{"id": "account-a", "plan_type": "enterprise"}]
        })))
        .with_priority(/*p*/ 1)
        .expect(/*r*/ 1)
        .mount(&http)
        .await;
    Mock::given(method("GET"))
        .and(wiremock::matchers::path("/backend-api/wham/profiles/me"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({"stats": {}})))
        .mount(&http)
        .await;
    view.refresh();
    crate::analytics::test_support::settle(&mut view).await;
    assert_eq!(
        view.visible_sections(),
        &[
            Section::Summary,
            Section::Credits,
            Section::Usage,
            Section::Plugins,
            Section::Skills,
        ]
    );
    assert_eq!(
        view.account.ready(),
        Some(&codex_protocol::account::PlanType::Enterprise)
    );
    for section in view.visible_sections() {
        if section.report().is_some() {
            assert!(view.sections[*section].history.ready().is_some());
        }
    }
    http.verify().await;
    view.cancel_loads();
    client.shutdown().await.unwrap();
    server.await.unwrap();
}
