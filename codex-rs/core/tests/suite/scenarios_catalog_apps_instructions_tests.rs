#![cfg(test)]

//! Complete request history for catalog Apps wording across model switches and resumes.

use super::*;
use codex_protocol::protocol::ThreadSettingsOverrides;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::submit_thread_settings;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_apps_instructions_follow_selected_model() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let apps = AppsTestServer::mount_searchable(&server).await?;
    let configure = move |config: &mut Config| {
        configure_scenario_catalog(config);
        config.workspace_roots = vec![config.cwd.clone()];
        config.chatgpt_base_url = apps.chatgpt_base_url.clone();
        for feature in [Feature::Apps, Feature::CatalogAppsInstructions] {
            config.features.enable(feature).unwrap();
        }
        let catalog = config.model_catalog.as_mut().unwrap();
        let base = catalog
            .models
            .iter()
            .find(|model| model.slug == "gpt-5.5")
            .unwrap()
            .clone();
        for (slug, wording, include_guidance) in [
            ("apps-bundled", None, true),
            ("apps-no-guidance", Some("Calendar guidance B."), false),
            (
                "apps-a",
                Some("  Calendar guidance A: {{literal}}.\n"),
                true,
            ),
            ("apps-b", Some("Calendar guidance B."), true),
            ("apps-clear", Some(""), true),
        ] {
            let mut model = base.clone();
            model.slug = slug.to_string();
            model.include_apps_usage_instructions = include_guidance;
            model.model_messages = Some(
                serde_json::from_value(json!({"apps": {"usage_instructions": wording}})).unwrap(),
            );
            catalog.models.push(model);
        }
    };
    let mut builder = test_codex()
        .with_model("apps-bundled")
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(configure.clone());
    let test = builder.build_with_auto_env(&server).await?;
    let mock = mount_sse_sequence(
        &server,
        (0..8)
            .map(|index| sse(vec![ev_completed(&format!("apps-{index}"))]))
            .collect(),
    )
    .await;

    test.submit_text_turn("Check with bundled guidance.")
        .await?;
    // Selecting catalog wording cannot override the receiving model's availability gate.
    for (model, prompt) in [
        ("apps-no-guidance", "Check while guidance is unavailable."),
        ("apps-a", "Check the calendar."),
        ("apps-b", "Check again with the new model."),
    ] {
        submit_thread_settings(
            &test.codex,
            ThreadSettingsOverrides {
                model: Some(model.to_string()),
                ..Default::default()
            },
        )
        .await?;
        test.submit_text_turn(prompt).await?;
    }
    builder = builder.with_config(configure.clone()).with_model("apps-b");
    let test = builder.restart_with_auto_env(&server, &test).await?;
    test.submit_text_turn("Check after restarting with the same guidance.")
        .await?;
    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some("apps-clear".to_string()),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("Clear the guidance.").await?;
    test.submit_text_turn("Continue with cleared guidance.")
        .await?;
    builder = builder.with_config(configure).with_model("apps-clear");
    let resumed = builder.restart_with_auto_env(&server, &test).await?;
    resumed
        .submit_text_turn("Check after restarting with cleared guidance.")
        .await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 8);
    assert_eq!(
        requests
            .iter()
            .map(|request| request.body_json()["model"].clone())
            .collect::<Vec<_>>(),
        vec![
            json!("apps-bundled"),
            json!("apps-no-guidance"),
            json!("apps-a"),
            json!("apps-b"),
            json!("apps-b"),
            json!("apps-clear"),
            json!("apps-clear"),
            json!("apps-clear"),
        ]
    );
    let guidance = requests
        .iter()
        .map(|request| {
            request
                .message_input_texts("developer")
                .into_iter()
                .filter(|text| {
                    text.starts_with("<apps_instructions>")
                        || text.starts_with("<catalog_apps_instructions>")
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        guidance.iter().map(Vec::len).collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 4, 5, 5, 5]
    );
    assert!(guidance[0][0].starts_with("<apps_instructions>"));
    assert!(guidance[2][2].contains("  Calendar guidance A: {{literal}}.\n"));
    assert!(guidance[3][3].contains("Calendar guidance B."));
    assert!(guidance[3][3].contains("replace all previously provided Apps usage instructions"));
    assert!(
        guidance[1][1].contains("The previously provided Apps usage instructions no longer apply.")
    );
    assert_eq!(guidance[1][1], guidance[5][4]);
    for pair in guidance.windows(2) {
        assert!(
            pair[1].starts_with(&pair[0]),
            "history must stay append-only"
        );
    }
    assert_eq!(
        guidance[4], guidance[3],
        "custom resume must not repeat guidance"
    );
    assert_eq!(
        guidance[6], guidance[5],
        "clearing twice must not repeat a notice"
    );
    assert_eq!(
        guidance[7], guidance[6],
        "cleared resume must not repeat a notice"
    );
    insta::assert_snapshot!(
        "catalog_apps_instructions_follow_selected_model",
        context_snapshot::format_request_history_snapshot(
            "Apps wording follows the selected model, retires bundled guidance when an unavailable model selects an override, preserves literal bytes, and survives both custom and cleared restarts without duplication.",
            &requests,
            &ContextSnapshotOptions::default().include_request_settings(),
        )
    );
    Ok(())
}
