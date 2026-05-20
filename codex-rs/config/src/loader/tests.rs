use super::*;
use crate::ConfigLayerStackOrdering;
use async_trait::async_trait;
use codex_file_system::CopyOptions;
use codex_file_system::CreateDirectoryOptions;
use codex_file_system::FileMetadata;
use codex_file_system::FileSystemResult;
use codex_file_system::FileSystemSandboxContext;
use codex_file_system::ReadDirectoryEntry;
use codex_file_system::RemoveOptions;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

struct TestFileSystem;

#[async_trait]
impl ExecutorFileSystem for TestFileSystem {
    async fn read_file(
        &self,
        path: &AbsolutePathBuf,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<u8>> {
        tokio::fs::read(path.as_path()).await
    }

    async fn write_file(
        &self,
        _path: &AbsolutePathBuf,
        _contents: Vec<u8>,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        unimplemented!("test filesystem only supports reads")
    }

    async fn create_directory(
        &self,
        _path: &AbsolutePathBuf,
        _create_directory_options: CreateDirectoryOptions,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        unimplemented!("test filesystem only supports reads")
    }

    async fn get_metadata(
        &self,
        path: &AbsolutePathBuf,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<FileMetadata> {
        let metadata = tokio::fs::metadata(path.as_path()).await?;
        let symlink_metadata = tokio::fs::symlink_metadata(path.as_path()).await?;
        Ok(FileMetadata {
            is_directory: metadata.is_dir(),
            is_file: metadata.is_file(),
            is_symlink: symlink_metadata.file_type().is_symlink(),
            created_at_ms: 0,
            modified_at_ms: 0,
        })
    }

    async fn read_directory(
        &self,
        _path: &AbsolutePathBuf,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>> {
        unimplemented!("test filesystem only supports reads")
    }

    async fn remove(
        &self,
        _path: &AbsolutePathBuf,
        _remove_options: RemoveOptions,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        unimplemented!("test filesystem only supports reads")
    }

    async fn copy(
        &self,
        _source_path: &AbsolutePathBuf,
        _destination_path: &AbsolutePathBuf,
        _copy_options: CopyOptions,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        unimplemented!("test filesystem only supports reads")
    }
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
        CloudRequirementsLoader::default(),
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
        message.contains("--profile-v2 `work` cannot be used"),
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
        CloudRequirementsLoader::default(),
        &crate::NoopThreadConfigLoader,
    )
    .await
    .expect("profile-v2 should allow unrelated legacy profiles in base user config");
}

#[tokio::test]
async fn app_server_host_project_root_markers_apply_before_project_layer_discovery() {
    let tmp = tempdir().expect("tempdir");
    let project_root = tmp.path().join("repo");
    let cwd = project_root.join("nested");
    std::fs::create_dir_all(project_root.join(".codex")).expect("create project config dir");
    std::fs::create_dir_all(&cwd).expect("create cwd");
    std::fs::write(project_root.join("WORKSPACE"), "").expect("write project root marker");
    std::fs::write(
        project_root.join(".codex").join(CONFIG_TOML_FILE),
        "model = \"gpt-project\"\n",
    )
    .expect("write project config");

    let stack = load_config_layers_state(
        &TestFileSystem,
        tmp.path(),
        Some(AbsolutePathBuf::try_from(cwd).expect("absolute cwd")),
        &[],
        ConfigLoadOptions {
            loader_overrides: LoaderOverrides::without_managed_config_for_tests(),
            strict_config: false,
            app_server_host_config: Some(
                toml::from_str(r#"project_root_markers = ["WORKSPACE"]"#)
                    .expect("host config toml"),
            ),
        },
        CloudRequirementsLoader::default(),
        &crate::NoopThreadConfigLoader,
    )
    .await
    .expect("host project markers should load");

    assert!(
        stack
            .get_layers(
                ConfigLayerStackOrdering::LowestPrecedenceFirst,
                /*include_disabled*/ true,
            )
            .iter()
            .any(
                |layer| matches!(&layer.name, ConfigLayerSource::Project { .. })
                    && layer.config.get("model").and_then(TomlValue::as_str) == Some("gpt-project")
            ),
        "host project root markers should expose the ancestor project config layer"
    );
}
