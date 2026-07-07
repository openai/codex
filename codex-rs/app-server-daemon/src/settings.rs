use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use tokio::fs;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DaemonSettings {
    pub(crate) remote_control_enabled: bool,
    pub(crate) codex_bin: Option<PathBuf>,
    #[serde(default)]
    pub(crate) analytics_default_enabled: bool,
}

impl DaemonSettings {
    pub(crate) async fn load(path: &Path) -> Result<Self> {
        let contents = match fs::read_to_string(path).await {
            Ok(contents) => contents,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("failed to read daemon settings {}", path.display()));
            }
        };

        serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse daemon settings {}", path.display()))
    }

    pub(crate) async fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.with_context(|| {
                format!(
                    "failed to create daemon settings directory {}",
                    parent.display()
                )
            })?;
        }

        let contents = serde_json::to_vec_pretty(self).context("failed to serialize settings")?;
        fs::write(path, contents)
            .await
            .with_context(|| format!("failed to write daemon settings {}", path.display()))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use pretty_assertions::assert_eq;

    use super::DaemonSettings;

    #[test]
    fn daemon_settings_use_camel_case_json() {
        assert_eq!(
            serde_json::to_string(&DaemonSettings {
                remote_control_enabled: true,
                codex_bin: Some("/Applications/Codex.app/Contents/Resources/codex".into()),
                analytics_default_enabled: true,
            })
            .expect("serialize"),
            r#"{"remoteControlEnabled":true,"codexBin":"/Applications/Codex.app/Contents/Resources/codex","analyticsDefaultEnabled":true}"#
        );
    }

    #[test]
    fn daemon_settings_load_without_launch_overrides() {
        assert_eq!(
            serde_json::from_str::<DaemonSettings>(r#"{"remoteControlEnabled":true}"#)
                .expect("deserialize"),
            DaemonSettings {
                remote_control_enabled: true,
                codex_bin: None,
                analytics_default_enabled: false,
            }
        );
    }
}
