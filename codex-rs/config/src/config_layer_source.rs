use codex_app_server_protocol::ConfigLayerSource;

pub fn format_config_layer_source(source: &ConfigLayerSource, config_toml_file: &str) -> String {
    match source {
        ConfigLayerSource::ProductDefaults { id, name } => {
            format!("product-defaults ({name}, {id})")
        }
        ConfigLayerSource::Mdm { domain, key } => {
            format!("MDM ({domain}:{key})")
        }
        ConfigLayerSource::System { file } => {
            format!("system ({})", file.as_path().display())
        }
        ConfigLayerSource::EnterpriseManaged { id, name } => {
            format!("enterprise-managed ({name}, {id})")
        }
        ConfigLayerSource::User { file, .. } => {
            format!("user ({})", file.as_path().display())
        }
        ConfigLayerSource::Project { dot_codex_folder } => {
            format!(
                "project ({}/{config_toml_file})",
                dot_codex_folder.as_path().display()
            )
        }
        ConfigLayerSource::SessionFlags => "session-flags".to_string(),
        ConfigLayerSource::LegacyManagedConfigTomlFromFile { file } => {
            format!("legacy managed_config.toml ({})", file.as_path().display())
        }
        ConfigLayerSource::LegacyManagedConfigTomlFromMdm => {
            "legacy managed_config.toml (MDM)".to_string()
        }
    }
}
