//! Strict Code Mode Only: deferral, mode overrides, and hidden/denied tools.
use super::*;
use crate::tools::registry::ToolRegistry;
use codex_extension_api::ToolPolicy;
use pretty_assertions::assert_eq;

#[tokio::test]
#[tracing_test::traced_test]
async fn strict_defers_third_party_tools_despite_caller_settings_and_logs_overrides() {
    for supports_search_tool in [false, true] {
        let plan = probe_with(
            |turn| {
                set_features(
                    turn,
                    &[
                        Feature::CodeMode,
                        Feature::CodeModeOnly,
                        Feature::CodeModeOnlyStrictThirdPartyTools,
                    ],
                );
                update_turn_settings_for_test(turn, |settings| {
                    Arc::make_mut(&mut settings.model_info).supports_search_tool =
                        supports_search_tool;
                });
                update_config(turn, |config| {
                    config.update_plan_enabled = true;
                    config.code_mode.direct_only_tool_namespaces =
                        vec!["direct_client".to_string()];
                    config.code_mode.excluded_tool_namespaces = vec!["excluded_client".to_string()];
                });
            },
            ToolPlanInputs {
                dynamic_tools: vec![
                    dynamic_tool(Some("client"), "eager", /*defer_loading*/ false),
                    dynamic_tool(Some("client"), "deferred", /*defer_loading*/ true),
                    dynamic_tool(Some("direct_client"), "eager", /*defer_loading*/ false),
                    dynamic_tool(
                        Some("excluded_client"),
                        "eager",
                        /*defer_loading*/ false,
                    ),
                    // Legal legacy input: app-server forbids deferLoading=true on a plain tool.
                    dynamic_tool(
                        /*namespace*/ None,
                        "client_echo",
                        /*defer_loading*/ false,
                    ),
                    // A collision must not make the built-in tool third-party.
                    dynamic_tool(
                        /*namespace*/ None,
                        "update_plan",
                        /*defer_loading*/ false,
                    ),
                ],
                ..ToolPlanInputs::default()
            },
        )
        .await;
        plan.assert_visible_contains(&["exec", "wait"]);
        plan.assert_visible_lacks(&[
            "tool_search",
            "client",
            "direct_client",
            "excluded_client",
            "client_echo",
        ]);
        assert_eq!(plan.exposure("update_plan"), ToolExposure::Direct);
        let ToolSpec::Freeform(exec) = plan.visible_spec("exec") else {
            panic!("expected exec");
        };
        for name in [
            ToolName::namespaced("client", "eager"),
            ToolName::namespaced("client", "deferred"),
            ToolName::namespaced("direct_client", "eager"),
            ToolName::namespaced("excluded_client", "eager"),
            ToolName::plain("client_echo"),
        ] {
            assert_eq!(plan.exposure(&name.to_string()), ToolExposure::Deferred);
            assert!(
                plan.code_mode_tool_names
                    .values()
                    .any(|nested| nested == &name),
                "{name} must be callable via exec"
            );
            assert!(
                !exec.description.contains(&format!("{}(args:", name.name)),
                "{name} schema must not be eager"
            );
        }
        assert!(
            !exec.description.contains("Deferring third-party tool"),
            "warnings must not enter the model prompt"
        );
    }
    assert!(logs_contain(
        "Deferring third-party tool because code_mode_only_strict_3p_tools is enabled"
    ));
    assert!(logs_contain("client_echo"));
}

#[tokio::test]
async fn strict_flag_does_nothing_outside_effective_code_mode_only() {
    let (_, mut turn) = make_session_and_context().await;
    set_features(&mut turn, &[Feature::CodeMode, Feature::CodeModeOnly]);
    turn.code_mode_available = true;
    update_config(&mut turn, |config| {
        config.code_mode.direct_only_tool_namespaces = vec!["client".to_string()];
        config.code_mode.excluded_tool_namespaces = vec!["client".to_string()];
    });
    for mode in [ToolMode::Direct, ToolMode::CodeMode] {
        update_turn_settings_for_test(&mut turn, |settings| {
            let model = Arc::make_mut(&mut settings.model_info);
            model.tool_mode = Some(mode);
            model.supports_search_tool = true;
        });
        let inputs = || ToolPlanInputs {
            dynamic_tools: vec![
                dynamic_tool(Some("client"), "eager", /*defer_loading*/ false),
                dynamic_tool(Some("client"), "deferred", /*defer_loading*/ true),
                dynamic_tool(
                    /*namespace*/ None,
                    "client_echo",
                    /*defer_loading*/ false,
                ),
            ],
            tool_runtimes: vec![mcp_runtime(
                "sample",
                "mcp__sample",
                "lookup",
                ToolExposure::Direct,
            )],
            ..ToolPlanInputs::default()
        };
        set_feature(
            &mut turn,
            Feature::CodeModeOnlyStrictThirdPartyTools,
            /*enabled*/ false,
        );
        let off = ToolPlanProbe::from_router(plan_with_model(&turn, turn.model_info(), inputs()));
        assert_eq!(off.tool_mode, mode);
        set_feature(
            &mut turn,
            Feature::CodeModeOnlyStrictThirdPartyTools,
            /*enabled*/ true,
        );
        let on = ToolPlanProbe::from_router(plan_with_model(&turn, turn.model_info(), inputs()));
        assert_eq!(on, off, "strict flag changed effective {mode:?}");
    }
}

#[tokio::test]
async fn strict_finalization_cannot_resurrect_hidden_or_policy_denied_tools() {
    let (_, mut turn) = make_session_and_context().await;
    set_features(
        &mut turn,
        &[
            Feature::CodeMode,
            Feature::CodeModeOnly,
            Feature::CodeModeOnlyStrictThirdPartyTools,
        ],
    );
    update_config(&mut turn, |config| {
        config.code_mode.direct_only_tool_namespaces = vec!["mcp__allowed".to_string()];
    });
    let allowed_mcp = ToolName::namespaced("mcp__allowed", "lookup");
    let hidden_mcp = ToolName::namespaced("mcp__allowed", "over_budget");
    let mut registry = ToolRegistry::with_tool_policy(Arc::new(ToolPolicy {
        allowed_tools: Some(vec![
            ToolName::plain("exec"),
            ToolName::plain("wait"),
            allowed_mcp.clone(),
            hidden_mcp.clone(),
            ToolName::plain("allowed_dynamic"),
        ]),
        ..Default::default()
    }));
    let hosted = append_source_tools(
        &turn,
        turn.model_info(),
        &mut registry,
        vec![
            mcp_runtime("allowed", "mcp__allowed", "lookup", ToolExposure::Direct),
            // MCP registration keeps budget-overflow tools in the registry as Hidden.
            // This is different from a caller setting omit_tools_from.
            mcp_runtime(
                "allowed",
                "mcp__allowed",
                "over_budget",
                ToolExposure::Hidden,
            ),
            mcp_runtime("denied", "mcp__denied", "lookup", ToolExposure::Direct),
        ],
        std::iter::empty::<Arc<dyn for<'call> ToolExecutor<ExtensionToolCall<'call>>>>(),
        &[
            dynamic_tool(
                /*namespace*/ None,
                "allowed_dynamic",
                /*defer_loading*/ false,
            ),
            dynamic_tool(
                /*namespace*/ None,
                "denied_dynamic",
                /*defer_loading*/ false,
            ),
        ],
    );
    let plan = ToolPlanProbe::from_router(ToolRouter::from_registry(
        &turn,
        turn.model_info(),
        registry,
        hosted,
        &Default::default(),
    ));
    plan.assert_registered_lacks(&[
        &ToolName::namespaced("mcp__denied", "lookup").to_string(),
        "denied_dynamic",
    ]);
    assert_eq!(
        plan.exposure(&allowed_mcp.to_string()),
        ToolExposure::Deferred
    );
    assert_eq!(plan.exposure(&hidden_mcp.to_string()), ToolExposure::Hidden);
    assert_eq!(plan.exposure("allowed_dynamic"), ToolExposure::Deferred);
    assert_eq!(
        plan.code_mode_tool_names
            .values()
            .cloned()
            .collect::<std::collections::HashSet<_>>(),
        [allowed_mcp, ToolName::plain("allowed_dynamic")]
            .into_iter()
            .collect(),
    );
}
