use super::*;
use pretty_assertions::assert_eq;
use std::io;
use tempfile::TempDir;

const EXTERNAL_AGENT_PROJECT_CONFIG_FILE: &str = ".claude.json";
const EXTERNAL_AGENT_PLUGIN_MANIFEST_DIR: &str = ".claude-plugin";
const SOURCE_EXTERNAL_AGENT_NAME: &str = "claude";
const SOURCE_EXTERNAL_AGENT_DISPLAY_NAME: &str = "Claude";
const SOURCE_EXTERNAL_AGENT_PRODUCT_NAME: &str = "Claude Code";
const SOURCE_EXTERNAL_AGENT_UPPER_NAME: &str = "CLAUDE";
const SOURCE_EXTERNAL_AGENT_UPPER_PRODUCT_NAME: &str = "CLAUDE-CODE";

fn fixture_paths() -> (TempDir, PathBuf, PathBuf) {
    let root = TempDir::new().expect("create tempdir");
    let external_agent_home = root.path().join(EXTERNAL_AGENT_DIR);
    let codex_home = root.path().join(".codex");
    (root, external_agent_home, codex_home)
}

fn service_for_paths(
    external_agent_home: PathBuf,
    codex_home: PathBuf,
) -> ExternalAgentConfigService {
    ExternalAgentConfigService::new_for_test(codex_home, external_agent_home)
}

fn github_plugin_details() -> MigrationDetails {
    MigrationDetails {
        plugins: vec![PluginsMigration {
            marketplace_name: "acme-tools".to_string(),
            plugin_names: vec!["formatter".to_string()],
        }],
        ..Default::default()
    }
}

fn assert_single_plugin_raw_error(
    raw_errors: &[ExternalAgentConfigImportRawError],
    failure_stage: &str,
    source: &str,
    error_type: Option<&str>,
) {
    assert_eq!(raw_errors.len(), 1);
    let raw_error = &raw_errors[0];
    assert_eq!(
        raw_error.item_type,
        ExternalAgentConfigMigrationItemType::Plugins
    );
    assert_eq!(raw_error.failure_stage, failure_stage);
    assert_eq!(raw_error.error_type.as_deref(), error_type);
    assert_eq!(raw_error.sub_error_type, None);
    assert_eq!(raw_error.cwd, None);
    assert_eq!(raw_error.source.as_deref(), Some(source));
    assert!(!raw_error.message.is_empty());
}

fn import_success(
    item_type: ExternalAgentConfigMigrationItemType,
    cwd: Option<PathBuf>,
    source: impl Into<String>,
    target: impl Into<String>,
) -> ExternalAgentConfigImportSuccess {
    ExternalAgentConfigImportSuccess {
        item_type,
        cwd,
        source: Some(source.into()),
        target: Some(target.into()),
    }
}

#[path = "service_tests/general.rs"]
mod general;

#[path = "service_tests/memory.rs"]
mod memory;

#[path = "service_tests/plugins.rs"]
mod plugins;
