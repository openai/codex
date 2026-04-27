use super::HostNameResolver;
use super::load_config_layers_state;
use crate::CloudRequirementsLoader;
use crate::ConfigRequirementsToml;
use crate::LoaderOverrides;
use crate::NoopThreadConfigLoader;
use crate::SandboxModeRequirement;
use codex_exec_server::LOCAL_FS;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tempfile::tempdir;
use toml::Value as TomlValue;

#[tokio::test]
async fn skips_hostname_lookup_without_remote_sandbox_config() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let codex_home = tmp.path().join("home");
    tokio::fs::create_dir_all(&codex_home).await?;
    let cwd = AbsolutePathBuf::from_absolute_path(tmp.path())?;

    let requirements: ConfigRequirementsToml = toml::from_str(
        r#"
            allowed_sandbox_modes = ["read-only"]
        "#,
    )?;
    let cloud_requirements = CloudRequirementsLoader::new(async move { Ok(Some(requirements)) });
    let layers = load_config_layers_state(
        LOCAL_FS.as_ref(),
        &codex_home,
        Some(cwd),
        &[] as &[(String, TomlValue)],
        LoaderOverrides::default(),
        cloud_requirements,
        &NoopThreadConfigLoader,
        HostNameResolver {
            resolver: Arc::new(|| panic!("hostname should not be resolved")),
        },
    )
    .await?;

    assert_eq!(
        layers.requirements_toml().allowed_sandbox_modes,
        Some(vec![SandboxModeRequirement::ReadOnly])
    );

    Ok(())
}
