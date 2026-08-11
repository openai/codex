use super::*;
use crate::AbsolutePathBufGuard;
use crate::ConfigLayerSource;
use crate::ConfigRequirementsToml;
use crate::FilesystemDenyReadPattern;
use crate::SandboxModeRequirement;
use crate::compose_requirements;
use crate::compose_requirements_for_hostname;
use crate::config_requirements::FilesystemRequirementsToml;
use crate::config_requirements::PermissionsRequirementsToml;
use crate::config_toml::ConfigToml;
use crate::types::SandboxWorkspaceWrite;
use codex_protocol::protocol::AskForApproval;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::RwLock;
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
    let cloned_loader = loader.clone();

    let (first, second) = tokio::join!(loader.get(), cloned_loader.get());
    assert_eq!(first, second);
    assert_eq!(loader.get().await, first);
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn getter_returns_latest_result_across_clones() {
    let initial_error = CloudConfigBundleLoadError::new(
        CloudConfigBundleLoadErrorCode::RequestFailed,
        /*status_code*/ None,
        "initial load failed",
    );
    let latest = Arc::new(RwLock::new(Err(initial_error.clone())));
    let getter_latest = Arc::clone(&latest);
    let loader = CloudConfigBundleLoader::from_getter(move || {
        let latest = Arc::clone(&getter_latest);
        async move { latest.read().expect("bundle state lock").clone() }
    });
    let cloned_loader = loader.clone();

    assert_eq!(loader.get().await, Err(initial_error));

    let bundle = CloudConfigBundle {
        config_toml: CloudConfigTomlBundle {
            enterprise_managed: vec![CloudConfigFragment {
                id: "managed".to_string(),
                name: "Managed".to_string(),
                contents: "model = \"managed\"".to_string(),
            }],
        },
        ..Default::default()
    };
    *latest.write().expect("bundle state lock") = Ok(Some(bundle.clone()));

    assert_eq!(cloned_loader.get().await, Ok(Some(bundle)));
    assert_eq!(CloudConfigBundleLoader::default().get().await, Ok(None));

    *latest.write().expect("bundle state lock") = Ok(None);

    assert_eq!(loader.get().await, Ok(None));
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

#[test]
fn bundle_layers_resolve_paths_and_requirements_for_the_execution_host() {
    let temp_dir = tempdir().expect("temporary directories");
    let executor_home = temp_dir.path().join("executor-home");
    let executor_codex_home = AbsolutePathBuf::from_absolute_path(executor_home.join(".codex"))
        .expect("absolute executor Codex home");
    let bundle = CloudConfigBundle {
        config_toml: CloudConfigTomlBundle {
            enterprise_managed: vec![CloudConfigFragment {
                id: "config".to_string(),
                name: "Executor config".to_string(),
                contents: r#"
[sandbox_workspace_write]
writable_roots = ["~/cloud-root", "./relative-root"]
"#
                .to_string(),
            }],
        },
        requirements_toml: CloudRequirementsTomlBundle {
            enterprise_managed: vec![CloudRequirementsFragment {
                id: "requirements".to_string(),
                name: "Executor requirements".to_string(),
                contents: r#"
[permissions.filesystem]
deny_read = ["~/private"]

[[remote_sandbox_config]]
hostname_patterns = ["executor-*"]
allowed_sandbox_modes = ["read-only"]
"#
                .to_string(),
            }],
        },
    };

    let (config, requirements) = AbsolutePathBufGuard::with_home_directory(&executor_home, || {
        let layers =
            CloudConfigBundleLayers::from_bundle_strict_config(bundle, &executor_codex_home)
                .expect("executor bundle should convert into layers");
        let config: ConfigToml = layers.enterprise_managed_config[0]
            .config
            .clone()
            .try_into()
            .expect("deserialize executor config");
        let requirements = compose_requirements_for_hostname(
            layers.enterprise_managed_requirements,
            Some("executor-01"),
        )
        .expect("compose executor requirements")
        .expect("executor requirements should be present")
        .into_toml();
        (config, requirements)
    });

    assert_eq!(
        config.sandbox_workspace_write,
        Some(SandboxWorkspaceWrite {
            writable_roots: vec![
                AbsolutePathBuf::from_absolute_path(executor_home.join("cloud-root"))
                    .expect("absolute cloud root"),
                AbsolutePathBuf::from_absolute_path(
                    executor_codex_home.as_path().join("relative-root"),
                )
                .expect("absolute relative root"),
            ],
            ..Default::default()
        })
    );
    assert_eq!(
        requirements,
        ConfigRequirementsToml {
            allowed_sandbox_modes: Some(vec![SandboxModeRequirement::ReadOnly]),
            permissions: Some(PermissionsRequirementsToml {
                filesystem: Some(FilesystemRequirementsToml {
                    deny_read: Some(vec![FilesystemDenyReadPattern::from(
                        AbsolutePathBuf::from_absolute_path(executor_home.join("private"))
                            .expect("absolute private root"),
                    )]),
                }),
                ..Default::default()
            }),
            ..Default::default()
        }
    );
}
