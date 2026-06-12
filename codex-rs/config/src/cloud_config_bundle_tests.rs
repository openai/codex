use super::*;
use crate::ConfigLayerSource;
use crate::ConfigRequirementsToml;
use crate::compose_requirements;
use codex_protocol::protocol::AskForApproval;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

fn bundle_with_model(model: &str) -> CloudConfigBundle {
    CloudConfigBundle {
        config_toml: CloudConfigTomlBundle {
            enterprise_managed: vec![CloudConfigFragment {
                id: model.to_string(),
                name: model.to_string(),
                contents: format!("model = \"{model}\""),
            }],
        },
        ..Default::default()
    }
}

#[tokio::test]
async fn pending_loader_receives_initial_result() {
    let bundle = bundle_with_model("initial");
    let (loader, publisher) = CloudConfigBundleLoader::pending();
    let cloned_loader = loader.clone();
    let load_task = tokio::spawn(async move { tokio::join!(loader.get(), cloned_loader.get()) });
    tokio::task::yield_now().await;

    assert!(publisher.publish(Ok(Some(bundle.clone()))));

    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_secs(1), load_task)
            .await
            .expect("initial result should wake loaders")
            .expect("loader task"),
        (Ok(Some(bundle.clone())), Ok(Some(bundle)))
    );
}

#[tokio::test]
async fn successful_result_replaces_error_and_later_error_is_ignored() {
    let bundle = bundle_with_model("recovered");
    let initial_error = CloudConfigBundleLoadError::new(
        CloudConfigBundleLoadErrorCode::RequestFailed,
        /*status_code*/ None,
        "initial load failed",
    );
    let (loader, publisher) = CloudConfigBundleLoader::pending();

    assert!(publisher.publish(Err(initial_error.clone())));
    assert_eq!(loader.get().await, Err(initial_error));

    assert!(publisher.publish(Ok(Some(bundle.clone()))));
    assert_eq!(loader.get().await, Ok(Some(bundle.clone())));

    assert!(publisher.publish(Err(CloudConfigBundleLoadError::new(
        CloudConfigBundleLoadErrorCode::RequestFailed,
        /*status_code*/ None,
        "refresh failed",
    ))));
    assert_eq!(loader.get().await, Ok(Some(bundle)));
}

#[tokio::test]
async fn refresh_updates_only_loader_and_its_clones() {
    let initial_bundle = bundle_with_model("initial");
    let refreshed_bundle = bundle_with_model("refreshed");
    let independent_loader = CloudConfigBundleLoader::from_result(Ok(None));
    let (loader, publisher) = CloudConfigBundleLoader::pending();
    let cloned_loader = loader.clone();

    assert!(publisher.publish(Ok(Some(initial_bundle.clone()))));
    assert_eq!(loader.get().await, Ok(Some(initial_bundle)));

    assert!(publisher.publish(Ok(Some(refreshed_bundle.clone()))));

    assert_eq!(loader.get().await, Ok(Some(refreshed_bundle.clone())));
    assert_eq!(cloned_loader.get().await, Ok(Some(refreshed_bundle)));
    assert_eq!(independent_loader.get().await, Ok(None));

    assert!(publisher.publish(Ok(None)));

    assert_eq!(loader.get().await, Ok(None));

    drop(cloned_loader);
    drop(loader);
    assert!(!publisher.publish(Ok(Some(CloudConfigBundle::default()))));
}

#[tokio::test]
async fn pending_loader_reports_dropped_publisher() {
    let (loader, publisher) = CloudConfigBundleLoader::pending();

    drop(publisher);

    assert_eq!(
        loader.get().await,
        Err(CloudConfigBundleLoadError::new(
            CloudConfigBundleLoadErrorCode::Internal,
            /*status_code*/ None,
            "cloud config bundle lifecycle ended before startup completed",
        ))
    );
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
