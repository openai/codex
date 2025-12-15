mod fingerprint;
mod layer_io;
mod macos;
mod merge;
mod overrides;
mod state;

#[cfg(test)]
mod tests;

use crate::config::CONFIG_TOML_FILE;
use codex_app_server_protocol::ConfigLayerName;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use toml::Value as TomlValue;

pub use merge::merge_toml_values;
pub use state::ConfigLayerEntry;
pub use state::ConfigLayerStack;
pub use state::LoaderOverrides;

const SESSION_FLAGS_SOURCE: &str = "--config";
const MDM_SOURCE: &str = "com.openai.codex/config_toml_base64";

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
    let cli_overrides = overrides::build_cli_overrides_layer(cli_overrides);

    Ok(ConfigLayerStack {
        user: ConfigLayerEntry::new(
            ConfigLayerName::User,
            codex_home.join(CONFIG_TOML_FILE),
            layers.base,
        ),
        session_flags: ConfigLayerEntry::new(
            ConfigLayerName::SessionFlags,
            PathBuf::from(SESSION_FLAGS_SOURCE),
            cli_overrides,
        ),
        system: layers.managed_config.map(|cfg| {
            ConfigLayerEntry::new(ConfigLayerName::System, managed_config_path.clone(), cfg)
        }),
        mdm: layers
            .managed_preferences
            .map(|cfg| ConfigLayerEntry::new(ConfigLayerName::Mdm, PathBuf::from(MDM_SOURCE), cfg)),
    })
}
