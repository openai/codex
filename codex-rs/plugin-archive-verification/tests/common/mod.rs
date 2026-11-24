//! Loads the signed archive and its catalog and signing-key responses from the same test vector.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde_json::Value;

pub const ALLOW_HTTP_BUNDLE_DOWNLOADS: &str =
    "CODEX_TEST_ALLOW_HTTP_REMOTE_PLUGIN_BUNDLE_DOWNLOADS";

pub struct PluginArchiveSigningFixture {
    pub archive: Vec<u8>,
    pub signing: Value,
    pub signing_keys_response: Value,
}

impl PluginArchiveSigningFixture {
    pub fn load() -> anyhow::Result<Self> {
        #[derive(Deserialize)]
        struct Vector {
            archive_base64: String,
            signing: Value,
            signing_keys_response: Value,
        }
        let vector: Vector = serde_json::from_str(include_str!(
            "../../testdata/plugin_archive_signing_v1.json"
        ))?;
        Ok(Self {
            archive: STANDARD.decode(vector.archive_base64)?,
            signing: vector.signing,
            signing_keys_response: vector.signing_keys_response,
        })
    }
}
