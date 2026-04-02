use schemars::r#gen::SchemaGenerator;
use schemars::schema::InstanceType;
use schemars::schema::ObjectValidation;
use schemars::schema::Schema;
use schemars::schema::SchemaObject;

const FEATURE_KEYS: &[&str] = &[
    "undo",
    "shell_tool",
    "unified_exec",
    "shell_zsh_fork",
    "shell_snapshot",
    "js_repl",
    "code_mode",
    "code_mode_only",
    "js_repl_tools_only",
    "web_search_request",
    "web_search_cached",
    "search_tool",
    "codex_git_commit",
    "runtime_metrics",
    "general_analytics",
    "sqlite",
    "memories",
    "child_agents_md",
    "image_detail_original",
    "apply_patch_freeform",
    "exec_permission_approvals",
    "codex_hooks",
    "request_permissions_tool",
    "use_linux_sandbox_bwrap",
    "use_legacy_landlock",
    "request_rule",
    "experimental_windows_sandbox",
    "elevated_windows_sandbox",
    "remote_models",
    "enable_request_compression",
    "multi_agent",
    "multi_agent_v2",
    "enable_fanout",
    "apps",
    "tool_search",
    "tool_suggest",
    "plugins",
    "image_generation",
    "skill_mcp_dependency_install",
    "skill_env_var_dependency_prompt",
    "steer",
    "default_mode_request_user_input",
    "guardian_approval",
    "collaboration_modes",
    "tool_call_mcp_elicitation",
    "personality",
    "fast_mode",
    "realtime_conversation",
    "tui_app_server",
    "prevent_idle_sleep",
    "responses_websockets",
    "responses_websockets_v2",
];

const LEGACY_FEATURE_KEYS: &[&str] = &[
    "connectors",
    "enable_experimental_windows_sandbox",
    "experimental_use_unified_exec_tool",
    "experimental_use_freeform_apply_patch",
    "include_apply_patch_tool",
    "request_permissions",
    "web_search",
    "collab",
    "memory_tool",
];

/// Schema for the `[features]` map with known + legacy keys only.
pub fn features_schema(schema_gen: &mut SchemaGenerator) -> Schema {
    let mut object = SchemaObject {
        instance_type: Some(InstanceType::Object.into()),
        ..Default::default()
    };

    let mut validation = ObjectValidation::default();
    for key in FEATURE_KEYS {
        validation
            .properties
            .insert((*key).to_string(), schema_gen.subschema_for::<bool>());
    }
    for key in LEGACY_FEATURE_KEYS {
        validation
            .properties
            .insert((*key).to_string(), schema_gen.subschema_for::<bool>());
    }
    validation.additional_properties = Some(Box::new(Schema::Bool(false)));
    object.object = Some(Box::new(validation));

    Schema::Object(object)
}
