use super::*;
use crate::ConfigLayerSource;
use crate::ConfigRequirementsToml;
use crate::compose_requirements;
use codex_protocol::protocol::AskForApproval;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tempfile::tempdir;

#[tokio::test]
async fn shared_future_runs_once() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = Arc::clone(&counter);
    let loader = CloudConfigBundleLoader::new(async move {
        counter_clone.fetch_add(1, Ordering::SeqCst);
        Ok(Some(CloudConfigBundle::default()))
    });

    let (first, second) = tokio::join!(loader.get(), loader.get());
    assert_eq!(first, second);
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn bundle_layers_preserve_enterprise_managed_bucket_order() {
    let tempdir = tempdir().expect("tempdir");
    let base_dir = AbsolutePathBuf::from_absolute_path(tempdir.path()).expect("absolute path");
    let layers = CloudConfigBundleLayers::from_bundle(
        CloudConfigBundle {
            config_toml: CloudConfigTomlBundle {
                product_defaults: vec![
                    CloudConfigFragment {
                        id: "default_high".to_string(),
                        name: "High defaults".to_string(),
                        contents: "model = \"default-high\"".to_string(),
                    },
                    CloudConfigFragment {
                        id: "default_low".to_string(),
                        name: "Low defaults".to_string(),
                        contents: "model = \"default-low\"".to_string(),
                    },
                ],
                enterprise_managed: vec![
                    CloudConfigFragment {
                        id: "cfg_high".to_string(),
                        name: "High config".to_string(),
                        contents: "model = \"high\"".to_string(),
                    },
                    CloudConfigFragment {
                        id: "cfg_low".to_string(),
                        name: "Low config".to_string(),
                        contents: "model = \"low\"".to_string(),
                    },
                ],
            },
            requirements_toml: CloudRequirementsTomlBundle {
                enterprise_managed: vec![
                    CloudRequirementsFragment {
                        id: "req_high".to_string(),
                        name: "High requirements".to_string(),
                        contents: "allowed_approval_policies = [\"on-request\"]".to_string(),
                    },
                    CloudRequirementsFragment {
                        id: "req_low".to_string(),
                        name: "Low requirements".to_string(),
                        contents: "allowed_approval_policies = [\"never\"]".to_string(),
                    },
                ],
            },
        },
        &base_dir,
    )
    .expect("bundle should be converted into layers");

    assert_eq!(
        layers
            .product_defaults_config
            .iter()
            .map(|layer| layer.name.clone())
            .collect::<Vec<_>>(),
        vec![
            ConfigLayerSource::ProductDefaults {
                id: "default_low".to_string(),
                name: "Low defaults".to_string(),
            },
            ConfigLayerSource::ProductDefaults {
                id: "default_high".to_string(),
                name: "High defaults".to_string(),
            },
        ]
    );
    assert_eq!(
        layers
            .enterprise_managed_config
            .iter()
            .map(|layer| layer.name.clone())
            .collect::<Vec<_>>(),
        vec![
            ConfigLayerSource::EnterpriseManaged {
                id: "cfg_low".to_string(),
                name: "Low config".to_string(),
            },
            ConfigLayerSource::EnterpriseManaged {
                id: "cfg_high".to_string(),
                name: "High config".to_string(),
            },
        ]
    );
    assert_eq!(
        compose_requirements(layers.enterprise_managed_requirements)
            .expect("requirements should compose")
            .expect("requirements should be present")
            .into_toml(),
        ConfigRequirementsToml {
            allowed_approval_policies: Some(vec![AskForApproval::OnRequest]),
            ..Default::default()
        }
    );
}

#[test]
fn bundle_layers_can_strict_validate_enterprise_managed_config() {
    let tempdir = tempdir().expect("tempdir");
    let base_dir = AbsolutePathBuf::from_absolute_path(tempdir.path()).expect("absolute path");
    let err = CloudConfigBundleLayers::from_bundle_strict_config(
        CloudConfigBundle {
            config_toml: CloudConfigTomlBundle {
                product_defaults: Vec::new(),
                enterprise_managed: vec![CloudConfigFragment {
                    id: "cfg".to_string(),
                    name: "Cloud config".to_string(),
                    contents: "unknown_key = true".to_string(),
                }],
            },
            requirements_toml: CloudRequirementsTomlBundle {
                enterprise_managed: Vec::new(),
            },
        },
        &base_dir,
    )
    .expect_err("strict config should reject unknown fields");

    assert_eq!(
        err,
        CloudConfigLayerError::Invalid {
            fragment: crate::CloudConfigFragmentSource {
                id: "cfg".to_string(),
                name: "Cloud config".to_string(),
            },
            message: "unknown configuration field `unknown_key`".to_string(),
        }
    );
}
