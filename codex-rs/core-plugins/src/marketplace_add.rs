use crate::installed_marketplaces::marketplace_install_root;
use crate::marketplace_policy::validate_marketplace_name_for_add;
use crate::marketplace_policy::validate_marketplace_source_for_add;
use codex_config::ConfigRequirements;
use codex_utils_absolute_path::AbsolutePathBuf;
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
pub(crate) use source::MarketplaceSource;
pub(crate) use source::parse_marketplace_source;
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
    requirements: ConfigRequirements,
    request: MarketplaceAddRequest,
) -> Result<MarketplaceAddOutcome, MarketplaceAddError> {
    tokio::task::spawn_blocking(move || {
        add_marketplace_sync(codex_home.as_path(), &requirements, request)
    })
    .await
    .map_err(|err| MarketplaceAddError::Internal(format!("failed to add marketplace: {err}")))?
}

pub fn is_local_marketplace_source(
    source: &str,
    explicit_ref: Option<String>,
) -> Result<bool, MarketplaceAddError> {
    Ok(matches!(
        parse_marketplace_source(source, explicit_ref)?,
        source::MarketplaceSource::Local { .. }
    ))
}

fn add_marketplace_sync(
    codex_home: &Path,
    requirements: &ConfigRequirements,
    request: MarketplaceAddRequest,
) -> Result<MarketplaceAddOutcome, MarketplaceAddError> {
    add_marketplace_sync_with_cloner(codex_home, requirements, request, clone_git_source)
}

fn add_marketplace_sync_with_cloner<F>(
    codex_home: &Path,
    requirements: &ConfigRequirements,
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
    let managed_marketplace_name =
        validate_marketplace_source_for_add(codex_home, requirements, &source)
            .map_err(MarketplaceAddError::InvalidRequest)?;
    if !sparse_paths.is_empty() && !matches!(source, MarketplaceSource::Git { .. }) {
        return Err(MarketplaceAddError::InvalidRequest(
            "--sparse is only supported for git marketplace sources".to_string(),
        ));
    }

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
        let marketplace_name = validate_marketplace_source_root(&existing_root)?;
        validate_marketplace_name_for_add(managed_marketplace_name, &marketplace_name)
            .map_err(MarketplaceAddError::InvalidRequest)?;
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
        validate_marketplace_name_for_add(managed_marketplace_name, &marketplace_name)
            .map_err(MarketplaceAddError::InvalidRequest)?;
        if find_marketplace_root_by_name(codex_home, &install_root, &marketplace_name)?.is_some() {
            return Err(MarketplaceAddError::InvalidRequest(format!(
                "marketplace '{marketplace_name}' is already added from a different source; remove it before adding this source"
            )));
        }
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

    stage_marketplace_source(&source, &sparse_paths, &staged_root, clone_source)?;

    let marketplace_name = validate_marketplace_source_root(&staged_root)?;
    validate_marketplace_name_for_add(managed_marketplace_name, &marketplace_name)
        .map_err(MarketplaceAddError::InvalidRequest)?;

    let destination = install_root.join(safe_marketplace_dir_name(&marketplace_name)?);
    ensure_marketplace_destination_is_inside_install_root(&install_root, &destination)?;
    if destination.exists() {
        return Err(MarketplaceAddError::InvalidRequest(format!(
            "marketplace '{marketplace_name}' is already added from a different source; remove it before adding this source"
        )));
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use codex_config::RequirementSource;
    use codex_config::RequirementsLayerEntry;
    use codex_config::compose_requirements;
    use pretty_assertions::assert_eq;
    use std::cell::Cell;
    use tempfile::TempDir;

    fn requirements(requirements_toml: &str) -> ConfigRequirements {
        let with_sources = compose_requirements([RequirementsLayerEntry::from_toml(
            RequirementSource::Unknown,
            requirements_toml,
        )])
        .expect("compose requirements")
        .expect("requirements should be present");
        ConfigRequirements::try_from(with_sources).expect("normalize requirements")
    }

    #[test]
    fn add_marketplace_sync_installs_marketplace_and_updates_config() -> Result<()> {
        let codex_home = TempDir::new()?;
        let source_root = TempDir::new()?;
        write_marketplace_source(source_root.path(), "remote copy")?;

        let result = add_marketplace_sync_with_cloner(
            codex_home.path(),
            &ConfigRequirements::default(),
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
    fn denied_git_marketplace_does_not_clone_or_create_install_root() {
        let codex_home = TempDir::new().expect("create Codex home");
        let requirements = requirements(
            r#"
[marketplaces]
restrict_to_allowed_sources = true

[marketplaces.allowed_sources.company]
source = "git"
url = "https://github.com/example/allowed.git"
"#,
        );
        let cloner_called = Cell::new(false);

        let err = add_marketplace_sync_with_cloner(
            codex_home.path(),
            &requirements,
            MarketplaceAddRequest {
                source: "https://github.com/example/blocked.git".to_string(),
                ref_name: None,
                sparse_paths: Vec::new(),
            },
            |_url, _ref_name, _sparse_paths, _destination| {
                cloner_called.set(true);
                Ok(())
            },
        )
        .expect_err("blocked marketplace should fail");

        assert!(err.to_string().contains("is not allowed by requirements"));
        assert!(!cloner_called.get());
        assert!(!marketplace_install_root(codex_home.path()).exists());
        assert!(
            !codex_home
                .path()
                .join(codex_config::CONFIG_TOML_FILE)
                .exists()
        );
    }

    #[test]
    fn add_marketplace_sync_installs_local_directory_source_and_updates_config() -> Result<()> {
        let codex_home = TempDir::new()?;
        let source_root = TempDir::new()?;
        write_marketplace_source(source_root.path(), "local copy")?;

        let result = add_marketplace_sync_with_cloner(
            codex_home.path(),
            &ConfigRequirements::default(),
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
        let expected_installed_root =
            AbsolutePathBuf::from_absolute_path(source_root.path().canonicalize()?)?;
        assert_eq!(result.installed_root, expected_installed_root);
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
    fn add_marketplace_sync_rejects_sparse_checkout_for_local_directory_source() -> Result<()> {
        let codex_home = TempDir::new()?;
        let source_root = TempDir::new()?;
        write_marketplace_source(source_root.path(), "local copy")?;

        let err = add_marketplace_sync_with_cloner(
            codex_home.path(),
            &ConfigRequirements::default(),
            MarketplaceAddRequest {
                source: source_root.path().display().to_string(),
                ref_name: None,
                sparse_paths: vec![".agents".to_string()],
            },
            |_url, _ref_name, _sparse_paths, _destination| {
                panic!("git cloner should not be called for local marketplace sources")
            },
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "--sparse is only supported for git marketplace sources"
        );
        assert!(
            !codex_home
                .path()
                .join(codex_config::CONFIG_TOML_FILE)
                .exists()
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
        let requirements = ConfigRequirements::default();
        let first_result = add_marketplace_sync_with_cloner(
            codex_home.path(),
            &requirements,
            request.clone(),
            |_url, _ref_name, _sparse_paths, _destination| {
                panic!("git cloner should not be called for local marketplace sources")
            },
        )?;
        let second_result = add_marketplace_sync_with_cloner(
            codex_home.path(),
            &requirements,
            request,
            |_url, _ref_name, _sparse_paths, _destination| {
                panic!("git cloner should not be called for local marketplace sources")
            },
        )?;

        assert!(!first_result.already_added);
        assert!(second_result.already_added);
        assert_eq!(second_result.installed_root, first_result.installed_root);

        Ok(())
    }

    fn write_marketplace_source(source: &Path, marker: &str) -> std::io::Result<()> {
        fs::create_dir_all(source.join(".agents/plugins"))?;
        fs::create_dir_all(source.join("plugins/sample/.codex-plugin"))?;
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
        fs::write(
            source.join("plugins/sample/.codex-plugin/plugin.json"),
            r#"{"name":"sample"}"#,
        )?;
        fs::write(source.join("plugins/sample/marker.txt"), marker)?;
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
