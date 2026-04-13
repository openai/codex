mod activation;
mod git;

use self::activation::activate_marketplace_root;
use self::activation::activated_marketplace_metadata_matches;
use self::activation::write_activated_marketplace_metadata;
use self::git::clone_git_source;
use self::git::git_remote_revision;
use super::installed_marketplaces::marketplace_install_root;
use super::validate_marketplace_root;
use codex_config::MarketplaceConfigUpdate;
use codex_config::record_user_marketplace;
use codex_config::types::MarketplaceConfig;
use codex_config::types::MarketplaceSourceType;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use tracing::warn;

use crate::config::CONFIG_TOML_FILE;
use crate::config::Config;

const MARKETPLACE_UPGRADE_GIT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfiguredMarketplaceUpgradeError {
    pub marketplace_name: String,
    pub message: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConfiguredMarketplaceUpgradeOutcome {
    pub selected_marketplaces: Vec<String>,
    pub upgraded_roots: Vec<AbsolutePathBuf>,
    pub errors: Vec<ConfiguredMarketplaceUpgradeError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfiguredGitMarketplace {
    name: String,
    source: String,
    ref_name: Option<String>,
    sparse_paths: Vec<String>,
    last_revision: Option<String>,
}

impl ConfiguredMarketplaceUpgradeOutcome {
    pub fn all_succeeded(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn configured_git_marketplace_names(config: &Config) -> Vec<String> {
    let mut names = configured_git_marketplaces(config)
        .into_iter()
        .map(|marketplace| marketplace.name)
        .collect::<Vec<_>>();
    names.sort_unstable();
    names
}

pub fn upgrade_configured_git_marketplaces(
    codex_home: &Path,
    config: &Config,
    marketplace_name: Option<&str>,
) -> ConfiguredMarketplaceUpgradeOutcome {
    let marketplaces = configured_git_marketplaces(config)
        .into_iter()
        .filter(|marketplace| marketplace_name.is_none_or(|name| marketplace.name.as_str() == name))
        .collect::<Vec<_>>();
    if marketplaces.is_empty() {
        return ConfiguredMarketplaceUpgradeOutcome::default();
    }

    let install_root = marketplace_install_root(codex_home);
    let selected_marketplaces = marketplaces
        .iter()
        .map(|marketplace| marketplace.name.clone())
        .collect();
    let mut upgraded_roots = Vec::new();
    let mut errors = Vec::new();
    for marketplace in marketplaces {
        match upgrade_configured_git_marketplace(codex_home, &install_root, &marketplace) {
            Ok(Some(upgraded_root)) => upgraded_roots.push(upgraded_root),
            Ok(None) => {}
            Err(err) => {
                errors.push(ConfiguredMarketplaceUpgradeError {
                    marketplace_name: marketplace.name,
                    message: err,
                });
            }
        }
    }

    ConfiguredMarketplaceUpgradeOutcome {
        selected_marketplaces,
        upgraded_roots,
        errors,
    }
}

fn configured_git_marketplaces(config: &Config) -> Vec<ConfiguredGitMarketplace> {
    let Some(user_layer) = config.config_layer_stack.get_user_layer() else {
        return Vec::new();
    };
    let Some(marketplaces_value) = user_layer.config.get("marketplaces") else {
        return Vec::new();
    };
    let marketplaces = match marketplaces_value
        .clone()
        .try_into::<HashMap<String, MarketplaceConfig>>()
    {
        Ok(marketplaces) => marketplaces,
        Err(err) => {
            warn!("invalid marketplaces config while preparing auto-upgrade: {err}");
            return Vec::new();
        }
    };

    let mut configured = marketplaces
        .into_iter()
        .filter_map(|(name, marketplace)| configured_git_marketplace_from_config(name, marketplace))
        .collect::<Vec<_>>();
    configured.sort_unstable_by(|left, right| left.name.cmp(&right.name));
    configured
}

fn configured_git_marketplace_from_config(
    name: String,
    marketplace: MarketplaceConfig,
) -> Option<ConfiguredGitMarketplace> {
    let MarketplaceConfig {
        last_updated: _,
        last_revision,
        source_type,
        source,
        ref_name,
        sparse_paths,
    } = marketplace;
    if source_type != Some(MarketplaceSourceType::Git) {
        return None;
    }
    let Some(source) = source else {
        warn!(
            marketplace = name,
            "ignoring configured Git marketplace without source"
        );
        return None;
    };
    Some(ConfiguredGitMarketplace {
        name,
        source,
        ref_name,
        sparse_paths: sparse_paths.unwrap_or_default(),
        last_revision,
    })
}

fn upgrade_configured_git_marketplace(
    codex_home: &Path,
    install_root: &Path,
    marketplace: &ConfiguredGitMarketplace,
) -> Result<Option<AbsolutePathBuf>, String> {
    super::validate_plugin_segment(&marketplace.name, "marketplace name")?;
    let remote_revision = git_remote_revision(
        &marketplace.source,
        marketplace.ref_name.as_deref(),
        MARKETPLACE_UPGRADE_GIT_TIMEOUT,
    )?;
    let destination = install_root.join(&marketplace.name);
    if destination
        .join(".agents/plugins/marketplace.json")
        .is_file()
        && marketplace.last_revision.as_deref() == Some(remote_revision.as_str())
        && activated_marketplace_metadata_matches(&destination, marketplace, &remote_revision)
    {
        return Ok(None);
    }

    let staging_parent = install_root.join(".staging");
    std::fs::create_dir_all(&staging_parent).map_err(|err| {
        format!(
            "failed to create marketplace upgrade staging directory {}: {err}",
            staging_parent.display()
        )
    })?;
    let staged_dir = tempfile::Builder::new()
        .prefix("marketplace-upgrade-")
        .tempdir_in(&staging_parent)
        .map_err(|err| {
            format!(
                "failed to create temporary marketplace upgrade directory in {}: {err}",
                staging_parent.display()
            )
        })?;

    let activated_revision = clone_git_source(
        &marketplace.source,
        marketplace.ref_name.as_deref(),
        &marketplace.sparse_paths,
        staged_dir.path(),
        MARKETPLACE_UPGRADE_GIT_TIMEOUT,
    )?;
    let marketplace_name = validate_marketplace_root(staged_dir.path())
        .map_err(|err| format!("failed to validate upgraded marketplace root: {err}"))?;
    if marketplace_name != marketplace.name {
        return Err(format!(
            "upgraded marketplace name `{marketplace_name}` does not match configured marketplace `{}`",
            marketplace.name
        ));
    }
    write_activated_marketplace_metadata(staged_dir.path(), marketplace, &activated_revision)?;

    let last_updated = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let update = MarketplaceConfigUpdate {
        last_updated: &last_updated,
        last_revision: Some(&activated_revision),
        source_type: "git",
        source: &marketplace.source,
        ref_name: marketplace.ref_name.as_deref(),
        sparse_paths: &marketplace.sparse_paths,
    };
    activate_marketplace_root(&destination, staged_dir, || {
        ensure_configured_git_marketplace_unchanged(codex_home, marketplace)?;
        record_user_marketplace(codex_home, &marketplace.name, &update).map_err(|err| {
            format!(
                "failed to record upgraded marketplace `{}` in user config.toml: {err}",
                marketplace.name
            )
        })
    })?;

    AbsolutePathBuf::try_from(destination)
        .map(Some)
        .map_err(|err| format!("upgraded marketplace path is not absolute: {err}"))
}
fn ensure_configured_git_marketplace_unchanged(
    codex_home: &Path,
    expected: &ConfiguredGitMarketplace,
) -> Result<(), String> {
    let current = read_configured_git_marketplace(codex_home, &expected.name)?;
    match current {
        Some(current) if current == *expected => Ok(()),
        Some(_) => Err(format!(
            "configured marketplace `{}` changed while auto-upgrade was in flight",
            expected.name
        )),
        None => Err(format!(
            "configured marketplace `{}` was removed or is no longer a Git marketplace",
            expected.name
        )),
    }
}

fn read_configured_git_marketplace(
    codex_home: &Path,
    marketplace_name: &str,
) -> Result<Option<ConfiguredGitMarketplace>, String> {
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    let raw_config = match std::fs::read_to_string(&config_path) {
        Ok(raw_config) => raw_config,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(format!(
                "failed to read user config {} while checking marketplace auto-upgrade: {err}",
                config_path.display()
            ));
        }
    };
    let config: toml::Value = toml::from_str(&raw_config).map_err(|err| {
        format!(
            "failed to parse user config {} while checking marketplace auto-upgrade: {err}",
            config_path.display()
        )
    })?;
    let Some(marketplaces_value) = config.get("marketplaces") else {
        return Ok(None);
    };
    let mut marketplaces = marketplaces_value
        .clone()
        .try_into::<HashMap<String, MarketplaceConfig>>()
        .map_err(|err| format!("invalid marketplaces config while checking auto-upgrade: {err}"))?;
    let Some(marketplace) = marketplaces.remove(marketplace_name) else {
        return Ok(None);
    };
    Ok(configured_git_marketplace_from_config(
        marketplace_name.to_string(),
        marketplace,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    use crate::config::CONFIG_TOML_FILE;
    use crate::plugins::test_support::load_plugins_config;
    use crate::plugins::test_support::write_file;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[tokio::test]
    async fn upgrade_configured_git_marketplace_installs_new_revision() {
        let codex_home = TempDir::new().unwrap();
        let source_repo = TempDir::new().unwrap();
        write_marketplace_repo(source_repo.path(), "debug", "new");
        init_git_repo(source_repo.path());
        let revision = git_output(source_repo.path(), &["rev-parse", "HEAD"]);
        write_file(
            &codex_home.path().join(CONFIG_TOML_FILE),
            &marketplace_config(source_repo.path(), "old-revision"),
        );

        let config = load_plugins_config(codex_home.path()).await;
        let outcome = upgrade_configured_git_marketplaces(codex_home.path(), &config, None);

        assert_eq!(
            outcome,
            ConfiguredMarketplaceUpgradeOutcome {
                selected_marketplaces: vec!["debug".to_string()],
                upgraded_roots: vec![
                    AbsolutePathBuf::try_from(
                        marketplace_install_root(codex_home.path()).join("debug")
                    )
                    .unwrap()
                ],
                errors: Vec::new(),
            }
        );
        assert_eq!(
            std::fs::read_to_string(
                marketplace_install_root(codex_home.path()).join("debug/plugins/sample/marker.txt")
            )
            .unwrap(),
            "new"
        );
        let config = std::fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE)).unwrap();
        assert!(config.contains(&format!(r#"last_revision = "{revision}""#)));
    }

    #[tokio::test]
    async fn upgrade_configured_git_marketplace_skips_matching_revision() {
        let codex_home = TempDir::new().unwrap();
        let source_repo = TempDir::new().unwrap();
        write_marketplace_repo(source_repo.path(), "debug", "new");
        init_git_repo(source_repo.path());
        let revision = git_output(source_repo.path(), &["rev-parse", "HEAD"]);
        let installed_root = marketplace_install_root(codex_home.path()).join("debug");
        write_marketplace_repo(&installed_root, "debug", "old");
        write_installed_metadata(&installed_root, source_repo.path(), None, &[], &revision);
        write_file(
            &codex_home.path().join(CONFIG_TOML_FILE),
            &marketplace_config(source_repo.path(), &revision),
        );

        let config = load_plugins_config(codex_home.path()).await;
        let outcome = upgrade_configured_git_marketplaces(codex_home.path(), &config, None);

        assert_eq!(
            outcome,
            ConfiguredMarketplaceUpgradeOutcome {
                selected_marketplaces: vec!["debug".to_string()],
                upgraded_roots: Vec::new(),
                errors: Vec::new(),
            }
        );
        assert_eq!(
            std::fs::read_to_string(installed_root.join("plugins/sample/marker.txt")).unwrap(),
            "old"
        );
    }

    #[tokio::test]
    async fn upgrade_configured_git_marketplace_reclones_when_install_metadata_differs() {
        let codex_home = TempDir::new().unwrap();
        let source_repo = TempDir::new().unwrap();
        write_marketplace_repo(source_repo.path(), "debug", "new");
        init_git_repo(source_repo.path());
        let revision = git_output(source_repo.path(), &["rev-parse", "HEAD"]);
        let installed_root = marketplace_install_root(codex_home.path()).join("debug");
        write_marketplace_repo(&installed_root, "debug", "old");
        write_installed_metadata(&installed_root, source_repo.path(), None, &[], &revision);
        write_file(
            &codex_home.path().join(CONFIG_TOML_FILE),
            &marketplace_config_with_ref(source_repo.path(), &revision, &revision),
        );

        let config = load_plugins_config(codex_home.path()).await;
        let outcome = upgrade_configured_git_marketplaces(codex_home.path(), &config, None);

        assert_eq!(
            outcome,
            ConfiguredMarketplaceUpgradeOutcome {
                selected_marketplaces: vec!["debug".to_string()],
                upgraded_roots: vec![
                    AbsolutePathBuf::try_from(
                        marketplace_install_root(codex_home.path()).join("debug")
                    )
                    .unwrap()
                ],
                errors: Vec::new(),
            }
        );
        assert_eq!(
            std::fs::read_to_string(installed_root.join("plugins/sample/marker.txt")).unwrap(),
            "new"
        );
    }

    #[tokio::test]
    async fn upgrade_configured_git_marketplace_keeps_existing_root_on_name_mismatch() {
        let codex_home = TempDir::new().unwrap();
        let source_repo = TempDir::new().unwrap();
        write_marketplace_repo(source_repo.path(), "other", "new");
        init_git_repo(source_repo.path());
        let installed_root = marketplace_install_root(codex_home.path()).join("debug");
        write_marketplace_repo(&installed_root, "debug", "old");
        write_file(
            &codex_home.path().join(CONFIG_TOML_FILE),
            &marketplace_config(source_repo.path(), "old-revision"),
        );

        let config = load_plugins_config(codex_home.path()).await;
        let outcome = upgrade_configured_git_marketplaces(codex_home.path(), &config, None);

        assert_eq!(outcome.selected_marketplaces, vec!["debug".to_string()]);
        assert!(outcome.upgraded_roots.is_empty());
        assert_eq!(outcome.errors.len(), 1);
        assert_eq!(outcome.errors[0].marketplace_name, "debug");
        assert_eq!(
            std::fs::read_to_string(installed_root.join("plugins/sample/marker.txt")).unwrap(),
            "old"
        );
    }

    #[tokio::test]
    async fn upgrade_configured_git_marketplace_keeps_existing_root_on_git_failure() {
        let codex_home = TempDir::new().unwrap();
        let missing_repo = codex_home.path().join("missing-repo");
        let installed_root = marketplace_install_root(codex_home.path()).join("debug");
        write_marketplace_repo(&installed_root, "debug", "old");
        write_file(
            &codex_home.path().join(CONFIG_TOML_FILE),
            &marketplace_config(&missing_repo, "old-revision"),
        );

        let config = load_plugins_config(codex_home.path()).await;
        let outcome = upgrade_configured_git_marketplaces(codex_home.path(), &config, None);

        assert_eq!(outcome.selected_marketplaces, vec!["debug".to_string()]);
        assert!(outcome.upgraded_roots.is_empty());
        assert_eq!(outcome.errors.len(), 1);
        assert_eq!(outcome.errors[0].marketplace_name, "debug");
        assert_eq!(
            std::fs::read_to_string(installed_root.join("plugins/sample/marker.txt")).unwrap(),
            "old"
        );
    }

    #[tokio::test]
    async fn upgrade_configured_git_marketplace_rolls_back_when_config_changes() {
        let codex_home = TempDir::new().unwrap();
        let source_repo = TempDir::new().unwrap();
        write_marketplace_repo(source_repo.path(), "debug", "new");
        init_git_repo(source_repo.path());
        let changed_source_repo = TempDir::new().unwrap();
        write_marketplace_repo(changed_source_repo.path(), "debug", "changed");
        init_git_repo(changed_source_repo.path());
        let installed_root = marketplace_install_root(codex_home.path()).join("debug");
        write_marketplace_repo(&installed_root, "debug", "old");
        write_file(
            &codex_home.path().join(CONFIG_TOML_FILE),
            &marketplace_config(source_repo.path(), "old-revision"),
        );
        let config = load_plugins_config(codex_home.path()).await;
        write_file(
            &codex_home.path().join(CONFIG_TOML_FILE),
            &marketplace_config(changed_source_repo.path(), "changed-revision"),
        );

        let outcome = upgrade_configured_git_marketplaces(codex_home.path(), &config, None);

        assert_eq!(outcome.selected_marketplaces, vec!["debug".to_string()]);
        assert!(outcome.upgraded_roots.is_empty());
        assert_eq!(outcome.errors.len(), 1);
        assert_eq!(outcome.errors[0].marketplace_name, "debug");
        assert_eq!(
            std::fs::read_to_string(installed_root.join("plugins/sample/marker.txt")).unwrap(),
            "old"
        );
        let config = std::fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE)).unwrap();
        assert!(config.contains(&changed_source_repo.path().display().to_string()));
        assert!(config.contains(r#"last_revision = "changed-revision""#));
    }

    #[tokio::test]
    async fn upgrade_configured_git_marketplaces_ignores_local_unconfigured_marketplace() {
        let codex_home = TempDir::new().unwrap();
        write_marketplace_repo(codex_home.path(), "local", "local");
        write_file(
            &codex_home.path().join(CONFIG_TOML_FILE),
            r#"[features]
plugins = true
"#,
        );

        let config = load_plugins_config(codex_home.path()).await;
        let outcome = upgrade_configured_git_marketplaces(codex_home.path(), &config, None);

        assert_eq!(
            outcome,
            ConfiguredMarketplaceUpgradeOutcome {
                selected_marketplaces: Vec::new(),
                upgraded_roots: Vec::new(),
                errors: Vec::new(),
            }
        );
        assert!(
            !marketplace_install_root(codex_home.path())
                .join("local")
                .exists()
        );
    }

    fn marketplace_config(source_repo: &Path, last_revision: &str) -> String {
        format!(
            r#"[features]
plugins = true

[marketplaces.debug]
last_updated = "2026-04-10T00:00:00Z"
last_revision = "{last_revision}"
source_type = "git"
source = "{}"
"#,
            source_repo.display()
        )
    }

    fn marketplace_config_with_ref(
        source_repo: &Path,
        last_revision: &str,
        ref_name: &str,
    ) -> String {
        format!(
            r#"[features]
plugins = true

[marketplaces.debug]
last_updated = "2026-04-10T00:00:00Z"
last_revision = "{last_revision}"
source_type = "git"
source = "{}"
ref = "{ref_name}"
"#,
            source_repo.display()
        )
    }

    fn write_installed_metadata(
        root: &Path,
        source_repo: &Path,
        ref_name: Option<&str>,
        sparse_paths: &[String],
        revision: &str,
    ) {
        let marketplace = ConfiguredGitMarketplace {
            name: "debug".to_string(),
            source: source_repo.display().to_string(),
            ref_name: ref_name.map(str::to_string),
            sparse_paths: sparse_paths.to_vec(),
            last_revision: Some(revision.to_string()),
        };
        write_activated_marketplace_metadata(root, &marketplace, revision)
            .expect("metadata should write");
    }

    fn write_marketplace_repo(root: &Path, marketplace_name: &str, marker: &str) {
        write_file(
            &root.join(".agents/plugins/marketplace.json"),
            &format!(
                r#"{{
  "name": "{marketplace_name}",
  "plugins": [
    {{
      "name": "sample",
      "source": {{
        "source": "local",
        "path": "./plugins/sample"
      }}
    }}
  ]
}}"#
            ),
        );
        write_file(
            &root.join("plugins/sample/.codex-plugin/plugin.json"),
            r#"{"name":"sample"}"#,
        );
        write_file(&root.join("plugins/sample/marker.txt"), marker);
    }

    fn init_git_repo(repo: &Path) {
        git(repo, &["init"]);
        git(repo, &["config", "user.email", "codex-test@example.com"]);
        git(repo, &["config", "user.name", "Codex Test"]);
        git(repo, &["add", "."]);
        git(repo, &["commit", "-m", "initial marketplace"]);
    }

    fn git(repo: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .expect("git should run");
        assert!(
            output.status.success(),
            "git -C {} {} failed\nstdout:\n{}\nstderr:\n{}",
            repo.display(),
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn git_output(repo: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .expect("git should run");
        assert!(
            output.status.success(),
            "git -C {} {} failed\nstdout:\n{}\nstderr:\n{}",
            repo.display(),
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }
}
