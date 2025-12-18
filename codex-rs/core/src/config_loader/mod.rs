mod fingerprint;
mod layer_io;
#[cfg(target_os = "macos")]
mod macos;
mod merge;
mod overrides;
mod state;

#[cfg(test)]
mod tests;

use crate::config::CONFIG_TOML_FILE;
use codex_app_server_protocol::ConfigLayerSource;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::io;
use std::path::Path;
use toml::Value as TomlValue;

pub use merge::merge_toml_values;
pub use state::ConfigLayerEntry;
pub use state::ConfigLayerStack;
pub use state::LoaderOverrides;

const MDM_PREFERENCES_DOMAIN: &str = "com.openai.codex";
const MDM_PREFERENCES_KEY: &str = "config_toml_base64";

/// Configuration layering pipeline (top overrides bottom):
///
///        +-------------------------+
///        | Managed preferences (*) |
///        +-------------------------+
///                    ^
///                    |
///        +-------------------------+
///        |  managed_config.toml   |
///        +-------------------------+
///                    ^
///                    |
///        +-------------------------+
///        |    config.toml (base)   |
///        +-------------------------+
///
/// (*) Only available on macOS via managed device profiles.
pub async fn load_config_layers_state(
    codex_home: &Path,
    cli_overrides: &[(String, TomlValue)],
    overrides: LoaderOverrides,
) -> io::Result<ConfigLayerStack> {
    let managed_config_path = overrides
        .managed_config_path
        .clone()
        .unwrap_or_else(|| layer_io::managed_config_default_path(codex_home));

    let layers = layer_io::load_config_layers_internal(codex_home, overrides).await?;
    let cli_overrides_layer = overrides::build_cli_overrides_layer(cli_overrides);
    let user_file = AbsolutePathBuf::from_absolute_path(codex_home.join(CONFIG_TOML_FILE))?;

    let system = match layers.managed_config {
        Some(cfg) => {
            let system_file = AbsolutePathBuf::from_absolute_path(managed_config_path.clone())?;
            Some(ConfigLayerEntry::new(
                ConfigLayerSource::System { file: system_file },
                cfg,
            ))
        }
        None => None,
    };

    Ok(ConfigLayerStack {
        user: ConfigLayerEntry::new(ConfigLayerSource::User { file: user_file }, layers.base),
        session_flags: ConfigLayerEntry::new(ConfigLayerSource::SessionFlags, cli_overrides_layer),
        system,
        mdm: layers.managed_preferences.map(|cfg| {
            ConfigLayerEntry::new(
                ConfigLayerSource::Mdm {
                    domain: MDM_PREFERENCES_DOMAIN.to_string(),
                    key: MDM_PREFERENCES_KEY.to_string(),
                },
                cfg,
            )
        }),
    })
}
