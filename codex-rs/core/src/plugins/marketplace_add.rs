use super::OPENAI_CURATED_MARKETPLACE_NAME;
use super::marketplace_install_root;
use super::validate_plugin_segment;
use codex_core_plugins::manifest::load_plugin_manifest;
use codex_core_plugins::marketplace::MarketplacePluginSource;
use codex_core_plugins::marketplace::find_marketplace_manifest_path;
use codex_core_plugins::marketplace::load_marketplace;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use tempfile::Builder;

mod install;
mod metadata;
mod source;

use install::clone_git_source;
use install::ensure_marketplace_destination_is_inside_install_root;
use install::marketplace_staging_root;
use install::replace_marketplace_root;
use install::safe_marketplace_dir_name;
use metadata::MarketplaceInstallMetadata;
use metadata::find_marketplace_root_by_name;
use metadata::installed_marketplace_root_for_source;
use metadata::record_added_marketplace_entry;
use source::MarketplaceSource;
use source::parse_marketplace_source;
use source::stage_marketplace_source;
use source::validate_marketplace_source_root;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketplaceAddRequest {
    pub source: String,
    pub ref_name: Option<String>,
    pub sparse_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketplaceAddOutcome {
    pub marketplace_name: String,
    pub source_display: String,
    pub installed_root: AbsolutePathBuf,
    pub already_added: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum MarketplaceAddError {
    #[error("{0}")]
    InvalidRequest(String),
    #[error("{0}")]
    Internal(String),
}

pub async fn add_marketplace(
    codex_home: PathBuf,
    request: MarketplaceAddRequest,
) -> Result<MarketplaceAddOutcome, MarketplaceAddError> {
    tokio::task::spawn_blocking(move || add_marketplace_sync(codex_home.as_path(), request))
        .await
        .map_err(|err| MarketplaceAddError::Internal(format!("failed to add marketplace: {err}")))?
}

fn add_marketplace_sync(
    codex_home: &Path,
    request: MarketplaceAddRequest,
) -> Result<MarketplaceAddOutcome, MarketplaceAddError> {
    add_marketplace_sync_with_cloner(codex_home, request, clone_git_source)
}

fn add_marketplace_sync_with_cloner<F>(
    codex_home: &Path,
    request: MarketplaceAddRequest,
    clone_source: F,
) -> Result<MarketplaceAddOutcome, MarketplaceAddError>
where
    F: Fn(&str, Option<&str>, &[String], &Path) -> Result<(), MarketplaceAddError>,
{
    let MarketplaceAddRequest {
        source,
        ref_name,
        sparse_paths,
    } = request;
    let source = parse_marketplace_source(&source, ref_name)?;

    let install_root = marketplace_install_root(codex_home);
    fs::create_dir_all(&install_root).map_err(|err| {
        MarketplaceAddError::Internal(format!(
            "failed to create marketplace install directory {}: {err}",
            install_root.display()
        ))
    })?;

    let install_metadata = MarketplaceInstallMetadata::from_source(&source, &sparse_paths);
    if let Some(existing_root) =
        installed_marketplace_root_for_source(codex_home, &install_root, &install_metadata)?
    {
        materialize_remote_plugin_sources(&existing_root, &clone_source)?;
        let marketplace_name = validate_marketplace_source_root(&existing_root)?;
        record_added_marketplace_entry(codex_home, &marketplace_name, &install_metadata)?;
        return Ok(MarketplaceAddOutcome {
            marketplace_name,
            source_display: source.display(),
            installed_root: AbsolutePathBuf::try_from(existing_root).map_err(|err| {
                MarketplaceAddError::Internal(format!(
                    "failed to resolve installed marketplace root: {err}"
                ))
            })?,
            already_added: true,
        });
    }

    if let MarketplaceSource::Local { path } = &source {
        let marketplace_name = validate_marketplace_source_root(path)?;
        if marketplace_name == OPENAI_CURATED_MARKETPLACE_NAME {
            return Err(MarketplaceAddError::InvalidRequest(format!(
                "marketplace '{OPENAI_CURATED_MARKETPLACE_NAME}' is reserved and cannot be added from {}",
                source.display()
            )));
        }
        if find_marketplace_root_by_name(codex_home, &install_root, &marketplace_name)?.is_some() {
            return Err(MarketplaceAddError::InvalidRequest(format!(
                "marketplace '{marketplace_name}' is already added from a different source; remove it before adding {}",
                source.display()
            )));
        }
        materialize_remote_plugin_sources(path, &clone_source)?;
        record_added_marketplace_entry(codex_home, &marketplace_name, &install_metadata)?;
        return Ok(MarketplaceAddOutcome {
            marketplace_name,
            source_display: source.display(),
            installed_root: AbsolutePathBuf::try_from(path.clone()).map_err(|err| {
                MarketplaceAddError::Internal(format!(
                    "failed to resolve installed marketplace root: {err}"
                ))
            })?,
            already_added: false,
        });
    }

    let staging_root = marketplace_staging_root(&install_root);
    fs::create_dir_all(&staging_root).map_err(|err| {
        MarketplaceAddError::Internal(format!(
            "failed to create marketplace staging directory {}: {err}",
            staging_root.display()
        ))
    })?;
    let staged_root = Builder::new()
        .prefix("marketplace-add-")
        .tempdir_in(&staging_root)
        .map_err(|err| {
            MarketplaceAddError::Internal(format!(
                "failed to create temporary marketplace directory in {}: {err}",
                staging_root.display()
            ))
        })?;
    let staged_root = staged_root.keep();

    stage_marketplace_source(&source, &sparse_paths, &staged_root, &clone_source)?;

    let marketplace_name = validate_marketplace_source_root(&staged_root)?;
    if marketplace_name == OPENAI_CURATED_MARKETPLACE_NAME {
        return Err(MarketplaceAddError::InvalidRequest(format!(
            "marketplace '{OPENAI_CURATED_MARKETPLACE_NAME}' is reserved and cannot be added from {}",
            source.display()
        )));
    }

    let destination = install_root.join(safe_marketplace_dir_name(&marketplace_name)?);
    ensure_marketplace_destination_is_inside_install_root(&install_root, &destination)?;
    if destination.exists() {
        return Err(MarketplaceAddError::InvalidRequest(format!(
            "marketplace '{marketplace_name}' is already added from a different source; remove it before adding {}",
            source.display()
        )));
    }
    materialize_remote_plugin_sources(&staged_root, &clone_source)?;

    replace_marketplace_root(&staged_root, &destination).map_err(|err| {
        MarketplaceAddError::Internal(format!(
            "failed to install marketplace at {}: {err}",
            destination.display()
        ))
    })?;
    if let Err(err) =
        record_added_marketplace_entry(codex_home, &marketplace_name, &install_metadata)
    {
        if let Err(rollback_err) = fs::rename(&destination, &staged_root) {
            return Err(MarketplaceAddError::Internal(format!(
                "{err}; additionally failed to roll back installed marketplace at {}: {rollback_err}",
                destination.display()
            )));
        }
        return Err(err);
    }

    Ok(MarketplaceAddOutcome {
        marketplace_name,
        source_display: source.display(),
        installed_root: AbsolutePathBuf::try_from(destination).map_err(|err| {
            MarketplaceAddError::Internal(format!(
                "failed to resolve installed marketplace root: {err}"
            ))
        })?,
        already_added: false,
    })
}

fn materialize_remote_plugin_sources<F>(
    marketplace_root: &Path,
    clone_source: F,
) -> Result<(), MarketplaceAddError>
where
    F: Fn(&str, Option<&str>, &[String], &Path) -> Result<(), MarketplaceAddError>,
{
    let Some(marketplace_path) = find_marketplace_manifest_path(marketplace_root) else {
        return Err(MarketplaceAddError::InvalidRequest(format!(
            "marketplace root {} does not contain a supported manifest",
            marketplace_root.display()
        )));
    };
    let marketplace = load_marketplace(&marketplace_path)
        .map_err(|err| MarketplaceAddError::InvalidRequest(err.to_string()))?;
    let remote_sources_root = marketplace_root.join(".codex-remote-sources");
    let staging_root = remote_sources_root.join(".staging");
    let mut materialized_names = HashSet::new();

    for plugin in marketplace.plugins {
        let MarketplacePluginSource::Git {
            url,
            path,
            ref_name,
            sha,
            ..
        } = plugin.source
        else {
            continue;
        };
        if !materialized_names.insert(plugin.name.clone()) {
            return Err(MarketplaceAddError::InvalidRequest(format!(
                "duplicate git plugin `{}` in marketplace `{}`",
                plugin.name, marketplace.name
            )));
        }

        fs::create_dir_all(&staging_root).map_err(|err| {
            MarketplaceAddError::Internal(format!(
                "failed to create remote plugin source staging directory {}: {err}",
                staging_root.display()
            ))
        })?;
        let staged_plugin_root = Builder::new()
            .prefix(&format!("{}-", plugin.name))
            .tempdir_in(&staging_root)
            .map_err(|err| {
                MarketplaceAddError::Internal(format!(
                    "failed to create remote plugin source staging directory in {}: {err}",
                    staging_root.display()
                ))
            })?;
        let checkout_root = staged_plugin_root.path().join("checkout");
        let empty_sparse_paths: &[String] = &[];
        clone_source(
            &url,
            sha.as_deref().or(ref_name.as_deref()),
            empty_sparse_paths,
            &checkout_root,
        )?;

        let plugin_root = match path.as_deref() {
            Some(path) => checkout_root.join(path),
            None => checkout_root,
        };
        if !plugin_root.is_dir() {
            return Err(MarketplaceAddError::InvalidRequest(format!(
                "materialized git plugin `{}` does not exist at {}",
                plugin.name,
                plugin_root.display()
            )));
        }
        if load_plugin_manifest(&plugin_root).is_none() {
            return Err(MarketplaceAddError::InvalidRequest(format!(
                "materialized git plugin `{}` is missing .codex-plugin/plugin.json at {}",
                plugin.name,
                plugin_root.display()
            )));
        }

        let destination = remote_plugin_source_root(&remote_sources_root, &plugin.name)?;
        replace_materialized_plugin_root(staged_plugin_root.path(), &destination, &staging_root)?;
    }

    if let Err(err) = fs::remove_dir(&staging_root)
        && err.kind() != std::io::ErrorKind::NotFound
        && err.kind() != std::io::ErrorKind::DirectoryNotEmpty
    {
        return Err(MarketplaceAddError::Internal(format!(
            "failed to clean up remote plugin source staging directory {}: {err}",
            staging_root.display()
        )));
    }

    Ok(())
}

fn remote_plugin_source_root(
    remote_sources_root: &Path,
    plugin_name: &str,
) -> Result<PathBuf, MarketplaceAddError> {
    validate_plugin_segment(plugin_name, "plugin name")
        .map_err(MarketplaceAddError::InvalidRequest)?;
    Ok(remote_sources_root.join(plugin_name))
}

fn replace_materialized_plugin_root(
    staged_plugin_root: &Path,
    destination: &Path,
    staging_root: &Path,
) -> Result<(), MarketplaceAddError> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            MarketplaceAddError::Internal(format!(
                "failed to create remote plugin source directory {}: {err}",
                parent.display()
            ))
        })?;
    }

    let backup = if destination.exists() {
        fs::create_dir_all(staging_root).map_err(|err| {
            MarketplaceAddError::Internal(format!(
                "failed to create remote plugin source staging directory {}: {err}",
                staging_root.display()
            ))
        })?;
        let backup = Builder::new()
            .prefix("backup-")
            .tempdir_in(staging_root)
            .map_err(|err| {
                MarketplaceAddError::Internal(format!(
                    "failed to create remote plugin source backup directory in {}: {err}",
                    staging_root.display()
                ))
            })?;
        let backup_path = backup.path().to_path_buf();
        fs::remove_dir(&backup_path).map_err(|err| {
            MarketplaceAddError::Internal(format!(
                "failed to prepare remote plugin source backup directory {}: {err}",
                backup_path.display()
            ))
        })?;
        fs::rename(destination, &backup_path).map_err(|err| {
            MarketplaceAddError::Internal(format!(
                "failed to back up existing remote plugin source {}: {err}",
                destination.display()
            ))
        })?;
        let _ = backup.keep();
        Some(backup_path)
    } else {
        None
    };

    if let Err(err) = fs::rename(staged_plugin_root, destination) {
        if let Some(backup) = &backup
            && let Err(rollback_err) = fs::rename(backup, destination)
        {
            return Err(MarketplaceAddError::Internal(format!(
                "failed to install remote plugin source at {}: {err}; additionally failed to restore previous source from {}: {rollback_err}",
                destination.display(),
                backup.display()
            )));
        }
        return Err(MarketplaceAddError::Internal(format!(
            "failed to install remote plugin source at {}: {err}",
            destination.display()
        )));
    }

    if let Some(backup) = backup
        && let Err(err) = fs::remove_dir_all(&backup)
    {
        return Err(MarketplaceAddError::Internal(format!(
            "failed to remove remote plugin source backup directory {}: {err}",
            backup.display()
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn add_marketplace_sync_installs_marketplace_and_updates_config() -> Result<()> {
        let codex_home = TempDir::new()?;
        let source_root = TempDir::new()?;
        write_marketplace_source(source_root.path(), "remote copy")?;

        let result = add_marketplace_sync_with_cloner(
            codex_home.path(),
            MarketplaceAddRequest {
                source: "https://github.com/owner/repo.git".to_string(),
                ref_name: None,
                sparse_paths: Vec::new(),
            },
            |_url, _ref_name, _sparse_paths, destination| {
                copy_dir_all(source_root.path(), destination)
                    .map_err(|err| MarketplaceAddError::Internal(err.to_string()))
            },
        )?;

        assert_eq!(result.marketplace_name, "debug");
        assert_eq!(result.source_display, "https://github.com/owner/repo.git");
        assert!(!result.already_added);
        assert!(
            result
                .installed_root
                .as_path()
                .join(".agents/plugins/marketplace.json")
                .is_file()
        );

        let config = fs::read_to_string(codex_home.path().join(codex_config::CONFIG_TOML_FILE))?;
        assert!(config.contains("[marketplaces.debug]"));
        assert!(config.contains("source_type = \"git\""));
        assert!(config.contains("source = \"https://github.com/owner/repo.git\""));
        Ok(())
    }

    #[test]
    fn add_marketplace_sync_materializes_remote_plugin_sources() -> Result<()> {
        let codex_home = TempDir::new()?;
        let marketplace_source_root = TempDir::new()?;
        let plugin_source_root = TempDir::new()?;
        let marketplace_url = "https://github.com/owner/marketplace.git";
        let plugin_url = "https://github.com/owner/toolkit.git";
        write_marketplace_source_with_remote_plugin(marketplace_source_root.path(), plugin_url)?;
        write_plugin_source(plugin_source_root.path(), "plugins/toolkit", "toolkit")?;

        let result = add_marketplace_sync_with_cloner(
            codex_home.path(),
            MarketplaceAddRequest {
                source: marketplace_url.to_string(),
                ref_name: None,
                sparse_paths: Vec::new(),
            },
            |url, _ref_name, _sparse_paths, destination| {
                let source = match url {
                    url if url == marketplace_url => marketplace_source_root.path(),
                    url if url == plugin_url => plugin_source_root.path(),
                    _ => {
                        return Err(MarketplaceAddError::Internal(format!(
                            "unexpected clone url: {url}"
                        )));
                    }
                };
                copy_dir_all(source, destination)
                    .map_err(|err| MarketplaceAddError::Internal(err.to_string()))
            },
        )?;

        assert_eq!(result.marketplace_name, "debug");
        assert!(
            result
                .installed_root
                .as_path()
                .join(".codex-remote-sources/toolkit/checkout/plugins/toolkit/.codex-plugin/plugin.json")
                .is_file()
        );
        Ok(())
    }

    #[test]
    fn add_marketplace_sync_installs_local_directory_source_and_updates_config() -> Result<()> {
        let codex_home = TempDir::new()?;
        let source_root = TempDir::new()?;
        write_marketplace_source(source_root.path(), "local copy")?;

        let result = add_marketplace_sync_with_cloner(
            codex_home.path(),
            MarketplaceAddRequest {
                source: source_root.path().display().to_string(),
                ref_name: None,
                sparse_paths: Vec::new(),
            },
            |_url, _ref_name, _sparse_paths, _destination| {
                panic!("git cloner should not be called for local marketplace sources")
            },
        )?;

        let expected_source = source_root.path().canonicalize()?.display().to_string();
        assert_eq!(result.marketplace_name, "debug");
        assert_eq!(result.source_display, expected_source);
        assert_eq!(
            result.installed_root.as_path(),
            source_root.path().canonicalize()?
        );
        assert!(!result.already_added);
        assert!(
            !marketplace_install_root(codex_home.path())
                .join("debug")
                .exists()
        );

        let config = fs::read_to_string(codex_home.path().join(codex_config::CONFIG_TOML_FILE))?;
        let config: toml::Value = toml::from_str(&config)?;
        assert_eq!(
            config["marketplaces"]["debug"]["source_type"].as_str(),
            Some("local")
        );
        assert_eq!(
            config["marketplaces"]["debug"]["source"].as_str(),
            Some(expected_source.as_str())
        );
        Ok(())
    }

    #[test]
    fn add_marketplace_sync_treats_existing_local_directory_source_as_already_added() -> Result<()>
    {
        let codex_home = TempDir::new()?;
        let source_root = TempDir::new()?;
        write_marketplace_source(source_root.path(), "local copy")?;

        let request = MarketplaceAddRequest {
            source: source_root.path().display().to_string(),
            ref_name: None,
            sparse_paths: Vec::new(),
        };
        let first_result = add_marketplace_sync_with_cloner(codex_home.path(), request.clone(), {
            |_url, _ref_name, _sparse_paths, _destination| {
                panic!("git cloner should not be called for local marketplace sources")
            }
        })?;
        let second_result = add_marketplace_sync_with_cloner(codex_home.path(), request, {
            |_url, _ref_name, _sparse_paths, _destination| {
                panic!("git cloner should not be called for local marketplace sources")
            }
        })?;

        assert!(!first_result.already_added);
        assert!(second_result.already_added);
        assert_eq!(second_result.installed_root, first_result.installed_root);

        Ok(())
    }

    fn write_marketplace_source(source: &Path, marker: &str) -> std::io::Result<()> {
        fs::create_dir_all(source.join(".agents/plugins"))?;
        write_plugin_source(source, "plugins/sample", "sample")?;
        fs::write(
            source.join(".agents/plugins/marketplace.json"),
            r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample",
      "source": {
        "source": "local",
        "path": "./plugins/sample"
      }
    }
  ]
}"#,
        )?;
        fs::write(source.join("plugins/sample/marker.txt"), marker)?;
        Ok(())
    }

    fn write_marketplace_source_with_remote_plugin(
        source: &Path,
        plugin_url: &str,
    ) -> std::io::Result<()> {
        fs::create_dir_all(source.join(".agents/plugins"))?;
        fs::write(
            source.join(".agents/plugins/marketplace.json"),
            format!(
                r#"{{
  "name": "debug",
  "plugins": [
    {{
      "name": "toolkit",
      "source": {{
        "source": "git-subdir",
        "url": "{plugin_url}",
        "path": "plugins/toolkit"
      }}
    }}
  ]
}}"#
            ),
        )?;
        Ok(())
    }

    fn write_plugin_source(
        source: &Path,
        plugin_path: &str,
        plugin_name: &str,
    ) -> std::io::Result<()> {
        fs::create_dir_all(source.join(plugin_path).join(".codex-plugin"))?;
        fs::write(
            source
                .join(plugin_path)
                .join(".codex-plugin")
                .join("plugin.json"),
            format!(r#"{{"name":"{plugin_name}"}}"#),
        )?;
        Ok(())
    }

    fn copy_dir_all(source: &Path, destination: &Path) -> std::io::Result<()> {
        fs::create_dir_all(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());
            if source_path.is_dir() {
                copy_dir_all(&source_path, &destination_path)?;
            } else {
                fs::copy(&source_path, &destination_path)?;
            }
        }
        Ok(())
    }
}
