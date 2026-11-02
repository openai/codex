//! Exercises CLI account checks through thread/resume, including attachment to loaded threads.

use super::*;
use app_test_support::ChatGptIdTokenClaims;
use app_test_support::encode_id_token;
use codex_app_server_protocol::LoginAccountResponse;
use pretty_assertions::assert_eq;
use test_case::test_case;

#[derive(Clone, Copy)]
enum AccountCheck {
    Default,
    Enabled,
}

#[derive(Clone, Copy)]
enum ResumeOutcome {
    Allowed,
    Denied,
}

#[test_case("codex-tui", AccountCheck::Enabled, Some(("other-user", "account")), Some(("user", "account")), ThreadHistoryMode::Legacy, ResumeOutcome::Denied; "different user")]
#[test_case("codex_exec", AccountCheck::Enabled, Some(("user", "other-account")), Some(("user", "account")), ThreadHistoryMode::Legacy, ResumeOutcome::Denied; "different workspace exec")]
#[test_case("codex-tui", AccountCheck::Enabled, Some(("user", "other-account")), Some(("user", "account")), ThreadHistoryMode::Paginated, ResumeOutcome::Denied; "paginated different workspace")]
#[test_case("codex-tui", AccountCheck::Enabled, Some(("user", "account")), Some(("user", "account")), ThreadHistoryMode::Legacy, ResumeOutcome::Allowed; "same owner")]
#[test_case("codex-tui", AccountCheck::Enabled, Some(("user", "account")), Some(("user", "account")), ThreadHistoryMode::Paginated, ResumeOutcome::Allowed; "paginated same owner")]
#[test_case("codex-tui", AccountCheck::Enabled, None, Some(("user", "account")), ThreadHistoryMode::Legacy, ResumeOutcome::Allowed; "untagged")]
#[test_case("codex-tui", AccountCheck::Default, Some(("other-user", "other-account")), Some(("user", "account")), ThreadHistoryMode::Legacy, ResumeOutcome::Allowed; "disabled by default")]
#[test_case("third-party-client", AccountCheck::Enabled, Some(("other-user", "other-account")), Some(("user", "account")), ThreadHistoryMode::Legacy, ResumeOutcome::Allowed; "third party")]
#[test_case("codex-tui", AccountCheck::Enabled, Some(("user", "account")), None, ThreadHistoryMode::Legacy, ResumeOutcome::Allowed; "api key")]
#[tokio::test]
async fn thread_resume_account_check(
    client_name: &str,
    account_check: AccountCheck,
    creator: Option<(&str, &str)>,
    current_owner: Option<(&str, &str)>,
    history_mode: ThreadHistoryMode,
    expected: ResumeOutcome,
) -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    let mut config = mock_responses_config(&server.uri());
    if matches!(account_check, AccountCheck::Enabled) {
        config = config.enable_feature(Feature::CliResumeAccountCheck);
    }
    config.write(codex_home.path())?;
    if let Some((user, account)) = current_owner {
        write_chatgpt_auth(
            codex_home.path(),
            ChatGptAuthFixture::new("test-token")
                .chatgpt_user_id(user)
                .chatgpt_account_id(account)
                .account_id(account),
            AuthCredentialsStoreMode::File,
        )?;
    }
    let create_rollout = match history_mode {
        ThreadHistoryMode::Legacy => create_fake_rollout,
        ThreadHistoryMode::Paginated => create_fake_paginated_rollout,
    };
    let thread_id = create_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let path = rollout_path(codex_home.path(), "2025-01-05T12-00-00", &thread_id);
    if let Some((user, account)) = creator {
        let contents = std::fs::read_to_string(&path)?;
        let (first, rest) = contents.split_once('\n').expect("rollout metadata");
        let mut first: serde_json::Value = serde_json::from_str(first)?;
        first["payload"]["creator_user_id"] = json!(user);
        first["payload"]["creator_account_id"] = json!(account);
        if history_mode == ThreadHistoryMode::Paginated && matches!(expected, ResumeOutcome::Denied)
        {
            // The owner mismatch must win even when loading the inherited history would fail.
            first["payload"]["history_base"] = json!({
                "thread_id": ThreadId::new(),
                "end_ordinal_exclusive": 1,
                "end_byte_offset": 1,
            });
        }
        std::fs::write(&path, format!("{first}\n{rest}"))?;
    }
    let original_meta = serde_json::to_value(read_session_meta_line(&path).await?)?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[(
            "OPENAI_API_KEY",
            current_owner.is_none().then_some("test-api-key"),
        )])
        .build()
        .await?;
    mcp.initialize_with_client_info(ClientInfo {
        name: client_name.to_string(),
        title: None,
        version: "0.0.0".to_string(),
    })
    .await?;

    // Check both direct ID lookup and explicit path lookup. Successful second
    // resumes also exercise attachment to the already loaded thread.
    for resume_path in [None, Some(path.clone())] {
        let resume_id = mcp
            .send_thread_resume_request(ThreadResumeParams {
                thread_id: thread_id.clone(),
                path: resume_path,
                exclude_turns: history_mode == ThreadHistoryMode::Paginated,
                ..Default::default()
            })
            .await?;
        match expected {
            ResumeOutcome::Allowed => {
                let response: ThreadResumeResponse = mcp.read_response(resume_id).await?;
                assert_eq!(response.thread.id, thread_id);
            }
            ResumeOutcome::Denied => {
                let error = mcp
                    .read_stream_until_error_message(RequestId::Integer(resume_id))
                    .await?;
                insta::assert_snapshot!(
                    "thread_resume_account_mismatch",
                    serde_json::to_string_pretty(&error.error)?,
                );
                let list_id = mcp
                    .send_thread_loaded_list_request(ThreadLoadedListParams::default())
                    .await?;
                let loaded: ThreadLoadedListResponse = mcp.read_response(list_id).await?;
                assert_eq!(
                    loaded,
                    ThreadLoadedListResponse {
                        data: Vec::new(),
                        next_cursor: None
                    },
                );
            }
        }
    }
    assert_eq!(
        serde_json::to_value(read_session_meta_line(&path).await?)?,
        original_meta,
    );
    Ok(())
}

#[tokio::test]
async fn thread_resume_account_check_loaded_thread_recovers_after_switching_back() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri())
        .enable_feature(Feature::CliResumeAccountCheck)
        .write(codex_home.path())?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("test-token")
            .chatgpt_user_id("user")
            .chatgpt_account_id("account")
            .account_id("account"),
        AuthCredentialsStoreMode::File,
    )?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build()
        .await?;
    mcp.initialize_with_client_info(ClientInfo {
        name: "codex-tui".to_string(),
        title: None,
        version: "0.0.0".to_string(),
    })
    .await?;
    let start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams::default())
        .await?;
    let ThreadStartResponse { thread, .. } = mcp.read_response(start_id).await?;
    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "Save this thread".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = mcp.read_response(turn_id).await?;
    mcp.read_stream_until_notification_message("turn/completed")
        .await?;
    let read_id = mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread.id,
            include_turns: false,
        })
        .await?;
    let ThreadReadResponse { thread, .. } = mcp.read_response(read_id).await?;
    let path = thread.path.as_ref().expect("persisted thread path");
    let original_meta = serde_json::to_value(read_session_meta_line(path.as_path()).await?)?;
    assert_eq!(
        (
            original_meta["creator_user_id"].clone(),
            original_meta["creator_account_id"].clone()
        ),
        (json!("user"), json!("account")),
    );

    for (account, expected) in [
        ("other-account", ResumeOutcome::Denied),
        ("account", ResumeOutcome::Allowed),
    ] {
        let token = encode_id_token(
            &ChatGptIdTokenClaims::new()
                .chatgpt_user_id("user")
                .chatgpt_account_id(account),
        )?;
        let login_id = mcp
            .send_chatgpt_auth_tokens_login_request(
                token,
                account.to_string(),
                /*chatgpt_plan_type*/ None,
            )
            .await?;
        let _: LoginAccountResponse = mcp.read_response(login_id).await?;
        let resume_id = mcp
            .send_thread_resume_request(ThreadResumeParams {
                thread_id: thread.id.clone(),
                exclude_turns: true,
                ..Default::default()
            })
            .await?;
        match expected {
            ResumeOutcome::Denied => {
                let error = mcp
                    .read_stream_until_error_message(RequestId::Integer(resume_id))
                    .await?;
                insta::assert_snapshot!(
                    "thread_resume_account_mismatch",
                    serde_json::to_string_pretty(&error.error)?,
                );
            }
            ResumeOutcome::Allowed => {
                let response: ThreadResumeResponse = mcp.read_response(resume_id).await?;
                assert_eq!(response.thread.id, thread.id);
            }
        }
    }
    assert_eq!(
        serde_json::to_value(read_session_meta_line(path.as_path()).await?)?,
        original_meta,
    );
    Ok(())
}
