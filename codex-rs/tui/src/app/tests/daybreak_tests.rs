use super::*;
use crate::daybreak::Notice;
use app_test_support::ChatGptAuthFixture;
use app_test_support::write_chatgpt_auth;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::AccountUpdatedNotification;
use codex_app_server_protocol::AuthMode;
use codex_app_server_protocol::CodexErrorInfo;
use codex_app_server_protocol::ErrorNotification;
use codex_login::AuthCredentialsStoreMode;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test]
async fn cyber_refusal_reads_eligibility_without_changing_the_model() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let backend = wiremock::MockServer::start().await;
    app.config.chatgpt_base_url = backend.uri();
    // Embedded account discovery reloads the bootstrap URL from disk.
    std::fs::write(
        app.config.codex_home.join("config.toml"),
        format!("chatgpt_base_url = {:?}\n", backend.uri()),
    )?;
    app_test_support::mount_workspace_routing(&backend).await;
    app.config.cli_auth_credentials_store_mode = AuthCredentialsStoreMode::File;
    write_chatgpt_auth(
        &app.config.codex_home,
        ChatGptAuthFixture::new("test-token")
            .account_id("account")
            .chatgpt_user_id("user"),
        AuthCredentialsStoreMode::File,
    )
    .expect("write synthetic auth");
    let server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let thread_id = ThreadId::new();
    let mut session = test_thread_session(thread_id, app.config.cwd.to_path_buf());
    session.model_provider_id = "openai".to_string();
    app.enqueue_primary_thread_session(session, Vec::new())
        .await?;
    for (status, enrolled, model, replace_account, expected) in [
        (200, true, "gpt-5.6-sol", false, Notice::Limited),
        (200, false, "gpt-5.6-sol", false, Notice::Apply),
        (503, false, "gpt-5.6-sol", false, Notice::Limited),
        (200, true, "gpt-6-astra", false, Notice::Astra),
        (200, false, "gpt-6-astra", false, Notice::Astra),
        (503, false, "gpt-6-astra", false, Notice::Astra),
        (200, true, "gpt-6-astra-wm", false, Notice::Astra),
        (200, true, "unmapped-model", false, Notice::Limited),
        (200, false, "gpt-5.6-sol", true, Notice::Limited),
    ] {
        backend.reset().await;
        app_test_support::mount_workspace_routing(&backend).await;
        let codex_home = app.config.codex_home.clone();
        let response = wiremock::ResponseTemplate::new(status)
            .set_delay(if model == "gpt-5.6-sol" && enrolled { Duration::from_secs(1) } else { Duration::ZERO })
            .set_body_json(json!({"programs": [{"program":"cyber", "state": if enrolled { "active" } else { "inactive" }, "grants": if enrolled { json!([{"level":"tac2"}]) } else { json!([]) }}]}));
        wiremock::Mock::given(wiremock::matchers::path("/accounts/verified_access"))
            .and(wiremock::matchers::header(
                "authorization",
                "Bearer test-token",
            ))
            .respond_with(move |_: &wiremock::Request| {
                if replace_account {
                    write_chatgpt_auth(
                        &codex_home,
                        ChatGptAuthFixture::new("test-token").account_id("other-account"),
                        AuthCredentialsStoreMode::File,
                    )
                    .unwrap();
                }
                response.clone()
            })
            .expect(1)
            .mount(&backend)
            .await;
        // Account discovery starts before a subsequent model switch to Sol.
        app.chat_widget.set_model("gpt-6-astra");
        app.handle_app_server_event(
            &server,
            AppServerEvent::ServerNotification(Box::new(ServerNotification::AccountUpdated(
                AccountUpdatedNotification {
                    auth_mode: Some(AuthMode::Chatgpt),
                    plan_type: None,
                },
            ))),
        )
        .await;
        if model != "gpt-5.6-sol" || !enrolled {
            wait_for_notice(&app.chat_widget.cyber_policy_notice).await;
        }
        app.active_thread_id = Some(thread_id);
        app.chat_widget.set_model(model);
        while events.try_recv().is_ok() {}
        // In the slow Sol case, discovery is still pending: do not block input on it.
        tokio::time::timeout(
            Duration::from_millis(250),
            app.handle_app_server_event(
                &server,
                AppServerEvent::ServerNotification(Box::new(ServerNotification::Error(
                    ErrorNotification {
                        error: AppServerTurnError {
                            misalignment: None,
                            message: "server fallback".into(),
                            codex_error_info: Some(CodexErrorInfo::CyberPolicy),
                            additional_details: None,
                        },
                        will_retry: false,
                        thread_id: thread_id.to_string(),
                        turn_id: "turn".into(),
                    },
                ))),
            ),
        )
        .await
        .expect("refusal must not wait for eligibility");
        let event = app.active_thread_rx.as_mut().unwrap().recv().await.unwrap();
        app.handle_thread_event_now(event);
        let cell = std::iter::from_fn(|| events.try_recv().ok())
            .find_map(|event| match event {
                AppEvent::InsertHistoryCell(cell) => Some(cell),
                _ => None,
            })
            .expect("cyber refusal cell");
        assert_eq!(
            cell.display_lines(/*width*/ 80),
            history_cell::new_cyber_policy_error_event(expected).display_lines(/*width*/ 80),
            "{model}"
        );
        assert_eq!(
            wait_for_notice(&app.chat_widget.cyber_policy_notice)
                .await
                .for_model(model),
            expected
        );
        backend.verify().await;
        assert_eq!(app.chat_widget.current_model(), model);
    }
    server.shutdown().await?;
    Ok(())
}

async fn wait_for_notice(cache: &crate::daybreak::NoticeCache) -> Notice {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(notice) = cache.get() {
                return *notice;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("eligibility should finish")
}
