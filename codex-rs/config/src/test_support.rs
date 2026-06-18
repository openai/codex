//! Test-only helpers exposed for cross-crate integration tests.
//!
//! Production code should not depend on this module.

use crate::CloudConfigBundle;
use crate::CloudConfigBundleLoader;
use crate::CloudConfigFragment;
use crate::CloudManagedLayer;
use crate::CloudRequirementsFragment;

#[derive(Debug, Clone, Default)]
pub struct CloudConfigBundleFixture {
    bundle: CloudConfigBundle,
}

impl CloudConfigBundleFixture {
    pub fn enterprise_requirement(contents: impl Into<String>) -> Self {
        Self::default().add_enterprise_requirement(contents)
    }

    pub fn enterprise_config(contents: impl Into<String>) -> Self {
        Self::default().add_enterprise_config(contents)
    }

    pub fn loader_with_enterprise_requirement(
        contents: impl Into<String>,
    ) -> CloudConfigBundleLoader {
        Self::enterprise_requirement(contents).into_loader()
    }

    pub fn loader_with_enterprise_config(contents: impl Into<String>) -> CloudConfigBundleLoader {
        Self::enterprise_config(contents).into_loader()
    }

    pub fn add_enterprise_requirement(mut self, contents: impl Into<String>) -> Self {
        let index = self
            .bundle
            .requirements_toml
            .managed_layers
            .system_overlay
            .len()
            + 1;
        self.bundle
            .requirements_toml
            .managed_layers
            .system_overlay
            .push(CloudRequirementsFragment {
                id: format!("req_{index}"),
                name: if index == 1 {
                    "Base requirements".to_string()
                } else {
                    format!("Requirements {index}")
                },
                contents: contents.into(),
            });
        self
    }

    pub fn add_enterprise_config(mut self, contents: impl Into<String>) -> Self {
        let index = self.bundle.config_toml.managed_layers.system_overlay.len() + 1;
        self.bundle
            .config_toml
            .managed_layers
            .system_overlay
            .push(CloudConfigFragment {
                id: format!("cfg_{index}"),
                name: if index == 1 {
                    "Base config".to_string()
                } else {
                    format!("Config {index}")
                },
                contents: contents.into(),
            });
        self
    }

    pub fn add_managed_requirement(
        mut self,
        layer: CloudManagedLayer,
        contents: impl Into<String>,
    ) -> Self {
        let fragments = match layer {
            CloudManagedLayer::Baseline => {
                &mut self.bundle.requirements_toml.managed_layers.baseline
            }
            CloudManagedLayer::SystemOverlay => {
                &mut self.bundle.requirements_toml.managed_layers.system_overlay
            }
        }
        .get_or_insert_default();
        let index = fragments.len() + 1;
        fragments.push(CloudRequirementsFragment {
            id: format!("managed_req_{index}"),
            name: format!("{layer} requirements {index}"),
            contents: contents.into(),
        });
        self
    }

    pub fn add_managed_config(
        mut self,
        layer: CloudManagedLayer,
        contents: impl Into<String>,
    ) -> Self {
        let fragments = match layer {
            CloudManagedLayer::Baseline => &mut self.bundle.config_toml.managed_layers.baseline,
            CloudManagedLayer::SystemOverlay => {
                &mut self.bundle.config_toml.managed_layers.system_overlay
            }
        }
        .get_or_insert_default();
        let index = fragments.len() + 1;
        fragments.push(CloudConfigFragment {
            id: format!("managed_cfg_{index}"),
            name: format!("{layer} config {index}"),
            contents: contents.into(),
        });
        self
    }

    pub fn into_bundle(self) -> CloudConfigBundle {
        self.bundle
    }

    pub fn into_loader(self) -> CloudConfigBundleLoader {
        let bundle = self.into_bundle();
        CloudConfigBundleLoader::new(async move { Ok(Some(bundle)) })
    }
}

#[cfg(test)]
#[path = "test_support_tests.rs"]
mod tests;
