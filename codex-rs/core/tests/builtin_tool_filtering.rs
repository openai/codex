use codex_core::config::{Config, ConfigOverrides, ConfigToml};
use codex_core::model_family::find_family_for_model;
use codex_core::tools::spec::{ToolsConfig, ToolsConfigParams, build_specs};
use std::path::PathBuf;
use tempfile::TempDir;

fn create_test_config_with_filtering(
    exclude: Vec<String>,
    include: Option<Vec<String>>,
) -> std::io::Result<Config> {
    let codex_home = TempDir::new()?.path().to_path_buf();
    let mut cfg = ConfigToml::default();
    cfg.exclude_builtin_tools = exclude;
    cfg.include_builtin_tools = include;

    Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides {
            cwd: Some(TempDir::new()?.path().to_path_buf()),
            ..Default::default()
        },
        codex_home,
    )
}

fn tool_names_from_config(config: &Config) -> Vec<String> {
    let model_family =
        find_family_for_model("gpt-5-codex").expect("gpt-5-codex should be a valid model family");

    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_family: &model_family,
        include_plan_tool: config.include_plan_tool,
        include_apply_patch_tool: config.include_apply_patch_tool,
        include_web_search_request: config.tools_web_search_request,
        use_streamable_shell_tool: config.use_experimental_streamable_shell_tool,
        include_view_image_tool: config.include_view_image_tool,
        experimental_unified_exec_tool: config.use_experimental_unified_exec_tool,
        exclude_builtin_tools: &config.exclude_builtin_tools,
        include_builtin_tools: &config.include_builtin_tools,
    });

    let (tools, _) = build_specs(&tools_config, None).build();

    tools
        .iter()
        .map(|t| match &t.spec {
            codex_core::client_common::tools::ToolSpec::Function(f) => f.name.clone(),
            codex_core::client_common::tools::ToolSpec::LocalShell {} => "local_shell".to_string(),
            codex_core::client_common::tools::ToolSpec::WebSearch {} => "web_search".to_string(),
            codex_core::client_common::tools::ToolSpec::Freeform(f) => f.name.clone(),
        })
        .collect()
}

#[test]
fn test_exclude_specific_tools() {
    let config = create_test_config_with_filtering(
        vec!["local_shell".to_string(), "read_file".to_string()],
        None,
    )
    .expect("Config creation should succeed");

    let tool_names = tool_names_from_config(&config);

    // local_shell and read_file should be excluded
    assert!(!tool_names.contains(&"local_shell".to_string()));
    assert!(!tool_names.contains(&"shell".to_string()));
    assert!(!tool_names.contains(&"read_file".to_string()));

    // Other tools like apply_patch should still be included if enabled
    // (Note: apply_patch requires explicit config, but view_image is default)
}

#[test]
fn test_include_only_specific_tools() {
    let config = create_test_config_with_filtering(vec![], Some(vec!["apply_patch".to_string()]))
        .expect("Config creation should succeed");

    let tool_names = tool_names_from_config(&config);

    // Only apply_patch should be included (if enabled)
    // All other builtin tools should be excluded
    assert!(!tool_names.contains(&"local_shell".to_string()));
    assert!(!tool_names.contains(&"shell".to_string()));
    assert!(!tool_names.contains(&"view_image".to_string()));
    assert!(!tool_names.contains(&"web_search".to_string()));
}

#[test]
fn test_include_empty_disables_all_builtin_tools() {
    let config = create_test_config_with_filtering(vec![], Some(vec![]))
        .expect("Config creation should succeed");

    let tool_names = tool_names_from_config(&config);

    // All builtin tools should be disabled
    assert!(!tool_names.contains(&"local_shell".to_string()));
    assert!(!tool_names.contains(&"shell".to_string()));
    assert!(!tool_names.contains(&"apply_patch".to_string()));
    assert!(!tool_names.contains(&"view_image".to_string()));
    assert!(!tool_names.contains(&"web_search".to_string()));
    assert!(!tool_names.contains(&"read_file".to_string()));
    assert!(!tool_names.contains(&"plan".to_string()));
}

#[test]
fn test_exclude_and_include_mutually_exclusive() {
    let result = create_test_config_with_filtering(
        vec!["local_shell".to_string()],
        Some(vec!["apply_patch".to_string()]),
    );

    // Should fail with error about mutual exclusion
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Cannot use both"));
}

#[test]
fn test_default_behavior_all_tools_enabled() {
    let config =
        create_test_config_with_filtering(vec![], None).expect("Config creation should succeed");

    let tool_names = tool_names_from_config(&config);

    // Default shell tools should be included
    // (Note: The actual tool name depends on shell_type and experimental flags)
    // With default config, we should have at least shell tools
    assert!(!tool_names.is_empty());
}

#[test]
fn test_exclude_web_search() {
    let mut cfg = ConfigToml::default();
    cfg.exclude_builtin_tools = vec!["web_search_request".to_string()];
    cfg.tools = Some(codex_core::config::ToolsToml {
        web_search: Some(true),
        view_image: None,
    });

    let codex_home = TempDir::new().unwrap().path().to_path_buf();
    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides {
            cwd: Some(TempDir::new().unwrap().path().to_path_buf()),
            ..Default::default()
        },
        codex_home,
    )
    .expect("Config creation should succeed");

    let tool_names = tool_names_from_config(&config);

    // web_search should be excluded even though enabled in tools config
    assert!(!tool_names.contains(&"web_search".to_string()));
}

#[test]
fn test_exclude_view_image() {
    let mut cfg = ConfigToml::default();
    cfg.exclude_builtin_tools = vec!["view_image".to_string()];

    let codex_home = TempDir::new().unwrap().path().to_path_buf();
    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides {
            cwd: Some(TempDir::new().unwrap().path().to_path_buf()),
            include_view_image_tool: Some(true),
            ..Default::default()
        },
        codex_home,
    )
    .expect("Config creation should succeed");

    let tool_names = tool_names_from_config(&config);

    // view_image should be excluded even though enabled
    assert!(!tool_names.contains(&"view_image".to_string()));
}

#[test]
fn test_exclude_plan_tool() {
    let mut cfg = ConfigToml::default();
    cfg.exclude_builtin_tools = vec!["plan".to_string()];

    let codex_home = TempDir::new().unwrap().path().to_path_buf();
    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides {
            cwd: Some(TempDir::new().unwrap().path().to_path_buf()),
            include_plan_tool: Some(true),
            ..Default::default()
        },
        codex_home,
    )
    .expect("Config creation should succeed");

    let tool_names = tool_names_from_config(&config);

    // plan tool (update_plan) should be excluded even though enabled
    assert!(!tool_names.contains(&"update_plan".to_string()));
}

#[test]
fn test_include_only_local_shell() {
    let config = create_test_config_with_filtering(vec![], Some(vec!["local_shell".to_string()]))
        .expect("Config creation should succeed");

    let tool_names = tool_names_from_config(&config);

    // Only shell tools should be available
    // All other builtin tools should be excluded
    assert!(!tool_names.contains(&"view_image".to_string()));
    assert!(!tool_names.contains(&"web_search".to_string()));
    assert!(!tool_names.contains(&"apply_patch".to_string()));
}
