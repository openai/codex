//! Code Mode finalization preserves tool policy and hidden-tool eligibility.
use super::*;
use crate::tools::registry::ToolRegistry;
use codex_extension_api::ToolPolicy;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn code_mode_finalization_preserves_hidden_and_policy_denied_tools() {
    let (_, mut turn) = make_session_and_context().await;
    set_features(&mut turn, &[Feature::CodeMode, Feature::CodeModeOnly]);
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
        ToolExposure::DirectModelOnly
    );
    assert_eq!(plan.exposure(&hidden_mcp.to_string()), ToolExposure::Hidden);
    assert_eq!(plan.exposure("allowed_dynamic"), ToolExposure::Direct);
    assert_eq!(
        plan.code_mode_tool_names
            .values()
            .cloned()
            .collect::<std::collections::HashSet<_>>(),
        [ToolName::plain("allowed_dynamic")].into_iter().collect(),
    );
}
