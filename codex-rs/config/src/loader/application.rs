//! Reads application requirements independently of user, project, and cloud configuration.
//! Captured local layers retain their positions around cloud layers during composition.

use crate::ApplicationRequirementsToml;
use crate::CloudRequirementsTomlBundle;
use crate::LoaderOverrides;
use crate::RequirementsLayerEntry;
use crate::compose_requirements;
use codex_file_system::ExecutorFileSystem;
use std::io;
use toml::Value;

/// Local application sources captured without waiting for a cloud request.
#[derive(Clone, Debug, Default)]
pub struct LocalApplicationRequirements {
    system: Option<RequirementsLayerEntry>,
    mdm: Option<RequirementsLayerEntry>,
    ignore_managed: bool,
}

impl LocalApplicationRequirements {
    /// Composes system, cloud, and MDM application policy in the normal requirements order.
    pub fn compose(
        &self,
        cloud: CloudRequirementsTomlBundle,
    ) -> io::Result<Option<ApplicationRequirementsToml>> {
        if self.ignore_managed {
            return Ok(None);
        }
        let mut layers = self.system.clone().into_iter().collect::<Vec<_>>();
        for layer in cloud.into_layers() {
            layers.extend(application_layer(layer)?);
        }
        layers.extend(self.mdm.clone());
        Ok(compose_requirements(layers)?
            .and_then(|requirements| requirements.into_toml().application))
    }
}

/// Reads only the local sources that can set application requirements.
/// macOS policy reads require a successful preference refresh before using cached values.
pub async fn load_local_application_requirements(
    fs: &dyn ExecutorFileSystem,
    overrides: &LoaderOverrides,
) -> io::Result<LocalApplicationRequirements> {
    if overrides.ignore_managed_requirements {
        return Ok(LocalApplicationRequirements {
            ignore_managed: true,
            ..Default::default()
        });
    }
    let system_path = super::system_requirements_toml_file_with_overrides(overrides)?;
    let system = super::load_requirements_toml(fs, &system_path)
        .await?
        .map(application_layer)
        .transpose()?
        .flatten();
    #[cfg(target_os = "macos")]
    let mdm = {
        if overrides.macos_managed_config_requirements_base64.is_none() {
            tokio::task::spawn_blocking(super::macos::synchronize_managed_preferences)
                .await
                .map_err(io::Error::other)??;
        }
        super::macos::load_managed_admin_requirements_layer(
            overrides
                .macos_managed_config_requirements_base64
                .as_deref(),
        )
        .await?
        .map(application_layer)
        .transpose()?
        .flatten()
    };
    #[cfg(not(target_os = "macos"))]
    let mdm = None;
    let local = LocalApplicationRequirements {
        system,
        mdm,
        ignore_managed: false,
    };
    // Validate local requirements before cloud loading.
    local.compose(CloudRequirementsTomlBundle::default())?;
    Ok(local)
}

fn application_layer(layer: RequirementsLayerEntry) -> io::Result<Option<RequirementsLayerEntry>> {
    let (source, mut value, _base_dir) = layer.into_raw_parts()?;
    let Some(application) = value
        .as_table_mut()
        .and_then(|table| table.remove("application"))
    else {
        return Ok(None);
    };
    Ok(Some(RequirementsLayerEntry::from_toml_value(
        source,
        Value::Table(toml::map::Map::from_iter([(
            "application".into(),
            application,
        )])),
    )))
}
