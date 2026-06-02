use codex_app_server_protocol::ConfigLayerSource;
use codex_file_system::ExecutorFileSystem;
use codex_model_provider_info::AMAZON_BEDROCK_PROVIDER_ID;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use std::io;
use std::path::Path;
use toml::Value as TomlValue;

use crate::ConfigLayerEntry;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AmazonBedrockAuthState {
    #[serde(default = "default_activates_provider")]
    activates_provider: bool,
}

pub(crate) async fn load_provider_state_layer(
    fs: &dyn ExecutorFileSystem,
    codex_home: &Path,
) -> io::Result<Option<ConfigLayerEntry>> {
    let auth_file = amazon_bedrock_auth_file(codex_home);
    let contents = match fs.read_file_text(&auth_file, /*sandbox*/ None).await {
        Ok(contents) => contents,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(io::Error::new(
                err.kind(),
                format!(
                    "Failed to read provider state file {}: {err}",
                    auth_file.as_path().display()
                ),
            ));
        }
    };
    let state: AmazonBedrockAuthState = serde_json::from_str(&contents).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Failed to parse provider state file {}: {err}",
                auth_file.as_path().display()
            ),
        )
    })?;
    if !state.activates_provider {
        return Ok(None);
    }

    let mut root = toml::map::Map::new();
    root.insert(
        "model_provider".to_string(),
        TomlValue::String(AMAZON_BEDROCK_PROVIDER_ID.to_string()),
    );
    Ok(Some(ConfigLayerEntry::new(
        ConfigLayerSource::ProviderState {
            provider: AMAZON_BEDROCK_PROVIDER_ID.to_string(),
            file: auth_file,
        },
        TomlValue::Table(root),
    )))
}

pub(crate) fn amazon_bedrock_auth_file(codex_home: &Path) -> AbsolutePathBuf {
    AbsolutePathBuf::resolve_path_against_base(
        "model-providers/amazon-bedrock/auth.json",
        codex_home,
    )
}

fn default_activates_provider() -> bool {
    true
}
