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
fn bundle_with_explicitly_empty_overlay_is_not_empty() {
    let bundle = CloudConfigBundle {
        config_toml: CloudConfigTomlBundle {
            enterprise_managed: Vec::new(),
            managed_layers: CloudConfigTomlManagedLayers {
                baseline: None,
                system_overlay: Some(Vec::new()),
            },
        },
        requirements_toml: CloudRequirementsTomlBundle::default(),
    };

    assert!(!bundle.is_empty());
    assert!(bundle.has_managed_layer_buckets());
}

#[test]
fn bundle_layers_preserve_enterprise_managed_bucket_order() {
    let tempdir = tempdir().expect("tempdir");
    let base_dir = AbsolutePathBuf::from_absolute_path(tempdir.path()).expect("absolute path");
    let layers = CloudConfigBundleLayers::from_bundle(
        CloudConfigBundle {
            config_toml: CloudConfigTomlBundle {
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
                managed_layers: Default::default(),
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
                managed_layers: Default::default(),
            },
        },
        &base_dir,
    )
    .expect("bundle should be converted into layers");

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
fn bundle_layers_preserve_managed_bucket_presence_and_order() {
    let tempdir = tempdir().expect("tempdir");
    let base_dir = AbsolutePathBuf::from_absolute_path(tempdir.path()).expect("absolute path");
    let layers = CloudConfigBundleLayers::from_bundle(
        CloudConfigBundle {
            config_toml: CloudConfigTomlBundle {
                enterprise_managed: Vec::new(),
                managed_layers: CloudConfigTomlManagedLayers {
                    baseline: Some(vec![
                        CloudConfigFragment {
                            id: "baseline_high".to_string(),
                            name: "Baseline high".to_string(),
                            contents: "model = \"high\"".to_string(),
                        },
                        CloudConfigFragment {
                            id: "baseline_low".to_string(),
                            name: "Baseline low".to_string(),
                            contents: "model = \"low\"".to_string(),
                        },
                    ]),
                    system_overlay: Some(Vec::new()),
                },
            },
            requirements_toml: CloudRequirementsTomlBundle {
                enterprise_managed: Vec::new(),
                managed_layers: CloudRequirementsTomlManagedLayers {
                    baseline: None,
                    system_overlay: Some(vec![CloudRequirementsFragment {
                        id: "overlay".to_string(),
                        name: "Overlay".to_string(),
                        contents: "allowed_approval_policies = [\"never\"]".to_string(),
                    }]),
                },
            },
        },
        &base_dir,
    )
    .expect("bundle should be converted into layers");

    assert_eq!(
        layers
            .baseline_config
            .expect("baseline should be present")
            .into_iter()
            .map(|layer| layer.name)
            .collect::<Vec<_>>(),
        vec![
            ConfigLayerSource::EnterpriseManaged {
                id: "baseline_low".to_string(),
                name: "Baseline low".to_string(),
            },
            ConfigLayerSource::EnterpriseManaged {
                id: "baseline_high".to_string(),
                name: "Baseline high".to_string(),
            },
        ]
    );
    assert_eq!(layers.system_overlay_config, Some(Vec::new()));
    assert!(layers.baseline_requirements.is_none());
    assert_eq!(
        compose_requirements(
            layers
                .system_overlay_requirements
                .expect("system overlay should be present")
        )
        .expect("requirements should compose")
        .expect("requirements should be present")
        .into_toml(),
        ConfigRequirementsToml {
            allowed_approval_policies: Some(vec![AskForApproval::Never]),
            ..Default::default()
        }
    );
}

#[test]
fn bundle_layers_can_strict_validate_managed_config() {
    let tempdir = tempdir().expect("tempdir");
    let base_dir = AbsolutePathBuf::from_absolute_path(tempdir.path()).expect("absolute path");
    let err = CloudConfigBundleLayers::from_bundle_strict_config(
        CloudConfigBundle {
            config_toml: CloudConfigTomlBundle {
                enterprise_managed: Vec::new(),
                managed_layers: CloudConfigTomlManagedLayers {
                    baseline: Some(vec![CloudConfigFragment {
                        id: "cfg".to_string(),
                        name: "Cloud config".to_string(),
                        contents: "unknown_key = true".to_string(),
                    }]),
                    system_overlay: None,
                },
            },
            requirements_toml: CloudRequirementsTomlBundle {
                enterprise_managed: Vec::new(),
                managed_layers: Default::default(),
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
