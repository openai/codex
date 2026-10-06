//! Verify Reserve recovery through accepted usage reads and the task settings RPC.

use super::*;
use crate::chatwidget::UserMessage;
use codex_app_server_protocol::GetAccountRateLimitsResponse;
use codex_app_server_protocol::ThreadSettingsUpdateParams;
use pretty_assertions::assert_eq;
use serde_json::json;

pub(super) fn reserve_response() -> GetAccountRateLimitsResponse {
    serde_json::from_value(json!({
        "accountId": "workspace-a", "ordinaryUsageAllowed": false,
        "rateLimits": {"limitId": "codex", "primary": {"usedPercent": 100}},
        "rateLimitsByLimitId": {"base_model_inference": {
            "limitId": "base_model_inference", "limitName": "gpt-reserve",
            "primary": {"usedPercent": 48, "windowDurationMins": 10080}
        }},
        "rateLimitUpsell": {
            "banner_type": "luna_reserve", "presentation": "dismissible",
            "title": "You’re now using Luna, a faster model for simpler tasks.",
            "description": "Add credits or upgrade to continue using the most advanced models.",
            "ctas": [{"action": "add_credits", "label": "Add credits"}]
        }
    }))
    .unwrap()
}

pub(super) fn configure_reserve_catalog(app: &mut App) {
    let mut models = app.model_catalog.try_list_models().unwrap();
    let mut reserve = models
        .iter()
        .find(|model| model.model == "gpt-5.5")
        .unwrap()
        .clone();
    reserve.model = "gpt-reserve".into();
    reserve.show_in_picker = false;
    models.push(reserve);
    let original = models
        .iter_mut()
        .find(|model| model.model == "gpt-5.5")
        .unwrap();
    let mut high = original.supported_reasoning_efforts[0].clone();
    high.effort = ReasoningEffortConfig::High;
    original.supported_reasoning_efforts.push(high);
    Arc::make_mut(&mut app.model_catalog).models = models;
    let mut tui = crate::tui::test_support::make_test_tui().unwrap();
    let mut init = app.chatwidget_init_for_forked_or_resumed_thread(
        &mut tui,
        app.config.clone(),
        /*initial_user_message*/ None,
    );
    init.has_chatgpt_account = true;
    init.has_codex_backend_auth = true;
    app.replace_chat_widget(ChatWidget::new_with_app_event(init));
    app.chat_widget
        .handle_thread_session(app.primary_session_configured.clone().unwrap());
    app.chat_widget.set_model("gpt-5.5");
}

#[tokio::test]
async fn luna_reserve_recovery_survives_task_reconstruction() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let (mut server, requests, proxy) =
        backend_banner_fallback_tests::start_fallback_thread(&mut app).await?;
    configure_reserve_catalog(&mut app);
    app.chat_widget
        .set_reasoning_effort(Some(ReasoningEffortConfig::High));
    app.chat_widget.update_backend_banner(&reserve_response());
    app.apply_backend_banner_fallback(&mut server).await;
    let mut resumed = app.primary_session_configured.clone().unwrap();
    resumed.model = "gpt-reserve".into();
    resumed.collaboration_mode = Some(Box::new(app.chat_widget.effective_collaboration_mode()));
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut init = app.chatwidget_init_for_forked_or_resumed_thread(
        &mut tui,
        app.config.clone(),
        /*initial_user_message*/ None,
    );
    init.has_chatgpt_account = true;
    init.has_codex_backend_auth = true;
    // No in-memory transition state is inherited: resume must recover the persisted target.
    app.replace_chat_widget(ChatWidget::new_with_app_event(init));
    app.chat_widget.handle_thread_session(resumed);
    assert_eq!(app.chat_widget.current_model(), "gpt-reserve");
    let mut recovered = reserve_response();
    recovered.ordinary_usage_allowed = Some(true);
    recovered.rate_limit_upsell = None;
    app.chat_widget.update_backend_banner(&recovered);
    app.apply_backend_banner_fallback(&mut server).await;
    assert_eq!(app.chat_widget.current_model(), "gpt-5.5");
    assert_eq!(
        app.chat_widget.current_reasoning_effort(),
        Some(ReasoningEffortConfig::High)
    );
    assert_eq!(
        requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request.method == "thread/settings/update")
            .count(),
        2
    );
    assert!(
        !app.config
            .codex_home
            .join("tui-luna-reserve")
            .join(format!("{}.json", app.active_thread_id.unwrap()))
            .exists()
    );
    server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn luna_reserve_entry_dispatches_an_already_queued_turn_with_accepted_settings() -> Result<()>
{
    for mode in [ModeKind::Default, ModeKind::Plan] {
        let (mut app, mut events, _ops) = make_test_app_with_channels().await;
        let (mut server, requests, proxy) =
            backend_banner_fallback_tests::start_fallback_thread(&mut app).await?;
        configure_reserve_catalog(&mut app);
        app.chat_widget
            .set_feature_enabled(Feature::FastMode, /*enabled*/ true);
        app.chat_widget.set_service_tier(Some("fast".into()));
        if mode == ModeKind::Plan {
            app.chat_widget
                .handle_key_event(KeyEvent::from(KeyCode::BackTab));
        }
        app.chat_widget
            .restore_user_message_to_composer(UserMessage::from("continue"));
        app.chat_widget
            .handle_key_event(KeyEvent::from(KeyCode::Enter));
        let pending = std::iter::from_fn(|| events.try_recv().ok())
            .find(|event| matches!(event, AppEvent::CodexOp(AppCommand::UserTurn { .. })))
            .expect("queued turn before the usage response");
        requests.lock().unwrap().clear();
        let mut tui = crate::tui::test_support::make_test_tui()?;
        let generation = app.rate_limit_hard_stop_generation;
        app.handle_event(
            &mut tui,
            &mut server,
            AppEvent::RateLimitsLoaded {
                request_id: 1,
                origin: RateLimitRefreshOrigin::Periodic,
                hard_stop_generation: generation,
                result: Ok(reserve_response()),
            },
        )
        .await?;
        app.handle_event(&mut tui, &mut server, pending).await?;
        let sent = requests.lock().unwrap().clone();
        let methods: Vec<_> = sent.iter().map(|request| request.method.as_str()).collect();
        assert_eq!(methods, ["thread/settings/update", "turn/start"]);
        let settings: ThreadSettingsUpdateParams =
            serde_json::from_value(sent[0].params.clone().unwrap())?;
        let turn: codex_app_server_protocol::TurnStartParams =
            serde_json::from_value(sent[1].params.clone().unwrap())?;
        assert_eq!(
            (
                turn.model,
                turn.effort,
                turn.service_tier,
                turn.collaboration_mode
            ),
            (
                settings.model,
                settings.effort,
                settings.service_tier,
                settings.collaboration_mode
            )
        );
        assert_eq!(app.chat_widget.effective_collaboration_mode().mode, mode);
        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}

#[tokio::test]
async fn luna_reserve_recovery_restores_task_and_pending_turn_after_fresh_backend_read()
-> Result<()> {
    for has_credits in [false, true] {
        let (mut app, mut events, _ops) = make_test_app_with_channels().await;
        let (mut server, requests, proxy) =
            backend_banner_fallback_tests::start_fallback_thread(&mut app).await?;
        configure_reserve_catalog(&mut app);
        app.chat_widget
            .set_feature_enabled(Feature::FastMode, /*enabled*/ false);
        app.chat_widget
            .set_feature_enabled(Feature::UltrafastMode, /*enabled*/ false);
        app.chat_widget
            .set_reasoning_effort(Some(ReasoningEffortConfig::High));
        let original_mode = app.chat_widget.effective_collaboration_mode();
        let config_path = app.config.codex_home.join("config.toml");
        let saved_config = std::fs::read(&config_path).ok();
        let mut tui = crate::tui::test_support::make_test_tui()?;
        requests.lock().unwrap().clear();
        let generation = app.rate_limit_hard_stop_generation;
        app.handle_event(
            &mut tui,
            &mut server,
            AppEvent::RateLimitsLoaded {
                request_id: 1,
                origin: RateLimitRefreshOrigin::Periodic,
                hard_stop_generation: generation,
                result: Ok(reserve_response()),
            },
        )
        .await?;
        assert_eq!(app.chat_widget.current_model(), "gpt-reserve");

        // A prompt composed before the next accepted read must use the recovered model, once.
        app.chat_widget
            .handle_key_event(KeyEvent::from(KeyCode::Esc));
        app.chat_widget
            .restore_user_message_to_composer(UserMessage::from("continue"));
        app.chat_widget
            .handle_key_event(KeyEvent::from(KeyCode::Enter));
        let mut pending = std::iter::from_fn(|| events.try_recv().ok())
            .find_map(|event| match event {
                AppEvent::CodexOp(op @ AppCommand::UserTurn { .. }) => Some(op),
                _ => None,
            })
            .expect("queued user turn");
        assert_matches!(&pending, AppCommand::UserTurn {model, ..} if model == "gpt-reserve");
        while events.try_recv().is_ok() {}

        let mut recovered = reserve_response();
        recovered.ordinary_usage_allowed = Some(!has_credits);
        recovered.rate_limits.credits = Some(codex_app_server_protocol::CreditsSnapshot {
            has_credits,
            unlimited: false,
            balance: None,
        });
        recovered.rate_limit_upsell = None;
        // A rounded percentage is not an authorization decision; backend allowed=true wins.
        for (request_id, hard_stop_generation, expected) in [
            (2, generation.wrapping_sub(1), "gpt-reserve"),
            (3, generation, "gpt-5.5"),
            (4, generation, "gpt-5.5"),
        ] {
            app.handle_event(
                &mut tui,
                &mut server,
                AppEvent::RateLimitsLoaded {
                    request_id,
                    origin: RateLimitRefreshOrigin::Periodic,
                    hard_stop_generation,
                    result: Ok(recovered.clone()),
                },
            )
            .await?;
            assert_eq!(app.chat_widget.current_model(), expected);
        }
        assert_eq!(
            app.chat_widget.effective_collaboration_mode(),
            original_mode
        );
        assert!(!render_bottom_popup(&app.chat_widget, /*width*/ 100).contains("Luna Reserve"));
        app.chat_widget
            .apply_reserve_fallback_to_pending_turn(&mut pending);
        assert_matches!(pending, AppCommand::UserTurn { model, effort, collaboration_mode, .. }
        if model == "gpt-5.5" && effort == original_mode.reasoning_effort()
            && collaboration_mode == Some(original_mode.clone()));
        let sent = requests.lock().unwrap().clone();
        assert_eq!(sent.len(), 2);
        let restored: ThreadSettingsUpdateParams =
            serde_json::from_value(sent[1].params.clone().unwrap())?;
        assert_eq!(
            restored,
            ThreadSettingsUpdateParams {
                thread_id: app.active_thread_id.unwrap().to_string(),
                model: Some("gpt-5.5".into()),
                effort: Some(ReasoningEffortConfig::High),
                collaboration_mode: Some(original_mode),
                service_tier: None,
                ..Default::default()
            }
        );
        assert_eq!(std::fs::read(&config_path).ok(), saved_config);
        let queued = std::iter::from_fn(|| events.try_recv().ok()).collect::<Vec<_>>();
        assert!(
            !queued.iter().any(|event| matches!(
                event,
                AppEvent::CodexOp(_) | AppEvent::PersistModelSelection { .. }
            )),
            "recovery must not replay a turn or change global defaults"
        );
        let notice = queued
            .iter()
            .find_map(|event| match event {
                AppEvent::InsertHistoryCell(cell) => {
                    Some(lines_to_single_string(&cell.display_lines(/*width*/ 100)))
                }
                _ => None,
            })
            .expect("recovery notice");
        insta::assert_snapshot!("luna_reserve_recovery_notice", notice);
        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}

#[tokio::test]
async fn luna_reserve_recovery_requires_permission_and_no_remaining_blocker() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let (mut server, requests, proxy) =
        backend_banner_fallback_tests::start_fallback_thread(&mut app).await?;
    configure_reserve_catalog(&mut app);
    app.chat_widget.update_backend_banner(&reserve_response());
    app.apply_backend_banner_fallback(&mut server).await;
    requests.lock().unwrap().clear();
    let mut recovered = reserve_response();
    recovered.rate_limit_upsell = None;
    recovered.rate_limits.primary.as_mut().unwrap().used_percent = 0;
    recovered.rate_limits.primary.as_mut().unwrap().resets_at = Some(0);
    for allowed in [None, Some(false)] {
        recovered.ordinary_usage_allowed = allowed;
        app.chat_widget.update_backend_banner(&recovered);
        app.apply_backend_banner_fallback(&mut server).await;
        assert_eq!(app.chat_widget.current_model(), "gpt-reserve");
    }
    recovered.ordinary_usage_allowed = Some(true);
    for blocker in [
        json!({"accountId": null, "ordinaryUsageAllowed": null}),
        json!({"rateLimitUpsell": {"unsupported": true}}),
        json!({"rateLimits": {"spendControlReached": true}}),
        json!({"rateLimits": {"rateLimitReachedType": "workspace_owner_usage_limit_reached"}}),
    ] {
        let mut raw = serde_json::to_value(&recovered)?;
        for (key, value) in blocker.as_object().unwrap() {
            raw[key] = value.clone();
        }
        app.chat_widget
            .update_backend_banner(&serde_json::from_value(raw)?);
        app.apply_backend_banner_fallback(&mut server).await;
        assert_eq!(app.chat_widget.current_model(), "gpt-reserve");
    }
    // Reset redemption clears presentation before the authoritative follow-up read.
    app.chat_widget.clear_backend_banner();
    app.chat_widget.update_backend_banner(&recovered);
    app.apply_backend_banner_fallback(&mut server).await;
    assert_eq!(app.chat_widget.current_model(), "gpt-5.5");
    assert_eq!(requests.lock().unwrap().len(), 1);
    server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn luna_reserve_recovery_does_not_override_manual_choice_or_account_change() -> Result<()> {
    for change_account in [false, true] {
        let (mut app, _events, _ops) = make_test_app_with_channels().await;
        let (mut server, requests, proxy) =
            backend_banner_fallback_tests::start_fallback_thread(&mut app).await?;
        configure_reserve_catalog(&mut app);
        app.chat_widget.update_backend_banner(&reserve_response());
        app.apply_backend_banner_fallback(&mut server).await;
        requests.lock().unwrap().clear();
        let mut recovered = reserve_response();
        recovered.ordinary_usage_allowed = Some(true);
        recovered.rate_limit_upsell = None;
        let expected = if change_account {
            recovered.account_id = Some("workspace-b".into());
            "gpt-reserve"
        } else {
            app.chat_widget.set_model("gpt-5.6-terra");
            "gpt-5.6-terra"
        };
        app.chat_widget.update_backend_banner(&recovered);
        app.apply_backend_banner_fallback(&mut server).await;
        recovered.account_id = Some("workspace-a".into());
        app.chat_widget.update_backend_banner(&recovered);
        app.apply_backend_banner_fallback(&mut server).await;
        assert_eq!(app.chat_widget.current_model(), expected);
        assert!(requests.lock().unwrap().is_empty());
        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}
