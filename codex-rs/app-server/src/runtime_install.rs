use std::ffi::OsStr;
use std::io;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RuntimeInstallPaths;
use codex_app_server_protocol::RuntimeInstallResponse;
use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::Environment;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::RemoveOptions;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use uuid::Uuid;

use crate::error_code::internal_error;
use crate::error_code::invalid_params;

const PUBLISHED_ARTIFACT_NAME: &str = "codex-primary-runtime";

pub(crate) async fn finalize_runtime_install(
    environment: &Environment,
    mut response: RuntimeInstallResponse,
) -> Result<RuntimeInstallResponse, JSONRPCErrorError> {
    if response.paths.bundled_plugin_marketplace_paths.is_empty()
        && response.paths.bundled_skill_paths.is_empty()
        && response.paths.skills_to_remove.is_empty()
    {
        return Ok(response);
    }

    let codex_home = environment.codex_home().await?;
    response.paths =
        finalize_runtime_paths(environment.get_filesystem(), &codex_home, response.paths).await?;
    Ok(response)
}

async fn finalize_runtime_paths(
    fs: Arc<dyn ExecutorFileSystem>,
    codex_home: &AbsolutePathBuf,
    mut paths: RuntimeInstallPaths,
) -> Result<RuntimeInstallPaths, JSONRPCErrorError> {
    paths.bundled_plugin_marketplace_paths = materialize_bundled_plugin_marketplaces(
        Arc::clone(&fs),
        codex_home,
        &paths.bundled_plugin_marketplace_paths,
    )
    .await?;
    paths.bundled_skill_paths = sync_primary_runtime_skills(
        fs,
        codex_home,
        &paths.bundled_skill_paths,
        &paths.skills_to_remove,
    )
    .await?;
    Ok(paths)
}

async fn materialize_bundled_plugin_marketplaces(
    fs: Arc<dyn ExecutorFileSystem>,
    codex_home: &AbsolutePathBuf,
    marketplace_roots: &[AbsolutePathBuf],
) -> Result<Vec<AbsolutePathBuf>, JSONRPCErrorError> {
    if marketplace_roots.is_empty() {
        return Ok(Vec::new());
    }

    let destination_root = absolute_path(
        codex_home
            .as_path()
            .join("plugins")
            .join(PUBLISHED_ARTIFACT_NAME)
            .join("marketplaces"),
    )?;
    let mut materialized = Vec::with_capacity(marketplace_roots.len());
    for marketplace_root in marketplace_roots {
        let marketplace_name = marketplace_root.as_path().file_name().ok_or_else(|| {
            invalid_params("bundled plugin marketplace path has no directory name")
        })?;
        let destination = absolute_path(
            destination_root
                .as_path()
                .join(safe_path_segment(marketplace_name)),
        )?;
        replace_directory(Arc::clone(&fs), marketplace_root, &destination).await?;
        materialized.push(destination);
    }
    Ok(materialized)
}

async fn sync_primary_runtime_skills(
    fs: Arc<dyn ExecutorFileSystem>,
    codex_home: &AbsolutePathBuf,
    bundled_skill_paths: &[AbsolutePathBuf],
    skills_to_remove: &[String],
) -> Result<Vec<AbsolutePathBuf>, JSONRPCErrorError> {
    if bundled_skill_paths.is_empty() && skills_to_remove.is_empty() {
        return Ok(Vec::new());
    }

    if bundled_skill_paths.is_empty() {
        move_legacy_primary_runtime_skills(fs, codex_home, skills_to_remove).await?;
        return Ok(Vec::new());
    }

    let destination_root = absolute_path(
        codex_home
            .as_path()
            .join("skills")
            .join(PUBLISHED_ARTIFACT_NAME),
    )?;
    let staging_root = temporary_sibling_path(&destination_root, "staging")?;
    let result = async {
        create_directory(Arc::clone(&fs), &staging_root).await?;

        let mut materialized = Vec::with_capacity(bundled_skill_paths.len());
        for bundled_skill_path in bundled_skill_paths {
            let skill_root = absolute_path(
                bundled_skill_path
                    .as_path()
                    .parent()
                    .ok_or_else(|| {
                        invalid_params(format!(
                            "bundled skill path {} has no parent directory",
                            bundled_skill_path.display()
                        ))
                    })?
                    .to_path_buf(),
            )?;
            let skill_name = skill_root.as_path().file_name().ok_or_else(|| {
                invalid_params(format!(
                    "bundled skill path {} has no skill directory name",
                    bundled_skill_path.display()
                ))
            })?;
            let staged_skill_root = absolute_path(staging_root.as_path().join(skill_name))?;
            copy_directory(Arc::clone(&fs), &skill_root, &staged_skill_root).await?;
            materialized.push(absolute_path(
                destination_root.as_path().join(skill_name).join("SKILL.md"),
            )?);
        }

        publish_staged_directory(Arc::clone(&fs), &staging_root, &destination_root).await?;
        move_legacy_primary_runtime_skills(Arc::clone(&fs), codex_home, skills_to_remove).await?;
        Ok(materialized)
    }
    .await;
    cleanup_directory(&fs, &staging_root, "staged primary runtime skills").await;
    result
}

async fn move_legacy_primary_runtime_skills(
    fs: Arc<dyn ExecutorFileSystem>,
    codex_home: &AbsolutePathBuf,
    skills_to_remove: &[String],
) -> Result<(), JSONRPCErrorError> {
    if skills_to_remove.is_empty() {
        return Ok(());
    }

    let skills_root = absolute_path(codex_home.as_path().join("skills"))?;
    for skill_dir in skills_to_remove {
        let skill_root = resolve_legacy_skill_directory(&skills_root, skill_dir)?;
        let skill_root_uri = PathUri::from_abs_path(&skill_root);
        let metadata = match fs.get_metadata(&skill_root_uri, /*sandbox*/ None).await {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => {
                return Err(internal_error(format!(
                    "failed to inspect legacy skill directory {}: {err}",
                    skill_root.display()
                )));
            }
        };
        if !metadata.is_directory {
            continue;
        }

        let backup_path = absolute_path(
            codex_home
                .as_path()
                .join(".tmp")
                .join("legacy-primary-runtime-skills")
                .join(format!(
                    "{}-{}",
                    skill_root
                        .as_path()
                        .file_name()
                        .and_then(OsStr::to_str)
                        .unwrap_or("skill"),
                    Uuid::new_v4()
                )),
        )?;
        if let Some(parent) = backup_path.as_path().parent() {
            create_directory(Arc::clone(&fs), &absolute_path(parent.to_path_buf())?).await?;
        }
        copy_directory(Arc::clone(&fs), &skill_root, &backup_path).await?;
        remove_if_exists(
            Arc::clone(&fs),
            &skill_root,
            RemoveOptions {
                recursive: true,
                force: true,
            },
        )
        .await?;
        tracing::info!(
            skill_dir = %skill_dir,
            skill_root = %skill_root.display(),
            backup_path = %backup_path.display(),
            "moved legacy primary runtime skill"
        );
    }
    Ok(())
}

fn resolve_legacy_skill_directory(
    skills_root: &AbsolutePathBuf,
    skill_dir: &str,
) -> Result<AbsolutePathBuf, JSONRPCErrorError> {
    let relative = Path::new(skill_dir);
    if !relative
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(invalid_params(format!(
            "legacy primary runtime skill path must stay within the skills directory: {skill_dir}"
        )));
    }
    absolute_path(skills_root.as_path().join(relative))
}

async fn replace_directory(
    fs: Arc<dyn ExecutorFileSystem>,
    source: &AbsolutePathBuf,
    destination: &AbsolutePathBuf,
) -> Result<(), JSONRPCErrorError> {
    if let Some(parent) = destination.as_path().parent() {
        create_directory(Arc::clone(&fs), &absolute_path(parent.to_path_buf())?).await?;
    }
    let staging_path = temporary_sibling_path(destination, "staging")?;
    let result = async {
        copy_directory(Arc::clone(&fs), source, &staging_path).await?;
        publish_staged_directory(Arc::clone(&fs), &staging_path, destination).await
    }
    .await;
    cleanup_directory(&fs, &staging_path, "staged runtime directory").await;
    result
}

async fn publish_staged_directory(
    fs: Arc<dyn ExecutorFileSystem>,
    staging_path: &AbsolutePathBuf,
    destination: &AbsolutePathBuf,
) -> Result<(), JSONRPCErrorError> {
    let backup_path = temporary_sibling_path(destination, "previous")?;
    let result = async {
        let destination_exists = path_exists(Arc::clone(&fs), destination).await?;
        if destination_exists {
            copy_directory(Arc::clone(&fs), destination, &backup_path).await?;
        }

        remove_if_exists(
            Arc::clone(&fs),
            destination,
            RemoveOptions {
                recursive: true,
                force: true,
            },
        )
        .await?;

        if let Err(error) = copy_directory(Arc::clone(&fs), staging_path, destination).await {
            remove_if_exists(
                Arc::clone(&fs),
                destination,
                RemoveOptions {
                    recursive: true,
                    force: true,
                },
            )
            .await?;
            if destination_exists
                && let Err(restore_error) =
                    copy_directory(Arc::clone(&fs), &backup_path, destination).await
            {
                return Err(internal_error(format!(
                    "failed to restore published runtime directory {} after replacement failed: {}; restore failed: {}",
                    destination.display(),
                    error.message,
                    restore_error.message
                )));
            }
            return Err(error);
        }

        Ok(())
    }
    .await;
    if result.is_ok() {
        cleanup_directory(&fs, &backup_path, "previous runtime directory").await;
    }
    result
}

async fn copy_directory(
    fs: Arc<dyn ExecutorFileSystem>,
    source: &AbsolutePathBuf,
    destination: &AbsolutePathBuf,
) -> Result<(), JSONRPCErrorError> {
    let source_uri = PathUri::from_abs_path(source);
    let destination_uri = PathUri::from_abs_path(destination);
    fs.copy(
        &source_uri,
        &destination_uri,
        CopyOptions { recursive: true },
        /*sandbox*/ None,
    )
    .await
    .map_err(|err| {
        internal_error(format!(
            "failed to copy directory {} to {}: {err}",
            source.display(),
            destination.display()
        ))
    })
}

async fn create_directory(
    fs: Arc<dyn ExecutorFileSystem>,
    path: &AbsolutePathBuf,
) -> Result<(), JSONRPCErrorError> {
    let path_uri = PathUri::from_abs_path(path);
    fs.create_directory(
        &path_uri,
        CreateDirectoryOptions { recursive: true },
        /*sandbox*/ None,
    )
    .await
    .map_err(|err| {
        internal_error(format!(
            "failed to create directory {}: {err}",
            path.display()
        ))
    })
}

async fn remove_if_exists(
    fs: Arc<dyn ExecutorFileSystem>,
    path: &AbsolutePathBuf,
    options: RemoveOptions,
) -> Result<(), JSONRPCErrorError> {
    let path_uri = PathUri::from_abs_path(path);
    match fs.remove(&path_uri, options, /*sandbox*/ None).await {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(internal_error(format!(
            "failed to remove directory {}: {err}",
            path.display()
        ))),
    }
}

async fn path_exists(
    fs: Arc<dyn ExecutorFileSystem>,
    path: &AbsolutePathBuf,
) -> Result<bool, JSONRPCErrorError> {
    let path_uri = PathUri::from_abs_path(path);
    match fs.get_metadata(&path_uri, /*sandbox*/ None).await {
        Ok(_) => Ok(true),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(internal_error(format!(
            "failed to inspect runtime path {}: {err}",
            path.display()
        ))),
    }
}

async fn cleanup_directory(fs: &Arc<dyn ExecutorFileSystem>, path: &AbsolutePathBuf, label: &str) {
    if let Err(error) = remove_if_exists(
        Arc::clone(fs),
        path,
        RemoveOptions {
            recursive: true,
            force: true,
        },
    )
    .await
    {
        tracing::warn!(
            path = %path.display(),
            error = %error.message,
            "failed to clean up {label}"
        );
    }
}

fn temporary_sibling_path(
    destination: &AbsolutePathBuf,
    label: &str,
) -> Result<AbsolutePathBuf, JSONRPCErrorError> {
    let parent = destination.as_path().parent().ok_or_else(|| {
        internal_error(format!(
            "runtime destination {} has no parent directory",
            destination.display()
        ))
    })?;
    let destination_name = destination
        .as_path()
        .file_name()
        .map(safe_path_segment)
        .unwrap_or_else(|| "runtime-item".to_string());
    absolute_path(parent.join(format!(".{destination_name}-{label}-{}", Uuid::new_v4())))
}

fn safe_path_segment(segment: &OsStr) -> String {
    let safe = segment
        .to_string_lossy()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    let safe = safe.trim_matches('.').to_string();
    if safe.is_empty() || safe == ".." {
        "runtime-item".to_string()
    } else {
        safe
    }
}

fn absolute_path(path: PathBuf) -> Result<AbsolutePathBuf, JSONRPCErrorError> {
    AbsolutePathBuf::from_absolute_path_checked(path)
        .map_err(|err| internal_error(format!("runtime path is not absolute: {err}")))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use codex_app_server_protocol::RuntimeInstallPaths;
    use codex_exec_server::LocalFileSystem;
    use pretty_assertions::assert_eq;
    use tokio::fs;

    use super::*;

    #[tokio::test]
    async fn finalize_runtime_paths_materializes_marketplaces_and_skills() {
        let codex_home = tempfile::tempdir().expect("codex home");
        let runtime = tempfile::tempdir().expect("runtime");
        let marketplace_root = runtime.path().join("market");
        fs::create_dir_all(marketplace_root.join(".agents/plugins"))
            .await
            .expect("create marketplace manifest dir");
        fs::write(
            marketplace_root.join(".agents/plugins/marketplace.json"),
            r#"{"name":"debug","plugins":[]}"#,
        )
        .await
        .expect("write marketplace");

        let bundled_skill_root = runtime.path().join("skills").join("debug");
        fs::create_dir_all(&bundled_skill_root)
            .await
            .expect("create bundled skill");
        fs::write(bundled_skill_root.join("SKILL.md"), "debug")
            .await
            .expect("write bundled skill");

        let legacy_skill_root = codex_home.path().join("skills").join("legacy");
        fs::create_dir_all(&legacy_skill_root)
            .await
            .expect("create legacy skill");
        fs::write(legacy_skill_root.join("SKILL.md"), "legacy")
            .await
            .expect("write legacy skill");

        let paths = RuntimeInstallPaths {
            bundled_plugin_marketplace_paths: vec![
                absolute_path(marketplace_root).expect("absolute marketplace path"),
            ],
            bundled_skill_paths: vec![
                absolute_path(bundled_skill_root.join("SKILL.md")).expect("absolute skill path"),
            ],
            node_modules_path: absolute_path(runtime.path().join("node_modules"))
                .expect("absolute node modules path"),
            node_path: absolute_path(runtime.path().join("node")).expect("absolute node path"),
            python_path: absolute_path(runtime.path().join("python"))
                .expect("absolute python path"),
            skills_to_remove: vec!["legacy".to_string()],
        };

        let finalized = finalize_runtime_paths(
            Arc::new(LocalFileSystem::unsandboxed()),
            &absolute_path(codex_home.path().to_path_buf()).expect("absolute codex home"),
            paths,
        )
        .await
        .expect("finalize runtime paths");

        let expected_marketplace_root = codex_home
            .path()
            .join("plugins")
            .join(PUBLISHED_ARTIFACT_NAME)
            .join("marketplaces")
            .join("market");
        let expected_skill_path = codex_home
            .path()
            .join("skills")
            .join(PUBLISHED_ARTIFACT_NAME)
            .join("debug")
            .join("SKILL.md");
        assert_eq!(
            finalized.bundled_plugin_marketplace_paths,
            vec![absolute_path(expected_marketplace_root.clone()).expect("absolute path")]
        );
        assert_eq!(
            finalized.bundled_skill_paths,
            vec![absolute_path(expected_skill_path.clone()).expect("absolute path")]
        );
        assert!(
            expected_marketplace_root
                .join(".agents/plugins/marketplace.json")
                .is_file()
        );
        assert_eq!(
            fs::read_to_string(expected_skill_path)
                .await
                .expect("read materialized skill"),
            "debug"
        );
        assert!(!legacy_skill_root.exists());
        assert_eq!(
            std::fs::read_dir(
                codex_home
                    .path()
                    .join(".tmp")
                    .join("legacy-primary-runtime-skills")
            )
            .expect("read legacy backups")
            .count(),
            1
        );
    }

    #[tokio::test]
    async fn move_legacy_primary_runtime_skills_rejects_parent_path_without_removing_skill() {
        let codex_home = tempfile::tempdir().expect("codex home");
        let existing_skill_path = codex_home
            .path()
            .join("skills")
            .join("existing")
            .join("SKILL.md");
        fs::create_dir_all(existing_skill_path.parent().expect("skill parent"))
            .await
            .expect("create existing skill");
        fs::write(&existing_skill_path, "existing")
            .await
            .expect("write existing skill");

        let error = move_legacy_primary_runtime_skills(
            Arc::new(LocalFileSystem::unsandboxed()),
            &absolute_path(codex_home.path().to_path_buf()).expect("absolute codex home"),
            &["../existing".to_string()],
        )
        .await
        .expect_err("parent path should fail");

        assert!(
            error.message.contains(
                "legacy primary runtime skill path must stay within the skills directory"
            )
        );
        assert_eq!(
            fs::read_to_string(existing_skill_path)
                .await
                .expect("read existing skill"),
            "existing"
        );
    }

    #[tokio::test]
    async fn materialize_bundled_plugin_marketplaces_preserves_existing_copy_on_copy_failure() {
        let codex_home = tempfile::tempdir().expect("codex home");
        let runtime = tempfile::tempdir().expect("runtime");
        let missing_marketplace_root = runtime.path().join("market");
        let published_manifest = codex_home
            .path()
            .join("plugins")
            .join(PUBLISHED_ARTIFACT_NAME)
            .join("marketplaces")
            .join("market")
            .join(".agents/plugins/marketplace.json");
        fs::create_dir_all(published_manifest.parent().expect("manifest parent"))
            .await
            .expect("create published marketplace");
        fs::write(&published_manifest, "previous")
            .await
            .expect("write published marketplace");

        let error = materialize_bundled_plugin_marketplaces(
            Arc::new(LocalFileSystem::unsandboxed()),
            &absolute_path(codex_home.path().to_path_buf()).expect("absolute codex home"),
            &[absolute_path(missing_marketplace_root).expect("absolute marketplace path")],
        )
        .await
        .expect_err("missing marketplace should fail");

        assert!(error.message.contains("failed to copy directory"));
        assert_eq!(
            fs::read_to_string(published_manifest)
                .await
                .expect("read published marketplace"),
            "previous"
        );
    }

    #[tokio::test]
    async fn sync_primary_runtime_skills_preserves_existing_copy_on_copy_failure() {
        let codex_home = tempfile::tempdir().expect("codex home");
        let runtime = tempfile::tempdir().expect("runtime");
        let missing_skill_path = runtime.path().join("skills").join("debug").join("SKILL.md");
        let published_skill_path = codex_home
            .path()
            .join("skills")
            .join(PUBLISHED_ARTIFACT_NAME)
            .join("existing")
            .join("SKILL.md");
        fs::create_dir_all(published_skill_path.parent().expect("skill parent"))
            .await
            .expect("create published skill");
        fs::write(&published_skill_path, "previous")
            .await
            .expect("write published skill");

        let error = sync_primary_runtime_skills(
            Arc::new(LocalFileSystem::unsandboxed()),
            &absolute_path(codex_home.path().to_path_buf()).expect("absolute codex home"),
            &[absolute_path(missing_skill_path).expect("absolute skill path")],
            &[],
        )
        .await
        .expect_err("missing skill should fail");

        assert!(error.message.contains("failed to copy directory"));
        assert_eq!(
            fs::read_to_string(published_skill_path)
                .await
                .expect("read published skill"),
            "previous"
        );
    }
}
