use super::*;
use codex_file_system::CopyOptions;
use codex_file_system::CreateDirectoryOptions;
use codex_file_system::ExecutorFileSystemFuture;
use codex_file_system::FileMetadata;
use codex_file_system::FileSystemReadStream;
use codex_file_system::FileSystemSandboxContext;
use codex_file_system::ReadDirectoryEntry;
use codex_file_system::RemoveOptions;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

struct TestFileSystem;

impl ExecutorFileSystem for TestFileSystem {
    fn canonicalize<'a>(
        &'a self,
        path: &'a PathUri,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, PathUri> {
        Box::pin(async move {
            let path = path.to_abs_path()?;
            let canonicalized = path.canonicalize()?;
            Ok(PathUri::from_abs_path(&canonicalized))
        })
    }

    fn read_file<'a>(
        &'a self,
        path: &'a PathUri,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<u8>> {
        Box::pin(async move {
            let path = path.to_abs_path()?;
            tokio::fs::read(path.as_path()).await
        })
    }

    fn read_file_stream<'a>(
        &'a self,
        _path: &'a PathUri,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FileSystemReadStream> {
        Box::pin(async {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "test filesystem does not support streaming reads",
            ))
        })
    }

    fn write_file<'a>(
        &'a self,
        _path: &'a PathUri,
        _contents: Vec<u8>,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        Box::pin(async move { unimplemented!("test filesystem only supports reads") })
    }

    fn create_directory<'a>(
        &'a self,
        _path: &'a PathUri,
        _create_directory_options: CreateDirectoryOptions,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        Box::pin(async move { unimplemented!("test filesystem only supports reads") })
    }

    fn get_metadata<'a>(
        &'a self,
        path: &'a PathUri,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FileMetadata> {
        Box::pin(async move {
            let path = path.to_abs_path()?;
            let metadata = tokio::fs::symlink_metadata(path.as_path()).await?;
            Ok(FileMetadata {
                is_directory: metadata.is_dir(),
                is_file: metadata.is_file(),
                is_symlink: metadata.file_type().is_symlink(),
                size: metadata.len(),
                created_at_ms: 0,
                modified_at_ms: 0,
            })
        })
    }

    fn read_directory<'a>(
        &'a self,
        _path: &'a PathUri,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<ReadDirectoryEntry>> {
        Box::pin(async move { unimplemented!("test filesystem only supports reads") })
    }

    fn remove<'a>(
        &'a self,
        _path: &'a PathUri,
        _remove_options: RemoveOptions,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        Box::pin(async move { unimplemented!("test filesystem only supports reads") })
    }

    fn copy<'a>(
        &'a self,
        _source_path: &'a PathUri,
        _destination_path: &'a PathUri,
        _copy_options: CopyOptions,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        Box::pin(async move { unimplemented!("test filesystem only supports reads") })
    }
}

#[tokio::test]
async fn packaged_defaults_have_lower_precedence_than_existing_config_layers() {
    let tmp = tempdir().expect("tempdir");
    let packaged_defaults_path =
        AbsolutePathBuf::resolve_path_against_base("packaged-defaults.toml", tmp.path());
    let system_config_path = tmp.path().join("system.toml");
    let user_config_path = tmp.path().join(CONFIG_TOML_FILE);

    std::fs::write(
        packaged_defaults_path.as_path(),
        r#"
model = "packaged-model"
model_provider = "packaged-provider"
model_context_window = 120000
"#,
    )
    .expect("write packaged defaults");
    std::fs::write(
        &system_config_path,
        r#"
model = "system-model"
model_provider = "system-provider"
"#,
    )
    .expect("write system config");
    std::fs::write(&user_config_path, r#"model = "user-model""#).expect("write user config");

    let mut overrides = LoaderOverrides::without_managed_config_for_tests();
    overrides.packaged_defaults_path = Some(packaged_defaults_path.clone());
    overrides.system_config_path = Some(system_config_path.clone());

    let stack = load_config_layers_state(
        &TestFileSystem,
        tmp.path(),
        /*cwd*/ None,
        &[(
            "model".to_string(),
            TomlValue::String("session-model".to_string()),
        )],
        overrides,
        &crate::NoopThreadConfigLoader,
    )
    .await
    .expect("load config layers");

    assert_eq!(
        stack
            .all_layers_low_to_high()
            .map(|layer| layer.name.clone())
            .collect::<Vec<_>>(),
        vec![
            ConfigLayerSource::PackagedDefaults {
                file: packaged_defaults_path,
            },
            ConfigLayerSource::System {
                file: AbsolutePathBuf::from_absolute_path(system_config_path)
                    .expect("absolute system config path"),
            },
            ConfigLayerSource::User {
                file: AbsolutePathBuf::from_absolute_path(user_config_path)
                    .expect("absolute user config path"),
                profile: None,
            },
            ConfigLayerSource::SessionFlags,
        ]
    );
    assert_eq!(
        stack.effective_config(),
        toml::toml! {
            model = "session-model"
            model_provider = "system-provider"
            model_context_window = 120000
        }
        .into()
    );
}

#[tokio::test]
async fn missing_packaged_defaults_file_returns_an_error() {
    let tmp = tempdir().expect("tempdir");
    let packaged_defaults_path =
        AbsolutePathBuf::resolve_path_against_base("packaged-defaults.toml", tmp.path());
    let mut overrides = LoaderOverrides::without_managed_config_for_tests();
    overrides.packaged_defaults_path = Some(packaged_defaults_path.clone());

    let err = load_config_layers_state(
        &TestFileSystem,
        tmp.path(),
        /*cwd*/ None,
        &[],
        overrides,
        &crate::NoopThreadConfigLoader,
    )
    .await
    .expect_err("an explicitly configured packaged defaults file must exist");

    assert_eq!(err.kind(), io::ErrorKind::NotFound);
    assert_eq!(
        err.to_string(),
        format!(
            "packaged defaults config file {} not found",
            packaged_defaults_path.display()
        )
    );
}

#[cfg(windows)]
#[tokio::test]
async fn default_windows_managed_config_is_ignored_with_warning() {
    let tmp = tempdir().expect("tempdir");
    let codex_home = tmp.path().join("codex-home");
    std::fs::create_dir_all(&codex_home).expect("create codex home");
    let managed_config_path = codex_home.join("managed_config.toml");
    std::fs::write(
        &managed_config_path,
        r#"
model = "legacy-model"
approval_policy = "never"
sandbox_mode = "danger-full-access"
"#,
    )
    .expect("write default legacy managed config");
    std::fs::write(codex_home.join(CONFIG_TOML_FILE), r#"model = "user-model""#)
        .expect("write user config");

    let mut overrides = LoaderOverrides::without_managed_config_for_tests();
    overrides.managed_config_path = None;
    overrides.system_config_path = Some(tmp.path().join("system-config.toml"));
    overrides.system_requirements_path = Some(tmp.path().join("requirements.toml"));
    let stack = load_config_layers_state(
        &TestFileSystem,
        &codex_home,
        /*cwd*/ None,
        &[],
        overrides,
        &crate::NoopThreadConfigLoader,
    )
    .await
    .expect("load config layers");

    assert_eq!(
        stack.effective_config().get("model"),
        Some(&TomlValue::String("user-model".to_string()))
    );
    assert_eq!(stack.requirements_toml().allowed_approval_policies, None);
    assert_eq!(stack.requirements_toml().allowed_sandbox_modes, None);
    assert!(stack.all_layers_low_to_high().all(|layer| !matches!(
        &layer.name,
        ConfigLayerSource::LegacyManagedConfigTomlFromFile { .. }
    )));
    let expected_warnings = vec![format!(
        "Ignoring deprecated managed config file at {}; CODEX_HOME/managed_config.toml is no longer supported on Windows. Use %ProgramData%\\OpenAI\\Codex\\requirements.toml for enforced settings or config.toml for defaults.",
        managed_config_path.display()
    )];
    assert_eq!(stack.startup_warnings(), Some(expected_warnings.as_slice()));
}

#[cfg(windows)]
#[test]
fn windows_local_managed_configuration_ignores_legacy_file_but_detects_requirements() {
    let tmp = tempdir().expect("tempdir");
    let codex_home = tmp.path().join("codex-home");
    std::fs::create_dir_all(&codex_home).expect("create codex home");
    std::fs::write(codex_home.join("managed_config.toml"), "")
        .expect("write default legacy managed config");
    let system_requirements_path = tmp.path().join("requirements.toml");

    let legacy_only = has_local_managed_configuration_with_system_requirements_path(
        &codex_home,
        &system_requirements_path,
    )
    .expect("check legacy-only managed configuration");
    std::fs::write(&system_requirements_path, "").expect("write system requirements");
    let with_system_requirements = has_local_managed_configuration_with_system_requirements_path(
        &codex_home,
        &system_requirements_path,
    )
    .expect("check system managed configuration");

    assert_eq!((legacy_only, with_system_requirements), (false, true));
}

#[tokio::test]
async fn profile_v2_rejects_matching_legacy_profile_in_base_user_config() {
    let tmp = tempdir().expect("tempdir");
    let selected_config = tmp.path().join("work.config.toml");

    std::fs::write(
        tmp.path().join(CONFIG_TOML_FILE),
        r#"
model = "gpt-main"

[profiles.work]
model = "gpt-work"
"#,
    )
    .expect("write default user config");
    std::fs::write(&selected_config, r#"model = "gpt-work-v2""#)
        .expect("write selected user config");

    let mut overrides = LoaderOverrides::without_managed_config_for_tests();
    overrides.user_config_path = Some(AbsolutePathBuf::resolve_path_against_base(
        "work.config.toml",
        tmp.path(),
    ));
    overrides.user_config_profile = Some("work".parse().expect("profile-v2 name"));

    let err = load_config_layers_state(
        &TestFileSystem,
        tmp.path(),
        /*cwd*/ None,
        &[],
        overrides,
        &crate::NoopThreadConfigLoader,
    )
    .await
    .expect_err("profile-v2 should reject a matching legacy profile in base user config");

    assert_eq!(
        err.kind(),
        io::ErrorKind::InvalidData,
        "a matching legacy profile should be a hard config error"
    );
    let message = err.to_string();
    assert!(
        message.contains("--profile `work` cannot be used"),
        "unexpected error message: {message}"
    );
    assert!(
        message.contains("config.toml"),
        "unexpected error message: {message}"
    );
    assert!(
        message.contains("[profiles.work]"),
        "unexpected error message: {message}"
    );
    assert!(
        message.contains("https://developers.openai.com/codex/config-advanced#profiles"),
        "unexpected error message: {message}"
    );
}

#[tokio::test]
async fn profile_v2_rejects_matching_legacy_profile_selector_in_base_user_config() {
    let tmp = tempdir().expect("tempdir");
    let selected_config = tmp.path().join("work.config.toml");

    std::fs::write(
        tmp.path().join(CONFIG_TOML_FILE),
        r#"
profile = "work"
model = "gpt-main"
"#,
    )
    .expect("write default user config");
    std::fs::write(&selected_config, r#"model = "gpt-work-v2""#)
        .expect("write selected user config");

    let mut overrides = LoaderOverrides::without_managed_config_for_tests();
    overrides.user_config_path = Some(AbsolutePathBuf::resolve_path_against_base(
        "work.config.toml",
        tmp.path(),
    ));
    overrides.user_config_profile = Some("work".parse().expect("profile-v2 name"));

    let err = load_config_layers_state(
        &TestFileSystem,
        tmp.path(),
        /*cwd*/ None,
        &[],
        overrides,
        &crate::NoopThreadConfigLoader,
    )
    .await
    .expect_err("profile-v2 should reject a matching legacy profile selector");

    assert_eq!(
        err.kind(),
        io::ErrorKind::InvalidData,
        "a matching legacy profile selector should be a hard config error"
    );
    let message = err.to_string();
    assert!(
        message.contains("--profile `work` cannot be used"),
        "unexpected error message: {message}"
    );
    assert!(
        message.contains("profile = \"work\""),
        "unexpected error message: {message}"
    );
    assert!(
        message.contains("work.config.toml"),
        "unexpected error message: {message}"
    );
}

#[tokio::test]
async fn profile_v2_allows_unrelated_legacy_profiles_in_base_user_config() {
    let tmp = tempdir().expect("tempdir");
    let selected_config = tmp.path().join("work.config.toml");

    std::fs::write(
        tmp.path().join(CONFIG_TOML_FILE),
        r#"
model = "gpt-main"

[profiles.dev]
model = "gpt-dev"
"#,
    )
    .expect("write default user config");
    std::fs::write(&selected_config, r#"model = "gpt-work-v2""#)
        .expect("write selected user config");

    let mut overrides = LoaderOverrides::without_managed_config_for_tests();
    overrides.user_config_path = Some(AbsolutePathBuf::resolve_path_against_base(
        "work.config.toml",
        tmp.path(),
    ));
    overrides.user_config_profile = Some("work".parse().expect("profile-v2 name"));

    load_config_layers_state(
        &TestFileSystem,
        tmp.path(),
        /*cwd*/ None,
        &[],
        overrides,
        &crate::NoopThreadConfigLoader,
    )
    .await
    .expect("profile-v2 should allow unrelated legacy profiles in base user config");
}

#[test]
fn local_layer_projection_preserves_override_blockers_and_cloud_position() {
    let tmp = tempdir().expect("tempdir");
    let base_dir = AbsolutePathBuf::from_absolute_path(tmp.path()).expect("absolute base");
    let layer = |source, contents| LocalTomlLayer {
        source,
        base_dir: base_dir.clone(),
        toml: toml::from_str(contents).expect("valid TOML"),
    };
    let layers = LocalConfigLayers {
        config: LocalTomlLayerStack {
            layers: vec![
                layer(
                    ConfigLayerSource::System {
                        file: base_dir.join("system.toml"),
                    },
                    r#"ignored=true
                    "literal.key"="literal"
                    array=[1,2]
                    [a]
                    b=1
                    c=2
                    "#,
                ),
                layer(
                    ConfigLayerSource::SessionFlags,
                    "a=2\nignored=false\nonly_user=true",
                ),
                layer(
                    ConfigLayerSource::LegacyManagedConfigTomlFromMdm,
                    "[a]\nunrequested=true",
                ),
            ],
            cloud_insertion_index: 1,
        },
        requirements: LocalTomlLayerStack {
            layers: Vec::<LocalTomlLayer<RequirementSource>>::new(),
            cloud_insertion_index: 0,
        },
    };

    let only_user = layers.clone().project(&[vec!["only_user".into()]], &[]);
    assert_eq!(only_user.config.layers.len(), 1);
    assert_eq!(only_user.config.cloud_insertion_index, 0);

    let projected = layers.project(
        &[
            vec!["a".into(), "b".into()],
            vec!["array".into(), "unused".into()],
            vec!["literal.key".into()],
        ],
        &[],
    );

    assert_eq!(
        projected.config,
        LocalTomlLayerStack {
            layers: vec![
                layer(
                    ConfigLayerSource::System {
                        file: base_dir.join("system.toml"),
                    },
                    r#""literal.key"="literal"
                    array=[1,2]
                    [a]
                    b=1"#,
                ),
                layer(ConfigLayerSource::SessionFlags, "a=2"),
                layer(ConfigLayerSource::LegacyManagedConfigTomlFromMdm, "[a]"),
            ],
            cloud_insertion_index: 1,
        }
    );

    let mut merged = TomlValue::Table(toml::map::Map::new());
    for layer in projected.config.layers {
        merge_toml_values(&mut merged, &layer.toml);
    }
    assert_eq!(
        merged.get("a"),
        Some(&TomlValue::Table(toml::map::Map::new()))
    );
}

#[tokio::test]
async fn local_layers_keep_raw_paths_order_and_legacy_requirements() {
    let tmp = tempdir().expect("tempdir");
    let codex_home = tmp.path().join("codex-home");
    let project = tmp.path().join("project");
    let dot_codex = project.join(".codex");
    let system_dir = tmp.path().join("system");
    let managed_dir = tmp.path().join("managed");
    for dir in [&codex_home, &dot_codex, &system_dir, &managed_dir] {
        std::fs::create_dir_all(dir).expect("create fixture directory");
    }
    std::fs::write(project.join(".project-root"), "").expect("write project marker");

    let project_key = project_trust_key(&project);
    let project_key = TomlValue::String(project_key).to_string();
    let user_config = |trust_level| {
        format!(
            "project_root_markers=[\".project-root\"]\nmodel_instructions_file=\"./user.md\"\n[projects.{project_key}]\ntrust_level=\"{trust_level}\""
        )
    };
    let user_file = codex_home.join(CONFIG_TOML_FILE);
    std::fs::write(&user_file, user_config("trusted")).expect("write user config");
    let system_file = system_dir.join(CONFIG_TOML_FILE);
    std::fs::write(&system_file, "model_instructions_file = \"./system.md\"")
        .expect("write system config");
    std::fs::write(
        dot_codex.join(CONFIG_TOML_FILE),
        "model_instructions_file = \"./project.md\"\nopenai_base_url = \"https://ignored\"",
    )
    .expect("write project config");
    let managed_file = managed_dir.join("managed_config.toml");
    std::fs::write(
        &managed_file,
        "approval_policy = \"never\"\nsandbox_mode = \"workspace-write\"\nmodel_instructions_file = \"./managed.md\"",
    )
    .expect("write legacy managed config");
    let requirements_file = managed_dir.join("requirements.toml");
    std::fs::write(
        &requirements_file,
        "allowed_sandbox_modes = [\"read-only\"]\nlog_dir = \"./logs\"",
    )
    .expect("write system requirements");

    let mut overrides = LoaderOverrides::with_managed_config_path_for_tests(managed_file.clone());
    overrides.system_config_path = Some(system_file.clone());
    overrides.system_requirements_path = Some(requirements_file.clone());
    let cwd = AbsolutePathBuf::from_absolute_path(&project).expect("absolute cwd");
    let layers = local::load_local_config_layers_with_overrides(
        &TestFileSystem,
        &codex_home,
        &cwd,
        &overrides,
    )
    .await
    .expect("load local layers");

    assert_eq!(
        layers
            .config
            .layers
            .iter()
            .map(|layer| layer.base_dir.to_path_buf())
            .collect::<Vec<_>>(),
        vec![
            system_dir.clone(),
            codex_home.clone(),
            dot_codex.clone(),
            managed_dir.clone(),
        ]
    );
    assert_eq!(layers.config.cloud_insertion_index, 1);
    assert_eq!(layers.requirements.cloud_insertion_index, 1);
    assert_eq!(
        (
            layers.config.layers[2].toml.clone(),
            layers.config.layers[3]
                .toml
                .get("model_instructions_file")
                .cloned(),
            layers.requirements.layers[0]
                .toml
                .get("log_dir")
                .cloned(),
            layers.requirements.layers[1].toml.clone(),
        ),
        (
            toml::from_str("model_instructions_file = \"./project.md\"")
                .expect("project TOML"),
            Some(TomlValue::String("./managed.md".into())),
            Some(TomlValue::String("./logs".into())),
            toml::from_str(
                "allowed_approval_policies=[\"never\"]\nallowed_sandbox_modes=[\"read-only\",\"workspace-write\"]"
            )
            .expect("legacy requirements TOML"),
        )
    );

    std::fs::write(&user_file, user_config("untrusted")).expect("write user config");
    let layers = local::load_local_config_layers_with_overrides(
        &TestFileSystem,
        &codex_home,
        &cwd,
        &overrides,
    )
    .await
    .expect("load local layers");
    assert_eq!(
        layers
            .config
            .layers
            .iter()
            .filter(|layer| matches!(layer.source, ConfigLayerSource::Project { .. }))
            .count(),
        0
    );
}
